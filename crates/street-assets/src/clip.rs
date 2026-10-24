//! One decoded animation ([`Clip`]) and the disc's clips by name
//! ([`Clips`]): decoding from the animation banks ([`crate::anim`]) and
//! sampling a joint, for the renderer (`street-render`'s `animation`) and
//! for the tables every machine in a match builds from its own disc
//! (`street-net`'s `disc`: the staged gamebreakers' hands, the clips'
//! state words). One implementation, so a headless match server samples
//! exactly what the game samples.
//!
//! Animations face the opposite way from the rest pose (their root rotation is
//! the rest pose's turned half a turn about y), so an animated player faces
//! -z in its own space.

use std::{
    collections::HashMap,
    io::{Read, Seek},
    sync::Arc,
};

use glam::{Mat4, Quat, Vec3};

use crate::{anim, iso::Iso};

/// Frames per second the game's animations are authored at.
pub const ANIM_FPS: f32 = 30.0;

/// Width relative to the outer height scale, as the native limb offsets use it.
/// Uniform, narrow, and database-free kit bodies retain their rest offsets.
pub fn native_body_width_ratio(body: Option<[f32; 3]>) -> f32 {
    match body {
        Some([height, width, _]) if width > height && height > 0.0 => width / height,
        _ => 1.0,
    }
}

/// `19d4c8` uses width scale on four player-joint translation components.
/// The player's outer transform already applies uniform height scale.
pub fn native_joint_translation(
    index: usize,
    translation: [f32; 3],
    body: Option<[f32; 3]>,
) -> Vec3 {
    let mut position = Vec3::from_array(translation);
    let ratio = native_body_width_ratio(body);
    if matches!(index, 1 | 5) {
        position.z *= ratio;
    } else if matches!(index, 13 | 17) {
        position.x *= ratio;
    }
    position
}

/// One decoded animation.
pub struct Clip {
    pub name: String,
    pub states: Vec<u32>,
    pub rotations: anim::Rotations,
    /// QFast stores raw quaternions and linearly interpolates components.
    pub native_interpolation: Option<anim::NativeInterpolation>,
    pub root: Option<anim::RootMotion>,
    /// Skeleton bone -> channel.
    pub channel_of: HashMap<usize, usize>,
}

impl Clip {
    /// Gameplay's `_X` flag word, wrapping with the clip. Staged hand
    /// endpoint sampling truncates its native frame before this call.
    pub fn state_at(&self, frame: usize) -> u32 {
        if self.states.is_empty() {
            return 0;
        }
        self.states[frame % self.states.len()]
    }

    /// State word for an animation record's mirror flag. FUN_00191440
    /// swaps five left/right flag pairs at 0x4546a8 when the mirrored row
    /// is loaded. Pose mirroring and this state remap are separate steps.
    pub fn state_at_mirrored(&self, frame: usize, mirrored: bool) -> u32 {
        let mut state = self.state_at(frame);
        if mirrored {
            for (left, right) in [(1, 2), (4, 8), (0x40, 0x200), (0x80, 0x400), (0x100, 0x800)] {
                if (state & left != 0) != (state & right != 0) {
                    state ^= left | right;
                }
            }
        }
        state
    }
    pub fn frames(&self) -> usize {
        self.rotations.frames.len()
    }

    pub fn seconds(&self) -> f32 {
        self.frames() as f32 / ANIM_FPS
    }

    /// Sample a joint in player-local space with the rest body's translations.
    /// [`Self::joint_position_with_body_scale`] also applies a database player's
    /// native width adjustment, as the spawned rig does.
    pub fn joint_position(
        &self,
        skeleton: &crate::eagl::Skeleton,
        joint: usize,
        frame: f32,
        mirrored: bool,
        in_place: bool,
    ) -> Option<Vec3> {
        self.joint_position_with_offset(skeleton, joint, frame, mirrored, in_place, Vec3::ZERO)
    }

    /// Transform a point expressed in the selected joint's local coordinates
    /// into player-local space. The animation, mirror and in-place rules match
    /// [`Self::joint_position`].
    pub fn joint_position_with_offset(
        &self,
        skeleton: &crate::eagl::Skeleton,
        joint: usize,
        frame: f32,
        mirrored: bool,
        in_place: bool,
        local_offset: Vec3,
    ) -> Option<Vec3> {
        self.sample_joint_position(
            skeleton,
            joint,
            frame,
            mirrored,
            in_place,
            local_offset,
            None,
        )
    }

    /// Sample the same native width-adjusted joint translations installed by
    /// `player::spawn`. Outer uniform height and size-option scale are applied
    /// by the consumer, after sampling the local model geometry.
    pub fn joint_position_with_body_scale(
        &self,
        skeleton: &crate::eagl::Skeleton,
        joint: usize,
        frame: f32,
        mirrored: bool,
        in_place: bool,
        body: Option<[f32; 3]>,
    ) -> Option<Vec3> {
        self.sample_joint_position(skeleton, joint, frame, mirrored, in_place, Vec3::ZERO, body)
    }

    #[expect(clippy::too_many_arguments)]
    fn sample_joint_position(
        &self,
        skeleton: &crate::eagl::Skeleton,
        joint: usize,
        frame: f32,
        mirrored: bool,
        in_place: bool,
        local_offset: Vec3,
        body: Option<[f32; 3]>,
    ) -> Option<Vec3> {
        skeleton.bones.get(joint)?;
        let (mirror_bones, native_mirror) = mirror_layout(skeleton);
        let mut matrices: Vec<Mat4> = Vec::with_capacity(skeleton.bones.len());
        for (index, bone) in skeleton.bones.iter().enumerate().take(joint + 1) {
            let source = if mirrored { mirror_bones[index] } else { index };
            let rotation = self
                .rotation(source, frame, false)
                .map(|q| mirrored_rotation(index, q, mirrored, native_mirror))
                .unwrap_or_else(|| Quat::from_array(bone.rotation));
            let mut translation = if index == 0 {
                self.root(frame, false, in_place)
                    .unwrap_or_else(|| Vec3::from_array(bone.translation))
            } else {
                native_joint_translation(index, bone.translation, body)
            };
            if index == 0 && mirrored {
                translation.x = -translation.x;
            }
            let local = Mat4::from_scale_rotation_translation(
                Vec3::from_array(bone.scale),
                rotation,
                translation,
            );
            let matrix = match bone.parent {
                Some(parent) => *matrices.get(parent)? * local,
                None => local,
            };
            matrices.push(matrix);
        }
        Some(matrices[joint].transform_point3(local_offset))
    }

    /// The local rotation of `bone` at `frame` (fractional), if the clip animates it.
    pub fn rotation(&self, bone: usize, frame: f32, looping: bool) -> Option<Quat> {
        let c = *self.channel_of.get(&bone)?;
        let (f0, f1, t) = self.sample_at(frame, looping);
        let a = Quat::from_array(self.rotations.frames[f0][c]);
        let b = Quat::from_array(self.rotations.frames[f1][c]);
        if self.native_interpolation == Some(anim::NativeInterpolation::RawPose) {
            // Original 00379b20 chooses the shorter signed component path,
            // retaining the endpoint lengths before normalizing the blend.
            // Host arithmetic here is not bit-exact VU arithmetic.
            let difference = b - a;
            let sum = b + a;
            let second = b * t;
            let first = a * (1.0 - t);
            let blend = if difference.length_squared() > sum.length_squared() {
                second - first
            } else {
                second + first
            };
            Some(blend.normalize())
        } else if self.native_interpolation == Some(anim::NativeInterpolation::Components) {
            // FUN_0038d834..0038d84c interpolates the decoded components,
            // including their non-unit lengths. Normalize only at the Bevy
            // display boundary; native physics must retain the raw values.
            let a = a.to_array();
            let b = b.to_array();
            let q = Quat::from_array(std::array::from_fn(|axis| {
                a[axis] + (b[axis] - a[axis]) * t
            }));
            Some(q.normalize())
        } else {
            Some(a.slerp(b, t))
        }
    }

    /// The root bone's local rotation at `frame` (fractional), if the clip
    /// animates it: what `FUN_00196698` reads from a blend's incoming root.
    pub fn root_rotation(&self, frame: f32, looping: bool) -> Option<Quat> {
        self.rotation(0, frame, looping)
    }

    /// The root's translation at `frame`, in skeleton units. In place, the
    /// root keeps its first frame's spot on the floor and only moves up and down.
    pub fn root(&self, frame: f32, looping: bool, in_place: bool) -> Option<Vec3> {
        let root = self.root.as_ref()?;
        let (f0, f1, t) = self.sample_at(frame, looping);
        let at = |f: usize| Vec3::from_array(root.frames[f.min(root.frames.len() - 1)]);
        let mut p = at(f0).lerp(at(f1), t);
        if in_place {
            let first = at(0);
            p.x = first.x;
            p.z = first.z;
        }
        Some(p * anim::TRANSLATION_SCALE)
    }

    fn sample_at(&self, frame: f32, looping: bool) -> (usize, usize, f32) {
        let n = self.frames().max(1);
        let frame = if looping {
            frame.rem_euclid(n as f32)
        } else {
            frame.clamp(0.0, (n - 1) as f32)
        };
        let f0 = (frame as usize).min(n - 1);
        let f1 = if looping {
            (f0 + 1) % n
        } else {
            (f0 + 1).min(n - 1)
        };
        (f0, f1, frame.fract())
    }
}

/// Decoded clips by name, loaded from the disc's banks on request.
#[derive(Default)]
pub struct Clips {
    clips: HashMap<String, Arc<Clip>>,
}

impl Clips {
    /// All clips on the user's disc, including script-selected alternates.
    pub fn load_all<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut names = std::collections::BTreeSet::new();
        for path in anim::BANKS {
            names.extend(anim::load(iso, path)?.animations().map(str::to_owned));
        }
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        Self::load(iso, &names)
    }
    /// Decode the named animations from whichever banks hold them.
    pub fn load<R: Read + Seek>(
        iso: &mut Iso<R>,
        names: &[&str],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut clips = HashMap::new();
        for path in anim::BANKS {
            if clips.len() == names.len() {
                break;
            }
            let bank = anim::load(iso, path)?;
            let available: std::collections::HashSet<&str> = bank.animations().collect();
            for &name in names {
                if clips.contains_key(name) || !available.contains(name) {
                    continue;
                }
                let (rotations, native_interpolation) = match bank.native_rotations(name)? {
                    Some(raw) => (
                        anim::Rotations {
                            bones: raw.bones,
                            frames: raw.frames,
                        },
                        Some(raw.interpolation),
                    ),
                    None => (
                        bank.rotations(name)?.ok_or_else(|| {
                            format!("{name}: rotation channel type not decoded yet")
                        })?,
                        None,
                    ),
                };
                let channel_of = rotations
                    .bones
                    .iter()
                    .enumerate()
                    .map(|(c, &b)| (b as usize, c))
                    .collect();
                clips.insert(
                    name.to_owned(),
                    Arc::new(Clip {
                        name: name.to_owned(),
                        states: bank
                            .states(name)?
                            .ok_or_else(|| format!("{name}: state channel type not decoded"))?,
                        rotations,
                        native_interpolation,
                        root: bank.root_motion(name)?,
                        channel_of,
                    }),
                );
            }
        }
        if let Some(missing) = names.iter().find(|n| !clips.contains_key(**n)) {
            return Err(format!("no animation named {missing:?}").into());
        }
        Ok(Self { clips })
    }

    pub fn get(&self, name: &str) -> Option<Arc<Clip>> {
        self.clips.get(name).cloned()
    }
}

pub fn mirror_rotation(q: Quat) -> Quat {
    Quat::from_xyzw(q.x, -q.y, -q.z, q.w)
}

// FUN_001914c8's fourteen triples at 0x3e6790 cover these 22 main
// player bones. Auxiliary facial bones are outside that native operation.
pub const NATIVE_MIRROR_BONES: [usize; 22] = [
    0, 5, 6, 7, 8, 1, 2, 3, 4, 9, 10, 11, 16, 17, 18, 19, 12, 13, 14, 15, 20, 21,
];
const NATIVE_PLAYER_NAMES: [&str; 22] = [
    "Root",
    "LThigh",
    "LLowLeg",
    "LFoot",
    "LToe",
    "RThigh",
    "RLowLeg",
    "RFoot",
    "RToe",
    "LowerTorso",
    "MidTorso",
    "UpperTorso",
    "LCollarBone",
    "LUpperArm",
    "LLowArm",
    "LHand",
    "RCollarBone",
    "RUpperArm",
    "RLowArm",
    "RHand",
    "Neck",
    "Head",
];

pub fn mirror_layout(skeleton: &crate::eagl::Skeleton) -> (Vec<usize>, bool) {
    let native = skeleton.bones.len() >= NATIVE_PLAYER_NAMES.len()
        && skeleton
            .bones
            .iter()
            .zip(NATIVE_PLAYER_NAMES)
            .all(|(b, name)| b.name == name);
    let map = skeleton
        .bones
        .iter()
        .enumerate()
        .map(|(index, bone)| {
            if native {
                return NATIVE_MIRROR_BONES.get(index).copied().unwrap_or(index);
            }
            let counterpart = bone
                .name
                .strip_prefix('L')
                .map(|s| format!("R{s}"))
                .or_else(|| bone.name.strip_prefix('R').map(|s| format!("L{s}")));
            counterpart
                .and_then(|name| skeleton.bones.iter().position(|b| b.name == name))
                .unwrap_or(index)
        })
        .collect();
    (map, native)
}

pub fn mirrored_rotation(bone: usize, q: Quat, mirrored: bool, native: bool) -> Quat {
    if !mirrored {
        return q;
    }
    if !native {
        return mirror_rotation(q);
    }
    match bone {
        // Type 1, raw 0019159c..001915fc: xyzw -> zwxy.
        0 => Quat::from_xyzw(q.z, q.w, q.x, q.y),
        // Type 4, raw 00191658..001916a4: xyzw -> xy(-z)(-w).
        1..=21 => Quat::from_xyzw(q.x, q.y, -q.z, -q.w),
        _ => q,
    }
}
