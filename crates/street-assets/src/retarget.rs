//! Playing one skeleton's animation on another: bones are matched by name
//! (case ignored, plus [`ALIASES`]) and each matched bone takes the source
//! bone's rotation relative to its rest pose, in model space. That keeps a
//! pose right when the two rigs hold their bones differently at rest (V3's
//! thighs are turned 180° about their z axis against Vol. 2's) or differ in
//! hierarchy (Mario's arms hang from `UpperTorso` with no collarbones).
//!
//! How the families compare (see docs/FORMATS.md): Vol. 2's 43-bone player
//! skeleton and V3's 44-bone one share all 22 body bones with the same rest
//! positions; V3's legs are turned 180° and its collarbones tilt by under
//! 3°. V3's Nintendo skeletons (15 bones) name a subset of them. league
//! Street's 43 bones are Vol. 2's names and order with other rest frames.

use crate::eagl::Skeleton;

/// Extra name pairs (source, target) beyond equal names.
pub const ALIASES: &[(&str, &str)] = &[
    // Vol. 2's face rig -> V3's.
    ("lowerlipL", "LLowerLip"),
    ("lowerlipR", "RLowerLip"),
    ("upperlipL", "LUpperLip"),
    ("upperlipR", "RUpperLip"),
];

type Quat = [f32; 4];

/// A local pose: (scale, rotation x y z w, translation) per bone.
pub type Locals = Vec<([f32; 3], Quat, [f32; 3])>;

pub struct Retarget {
    /// For each target bone, the source bone it follows.
    pub source_of: Vec<Option<usize>>,
    source_parents: Vec<Option<usize>>,
    target_parents: Vec<Option<usize>>,
    source_rest: Vec<Quat>,
    target_rest: Vec<Quat>,
    target_locals: Locals,
    /// Target root height over source root height, for root motion.
    pub root_scale: f32,
}

impl Retarget {
    pub fn new(source: &Skeleton, target: &Skeleton) -> Self {
        let source_of = target
            .bones
            .iter()
            .map(|t| {
                source
                    .bones
                    .iter()
                    .position(|s| s.name.eq_ignore_ascii_case(&t.name))
                    .or_else(|| {
                        let alias = ALIASES
                            .iter()
                            .find(|(_, b)| b.eq_ignore_ascii_case(&t.name))?
                            .0;
                        source
                            .bones
                            .iter()
                            .position(|s| s.name.eq_ignore_ascii_case(alias))
                    })
            })
            .collect();
        let rest = |s: &Skeleton| {
            let mut out: Vec<Quat> = Vec::with_capacity(s.bones.len());
            for b in &s.bones {
                let q = match b.parent {
                    Some(p) => qmul(out[p], b.rotation),
                    None => b.rotation,
                };
                out.push(q);
            }
            out
        };
        let leg = |s: &Skeleton| {
            s.bones
                .iter()
                .position(|b| b.name.eq_ignore_ascii_case("LThigh"))
                .map(|i| {
                    let w = crate::character::invert_affine(&s.bones[i].inverse_bind);
                    w[13]
                })
        };
        let root_scale = match (leg(source), leg(target)) {
            (Some(a), Some(b)) if a.abs() > 1e-3 => (b / a).abs(),
            _ => 1.0,
        };
        Self {
            source_of,
            source_parents: source.bones.iter().map(|b| b.parent).collect(),
            target_parents: target.bones.iter().map(|b| b.parent).collect(),
            source_rest: rest(source),
            target_rest: rest(target),
            target_locals: target
                .bones
                .iter()
                .map(|b| (b.scale, b.rotation, b.translation))
                .collect(),
            root_scale,
        }
    }

    /// The target's local pose for a source pose given as one local
    /// rotation per source bone. Translations stay the target's own.
    pub fn pose(&self, source_rotations: &[Quat]) -> Locals {
        let mut source_world: Vec<Quat> = Vec::with_capacity(source_rotations.len());
        for (i, &r) in source_rotations.iter().enumerate() {
            let q = match self.source_parents.get(i).copied().flatten() {
                Some(p) if p < source_world.len() => qmul(source_world[p], r),
                _ => r,
            };
            source_world.push(q);
        }
        let mut out = self.target_locals.clone();
        let mut world: Vec<Quat> = Vec::with_capacity(out.len());
        #[allow(clippy::needless_range_loop)]
        for i in 0..out.len() {
            let parent = self.target_parents[i].map(|p| world[p]);
            let w = match self.source_of[i].filter(|&s| s < source_world.len()) {
                Some(s) => qmul(
                    source_world[s],
                    qmul(qconj(self.source_rest[s]), self.target_rest[i]),
                ),
                None => match parent {
                    Some(p) => qmul(p, out[i].1),
                    None => out[i].1,
                },
            };
            out[i].1 = normalize(match parent {
                Some(p) => qmul(qconj(p), w),
                None => w,
            });
            world.push(w);
        }
        out
    }
}

pub fn qmul(a: Quat, b: Quat) -> Quat {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn qconj(q: Quat) -> Quat {
    [-q[0], -q[1], -q[2], q[3]]
}

fn normalize(q: Quat) -> Quat {
    let n = q.iter().map(|c| c * c).sum::<f32>().sqrt();
    if n > 1e-12 {
        q.map(|c| c / n)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eagl::Bone;

    fn bone(name: &str, parent: Option<usize>, rotation: Quat) -> Bone {
        Bone {
            name: name.into(),
            parent,
            scale: [1.0; 3],
            rotation,
            translation: [1.0, 0.0, 0.0],
            id: 0,
            inverse_bind: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    #[test]
    fn rest_maps_to_rest_and_rolled_bones_follow() {
        let roll = [1.0, 0.0, 0.0, 0.0]; // 180 degrees about x
        let source = Skeleton {
            bones: vec![
                bone("Root", None, [0.0, 0.0, 0.0, 1.0]),
                bone("LThigh", Some(0), [0.0, 0.0, 0.0, 1.0]),
            ],
        };
        let target = Skeleton {
            bones: vec![
                bone("Root", None, [0.0, 0.0, 0.0, 1.0]),
                bone("lthigh", Some(0), roll),
            ],
        };
        let r = Retarget::new(&source, &target);
        let rest = r.pose(&[[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]]);
        let d: f32 = (0..4).map(|k| rest[1].1[k] * roll[k]).sum();
        assert!((d.abs() - 1.0).abs() < 1e-5);
        // A 90-degree bend about z on the source bends the target the same
        // way in model space.
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let bend = [0.0, 0.0, h, h];
        let posed = r.pose(&[[0.0, 0.0, 0.0, 1.0], bend]);
        let want = qmul(bend, roll);
        let d: f32 = (0..4).map(|k| posed[1].1[k] * want[k]).sum();
        assert!((d.abs() - 1.0).abs() < 1e-5);
    }
}
