//! The players' skeleton and every animation record's clip as plain data,
//! for the simulation's rules that read bones (`street_sim::pose`): the
//! skeleton of `XF000002`, the 1144 animation records of `0x3de8a0` (a
//! clip's name and mirror flag) and, for each clip they name, its rotation
//! track (the native readers' values), its root translation in court units
//! and its `_X` state words.

use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::{Result, anim, eagl, iso::Iso, player, timeline};

/// One clip as the simulation samples it.
#[derive(Clone, Debug, PartialEq)]
pub struct PoseClip {
    pub name: String,
    /// An `FnRawPoseChannel` rotation track.
    pub raw_pose: bool,
    /// The skeleton bone of each rotation channel.
    pub bones: Vec<u8>,
    /// `rotations[frame][channel]`, quaternions (x, y, z, w).
    pub rotations: Vec<Vec<[f32; 4]>>,
    /// The root's translation per frame, in court units at scale 1
    /// ([`anim::TRANSLATION_SCALE`]); empty when the clip has none.
    pub root: Vec<[f32; 3]>,
    /// The `_X` state word per frame; empty when the clip has none.
    pub states: Vec<u32>,
}

/// The skeleton, the clips and each record's clip and mirror flag.
#[derive(Clone, Debug)]
pub struct PoseData {
    pub skeleton: eagl::Skeleton,
    pub clips: Vec<PoseClip>,
    pub records: Vec<Option<(u16, bool)>>,
}

impl PoseData {
    /// Every clip an animation record names, from the first bank that
    /// holds it (`anim::BANKS` order).
    pub fn read<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self> {
        let skeleton = player::skeleton(iso)?;
        let records = timeline::animation_records(&iso.read("/SLUS_206.51")?)?;
        let wanted: std::collections::HashSet<&str> =
            records.iter().filter_map(|r| r.name.as_deref()).collect();
        let mut index: HashMap<String, u16> = HashMap::new();
        let mut clips = Vec::new();
        for path in anim::BANKS {
            let bank = anim::load(iso, path)?;
            let names: Vec<String> = bank.animations().map(str::to_owned).collect();
            for name in names {
                if index.contains_key(&name) || !wanted.contains(name.as_str()) {
                    continue;
                }
                let (bones, rotations, raw_pose) = match bank.native_rotations(&name)? {
                    Some(n) => (
                        n.bones,
                        n.frames,
                        n.interpolation == anim::NativeInterpolation::RawPose,
                    ),
                    None => match bank.rotations(&name)? {
                        Some(r) => (r.bones, r.frames, false),
                        None => continue,
                    },
                };
                let root = bank
                    .root_motion(&name)?
                    .map(|r| {
                        r.frames
                            .iter()
                            .map(|f| f.map(|c| c * anim::TRANSLATION_SCALE))
                            .collect()
                    })
                    .unwrap_or_default();
                let states = bank.states(&name)?.unwrap_or_default();
                index.insert(name.clone(), clips.len() as u16);
                clips.push(PoseClip {
                    name,
                    raw_pose,
                    bones,
                    rotations,
                    root,
                    states,
                });
            }
        }
        let records = records
            .iter()
            .map(|r| {
                let clip = *index.get(r.name.as_deref()?)?;
                Some((clip, r.mirrored))
            })
            .collect();
        Ok(Self {
            skeleton,
            clips,
            records,
        })
    }
}
