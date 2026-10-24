//! Vol. 2's animations as neutral [`Clip`]s: the banks in `/ANIM`, each clip
//! a rotation track per animated bone and the root's floor-relative
//! translation, at 30 frames per second.
//!
//! A `@mirror` suffix names a clip played as its left-handed twin: the
//! animation records the game marks mirrored (`0x3de8a0 + 24`) play the
//! named clip through `FUN_001914c8`'s bone swap and quaternion reflection
//! (`street-render`'s `mirrored_rotation`), which is baked here.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek};

use street_assets::{anim, iso::Iso};

use crate::model::Clip;

/// Frames per second the game's animations are authored at.
pub const FPS: f32 = 30.0;

/// Suffix of a baked mirrored clip.
pub const MIRROR: &str = "@mirror";

/// `FUN_001914c8`'s left/right pairs over the first 22 bones.
const MIRROR_BONES: [usize; 22] = [
    0, 5, 6, 7, 8, 1, 2, 3, 4, 9, 10, 11, 16, 17, 18, 19, 12, 13, 14, 15, 20, 21,
];

/// Every bank, decoded once; clips are read from it by name.
pub struct Banks {
    banks: Vec<(String, anim::Bank)>,
    /// Clip name -> bank.
    index: BTreeMap<String, usize>,
}

impl Banks {
    pub fn load<R: Read + Seek>(iso: &mut Iso<R>) -> street_assets::Result<Self> {
        let mut banks = Vec::new();
        let mut index = BTreeMap::new();
        for path in anim::BANKS {
            let bank = anim::load(iso, path)?;
            for name in bank.animations() {
                index.entry(name.to_owned()).or_insert(banks.len());
            }
            let short = path
                .rsplit('/')
                .next()
                .unwrap_or(path)
                .trim_end_matches(".BIN");
            banks.push((short.to_owned(), bank));
        }
        Ok(Self { banks, index })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.index.keys().map(String::as_str)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.index
            .contains_key(name.strip_suffix(MIRROR).unwrap_or(name))
    }

    /// The bank a clip is in (`ANIBIG`, ...).
    pub fn bank_of(&self, name: &str) -> Option<&str> {
        let i = *self.index.get(name.strip_suffix(MIRROR).unwrap_or(name))?;
        Some(&self.banks[i].0)
    }

    /// Clips in bank order: every clip of the bank at `path`'s short name.
    pub fn bank_clips(&self, bank: &str) -> Vec<&str> {
        self.index
            .iter()
            .filter(|&(_, &i)| self.banks[i].0 == bank)
            .map(|(n, _)| n.as_str())
            .collect()
    }

    pub fn bank_names(&self) -> impl Iterator<Item = &str> {
        self.banks.iter().map(|(n, _)| n.as_str())
    }

    /// The clip `name` (or its mirrored twin, with [`MIRROR`]); `None` when
    /// no bank has it or its tracks use a channel type not decoded.
    pub fn clip(&self, name: &str) -> street_assets::Result<Option<Clip>> {
        let (base, mirrored) = match name.strip_suffix(MIRROR) {
            Some(b) => (b, true),
            None => (name, false),
        };
        let Some(&i) = self.index.get(base) else {
            return Ok(None);
        };
        let bank = &self.banks[i].1;
        let Some(rotations) = bank.rotations(base)? else {
            return Ok(None);
        };
        let frames = rotations.frames.len();
        let mut tracks: BTreeMap<usize, Vec<[f32; 4]>> = BTreeMap::new();
        for (c, &bone) in rotations.bones.iter().enumerate() {
            let bone = usize::from(bone);
            let target = if mirrored {
                MIRROR_BONES.get(bone).copied().unwrap_or(bone)
            } else {
                bone
            };
            let keys = rotations
                .frames
                .iter()
                .map(|f| {
                    let q = f[c];
                    if mirrored {
                        mirror_rotation(target, q)
                    } else {
                        q
                    }
                })
                .collect();
            tracks.insert(target, keys);
        }
        let translations = match bank.root_motion(base)? {
            Some(root) if !root.frames.is_empty() => vec![(
                0,
                root.frames
                    .iter()
                    .map(|t| {
                        let mut t = t.map(|v| v * anim::TRANSLATION_SCALE);
                        if mirrored {
                            t[0] = -t[0];
                        }
                        t
                    })
                    .collect(),
            )],
            _ => Vec::new(),
        };
        Ok(Some(Clip {
            name: name.to_owned(),
            fps: FPS,
            frames,
            rotations: tracks.into_iter().collect(),
            translations,
        }))
    }

    /// Decode `names` (unknown or undecodable ones are left out, and
    /// returned second).
    pub fn clips(&self, names: &BTreeSet<String>) -> (Vec<Clip>, Vec<String>) {
        let mut clips = Vec::new();
        let mut missing = Vec::new();
        for name in names {
            match self.clip(name) {
                Ok(Some(c)) => clips.push(c),
                _ => missing.push(name.clone()),
            }
        }
        (clips, missing)
    }
}

/// `street-render`'s native `mirrored_rotation`: the root's components
/// permuted (raw `0019159c`), the body's reflected in z (raw `00191658`).
fn mirror_rotation(bone: usize, q: [f32; 4]) -> [f32; 4] {
    match bone {
        0 => [q[2], q[3], q[0], q[1]],
        1..=21 => [q[0], q[1], -q[2], -q[3]],
        _ => q,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirroring_twice_is_the_identity() {
        let q = [0.1, 0.2, 0.3, 0.9];
        for bone in [0, 5, 21, 30] {
            assert_eq!(mirror_rotation(bone, mirror_rotation(bone, q)), q);
        }
        for (i, &m) in MIRROR_BONES.iter().enumerate() {
            assert_eq!(MIRROR_BONES[m], i);
        }
    }
}
