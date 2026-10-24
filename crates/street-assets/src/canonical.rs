//! Any game's character on Vol. 2's skeleton, so every game's animations
//! play on every character by bone name alone.
//!
//! [`rebind`] puts a [`Character`] (V3's 44-bone people, the Nintendo
//! All-Stars' 15-bone rig, the original game's 43 bones three times larger) on the
//! **canonical skeleton**: Vol. 2's 43 bones with Vol. 2's names, parents and
//! local rest rotations, so a Vol. 2 clip's rotation for a bone means the same
//! thing on it. Only the bones' positions are the character's own:
//!
//! - The body is scaled so its root (the hips) stands [`VOL2_HIP`] units over
//!   its soles, as on Vol. 2's skeleton, and moved so the root sits where
//!   Vol. 2's does. Every game's characters already face Vol. 2's way, in a
//!   T pose, y up.
//! - Each of Vol. 2's 22 body bones the character has ([`VOL2_BODY_BONES`],
//!   matched by name) sits at that bone's place. A body bone it lacks
//!   (Mario's collarbones, lower and mid torso, head, toes) sits between its
//!   parent and its nearest descendant the character has, in Vol. 2's
//!   proportions, or at Vol. 2's offset from its parent when none follows;
//!   the face bones sit at Vol. 2's offsets from the head. They carry no
//!   weight.
//! - Each vertex keeps its place (scaled and moved with the body) and its
//!   weights; triangles are wound to agree with their normals
//!   ([`wound_by_normals`]); a weight on a bone that is
//!   not one of the body bones (V3's face, shirt, shorts and necklace bones,
//!   the original game's face) moves to its nearest ancestor that is.
//!
//! The rest pose therefore draws exactly the character's own bind pose, and
//! a clip moves each body part the way it moves Vol. 2's. [`Baker`] turns
//! another rig's animation into Vol. 2's bone frames ([`crate::retarget`])
//! and scales its root motion the same way, so V3's and the original game's clips
//! play on Vol. 2's characters as well.

use crate::{
    Error, Result,
    character::{self, Character, Mesh},
    eagl::{Bone, Skeleton},
    gc_character::VOL2_BODY_BONES,
    retarget::{Retarget, qconj, qmul},
};

/// Height of Vol. 2's root over the soles at rest, in skeleton units (the
/// `Medium1Body` model's feet are at -4.05; every clip's root track stands
/// the hips this high).
pub const VOL2_HIP: f32 = 4.05;

/// A character on the canonical skeleton.
#[derive(Clone, Debug)]
pub struct Rebound {
    /// Skeleton: Vol. 2's bones; meshes in Vol. 2's model space (root at
    /// Vol. 2's root, soles near `-VOL2_HIP`); textures as they were.
    pub character: Character,
    /// Source units to canonical units.
    pub scale: f32,
    /// The root's height over the soles (canonical units).
    pub hip: f32,
    /// Soles to crown (canonical units).
    pub height: f32,
    /// How many of Vol. 2's 22 body bones the source has.
    pub matched: usize,
}

impl Rebound {
    /// Lowest vertex (y) in canonical units.
    pub fn floor(&self) -> f32 {
        self.character.bounds().map_or(-VOL2_HIP, |(lo, _)| lo[1])
    }
}

fn pos(m: &[f32; 16]) -> [f32; 3] {
    [m[12], m[13], m[14]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scaled(a: [f32; 3], k: f32) -> [f32; 3] {
    a.map(|v| v * k)
}

fn len(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// Rotate `v` by unit quaternion `q`.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let p = qmul(qmul(q, [v[0], v[1], v[2], 0.0]), qconj(q));
    [p[0], p[1], p[2]]
}

/// Index of the bone named `name` (case ignored).
fn find(s: &Skeleton, name: &str) -> Option<usize> {
    s.bones
        .iter()
        .position(|b| b.name.eq_ignore_ascii_case(name))
}

/// Whether `name` is one of Vol. 2's 22 body bones.
pub fn is_body_bone(name: &str) -> bool {
    VOL2_BODY_BONES.iter().any(|b| b.eq_ignore_ascii_case(name))
}

/// Vol. 2's rest pose from its local transforms, unit scale: world
/// rotations and positions per bone.
fn rest(vol2: &Skeleton) -> (Vec<[f32; 4]>, Vec<[f32; 3]>) {
    let mut rot: Vec<[f32; 4]> = Vec::with_capacity(vol2.bones.len());
    let mut at: Vec<[f32; 3]> = Vec::with_capacity(vol2.bones.len());
    for b in &vol2.bones {
        match b.parent.filter(|&p| p < rot.len()) {
            Some(p) => {
                at.push(add(at[p], rotate(rot[p], b.translation)));
                rot.push(qmul(rot[p], b.rotation));
            }
            None => {
                at.push(b.translation);
                rot.push(b.rotation);
            }
        }
    }
    (rot, at)
}

/// The source rig's hip height: its root's height over `floor` (the lowest
/// vertex of a character on it), for [`Baker`]'s root scale.
pub fn hip_height(rig: &Skeleton, floor: f32) -> f32 {
    let world = character::pose_from_locals(&rig.bones, &[]);
    let root = find(rig, "Root").unwrap_or(0);
    world.get(root).map_or(1.0, |m| m[13] - floor)
}

/// Put `c` on the canonical skeleton (see the module docs). `vol2` is Vol.
/// 2's player skeleton (`player::skeleton`).
pub fn rebind(c: &Character, vol2: &Skeleton) -> Result<Rebound> {
    let src = &c.skeleton;
    let src_world = c.bind_matrices();
    let src_pos: Vec<[f32; 3]> = src_world.iter().map(pos).collect();
    let (v_rot, v_pos) = rest(vol2);
    let n = vol2.bones.len();

    // Vol. 2's body bones the source has.
    let matched: Vec<Option<usize>> = vol2
        .bones
        .iter()
        .map(|b| {
            if is_body_bone(&b.name) {
                find(src, &b.name)
            } else {
                None
            }
        })
        .collect();
    let count = matched.iter().flatten().count();
    if count < 8 {
        return Err(Error::format(format!(
            "{}: only {count} of Vol. 2's body bones",
            c.id
        )));
    }
    let v_root = find(vol2, "Root").unwrap_or(0);
    let s_root = matched[v_root]
        .map(|i| src_pos[i])
        .or_else(|| {
            let l = find(src, "LThigh")?;
            let r = find(src, "RThigh")?;
            Some(scaled(add(src_pos[l], src_pos[r]), 0.5))
        })
        .ok_or_else(|| Error::format(format!("{}: no root or thighs", c.id)))?;
    let floor = c
        .bounds()
        .map(|(lo, _)| lo[1])
        .ok_or_else(|| Error::format(format!("{}: no vertices", c.id)))?;
    let hip = s_root[1] - floor;
    if hip <= 1e-3 {
        return Err(Error::format(format!("{}: root at the soles", c.id)));
    }
    let k = VOL2_HIP / hip;
    let origin = v_pos[v_root];
    let map = |p: [f32; 3]| add(scaled(sub(p, s_root), k), origin);

    // Canonical positions, parents first.
    // The nearest matched descendant of `j` through unmatched bones, and the
    // path length to it in Vol. 2's rest pose.
    fn descendant(
        vol2: &Skeleton,
        v_pos: &[[f32; 3]],
        matched: &[Option<usize>],
        j: usize,
    ) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for c in (0..vol2.bones.len()).filter(|&c| vol2.bones[c].parent == Some(j)) {
            let step = len(sub(v_pos[c], v_pos[j]));
            let found = if matched[c].is_some() {
                Some((c, step))
            } else if is_body_bone(&vol2.bones[c].name) {
                descendant(vol2, v_pos, matched, c).map(|(d, l)| (d, l + step))
            } else {
                None
            };
            if let Some(f) = found
                && best.is_none_or(|b| f.1 < b.1)
            {
                best = Some(f);
            }
        }
        best
    }
    let mut at: Vec<[f32; 3]> = Vec::with_capacity(n);
    for (j, b) in vol2.bones.iter().enumerate() {
        let p = b.parent.filter(|&p| p < at.len());
        let here = if j == v_root {
            origin
        } else if let Some(i) = matched[j] {
            map(src_pos[i])
        } else if let Some(p) = p {
            let own = len(sub(v_pos[j], v_pos[p]));
            match descendant(vol2, &v_pos, &matched, j)
                .filter(|_| is_body_bone(&b.name))
                .and_then(|(d, rest)| Some((map(src_pos[matched[d]?]), rest)))
            {
                Some((target, rest)) if own + rest > 1e-6 => {
                    let t = own / (own + rest);
                    add(at[p], scaled(sub(target, at[p]), t))
                }
                _ => add(at[p], scaled(sub(v_pos[j], v_pos[p]), k)),
            }
        } else {
            map(v_pos[j])
        };
        at.push(here);
    }

    // Bones: Vol. 2's rotations, translations to those positions.
    let bones: Vec<Bone> = vol2
        .bones
        .iter()
        .enumerate()
        .map(|(j, b)| {
            let translation = match b.parent.filter(|&p| p < j) {
                Some(p) => rotate(qconj(v_rot[p]), sub(at[j], at[p])),
                None => at[j],
            };
            Bone {
                name: b.name.clone(),
                parent: b.parent,
                scale: [1.0; 3],
                rotation: b.rotation,
                translation,
                id: b.id,
                inverse_bind: character::invert_affine(&character::trs([1.0; 3], v_rot[j], at[j])),
            }
        })
        .collect();

    // Source bone -> canonical bone: by name for body bones, else the
    // nearest ancestor that maps.
    let by_name: Vec<Option<usize>> = src
        .bones
        .iter()
        .map(|b| {
            if is_body_bone(&b.name) {
                find(vol2, &b.name)
            } else {
                None
            }
        })
        .collect();
    let remap: Vec<u16> = (0..src.bones.len())
        .map(|i| {
            let mut at = Some(i);
            let mut guard = 0;
            while let Some(k) = at {
                if let Some(j) = by_name[k] {
                    return j as u16;
                }
                at = src.bones[k].parent.filter(|&p| p != k);
                guard += 1;
                if guard > 256 {
                    break;
                }
            }
            v_root as u16
        })
        .collect();

    let meshes: Vec<Mesh> = c
        .meshes
        .iter()
        .map(|m| {
            let (joints, weights) = m
                .joints
                .iter()
                .zip(&m.weights)
                .map(|(j, w)| merge(j.map(|b| remap.get(b as usize).copied().unwrap_or(0)), *w))
                .unzip();
            let positions: Vec<[f32; 3]> = m.positions.iter().map(|&p| map(p)).collect();
            let indices = wound_by_normals(&positions, &m.normals, &m.indices);
            Mesh {
                name: m.name.clone(),
                positions,
                normals: m.normals.clone(),
                uvs: m.uvs.clone(),
                joints,
                weights,
                indices,
                texture: m.texture,
            }
        })
        .collect();
    let character = Character {
        id: c.id.clone(),
        name: c.name.clone(),
        height_in: c.height_in,
        skeleton: Skeleton { bones },
        meshes,
        textures: c.textures.clone(),
        notes: c.notes.clone(),
    };
    let (lo, hi) = character.bounds().unwrap_or(([0.0; 3], [0.0; 3]));
    Ok(Rebound {
        hip: origin[1] - lo[1],
        height: hi[1] - lo[1],
        scale: k,
        matched: count,
        character,
    })
}

/// Triangles turned to agree with their vertex normals (counter-clockwise
/// seen from the side the normals face), as Vol. 2's decoder winds them:
/// the original game's strips alternate their winding and some of V3's batches are
/// mirrored, which renderers that light back faces differently show as
/// dark patches.
pub fn wound_by_normals(positions: &[[f32; 3]], normals: &[[f32; 3]], indices: &[u32]) -> Vec<u32> {
    let mut out = indices.to_vec();
    if normals.len() != positions.len() {
        return out;
    }
    for t in out.as_chunks_mut::<3>().0 {
        let [a, b, c] = t.map(|i| i as usize);
        if a >= positions.len() || b >= positions.len() || c >= positions.len() {
            continue;
        }
        let (e1, e2) = (
            sub(positions[b], positions[a]),
            sub(positions[c], positions[a]),
        );
        let face = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let n = add(add(normals[a], normals[b]), normals[c]);
        if face[0] * n[0] + face[1] * n[1] + face[2] * n[2] < 0.0 {
            t.swap(1, 2);
        }
    }
    out
}

/// Joints with repeats folded together (weights added), heaviest first.
fn merge(j: [u16; 4], w: [f32; 4]) -> ([u16; 4], [f32; 4]) {
    let mut out: Vec<(u16, f32)> = Vec::with_capacity(4);
    for k in 0..4 {
        if w[k] <= 0.0 {
            continue;
        }
        match out.iter_mut().find(|(b, _)| *b == j[k]) {
            Some(e) => e.1 += w[k],
            None => out.push((j[k], w[k])),
        }
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut joints = [0u16; 4];
    let mut weights = [0.0f32; 4];
    for (k, (b, x)) in out.into_iter().enumerate() {
        joints[k] = b;
        weights[k] = x;
    }
    if weights[0] == 0.0 {
        weights[0] = 1.0;
    }
    (joints, weights)
}

/// One clip baked onto Vol. 2's skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Baked {
    /// `(Vol. 2 bone, local rotation per frame)` for the 22 body bones.
    pub rotations: Vec<(usize, Vec<[f32; 4]>)>,
    /// The root's translation per frame, Vol. 2 skeleton units, floor at 0.
    pub root: Vec<[f32; 3]>,
}

/// Turns one rig's animations into Vol. 2's bone frames.
pub struct Baker {
    retarget: Retarget,
    rig_rest: Vec<[f32; 4]>,
    rig_len: usize,
    body: Vec<usize>,
    /// Source root units to Vol. 2 units.
    pub root_scale: f32,
}

impl Baker {
    /// `rig` is the skeleton the clips' channels index; `rig_hip` its root's
    /// height over the soles at rest, in the clips' root units
    /// ([`hip_height`]).
    pub fn new(rig: &Skeleton, vol2: &Skeleton, rig_hip: f32) -> Self {
        Self {
            retarget: Retarget::new(rig, vol2),
            rig_rest: rig.bones.iter().map(|b| b.rotation).collect(),
            rig_len: rig.bones.len(),
            body: (0..vol2.bones.len())
                .filter(|&j| is_body_bone(&vol2.bones[j].name))
                .collect(),
            root_scale: if rig_hip > 1e-3 {
                VOL2_HIP / rig_hip
            } else {
                1.0
            },
        }
    }

    /// Bake: `bones[c]` is the rig bone channel `c` drives, `frames[f][c]`
    /// its local rotation; `root` the root's translation per frame in the
    /// rig's units (floor at 0), if the clip has one.
    pub fn bake(&self, bones: &[u8], frames: &[Vec<[f32; 4]>], root: Option<&[[f32; 3]]>) -> Baked {
        let mut tracks: Vec<Vec<[f32; 4]>> =
            vec![Vec::with_capacity(frames.len()); self.body.len()];
        for frame in frames {
            let mut rotations = self.rig_rest.clone();
            for (&b, &q) in bones.iter().zip(frame) {
                if let Some(r) = rotations.get_mut(b as usize) {
                    *r = q;
                }
            }
            rotations.truncate(self.rig_len);
            let locals = self.retarget.pose(&rotations);
            for (t, &j) in tracks.iter_mut().zip(&self.body) {
                t.push(locals[j].1);
            }
        }
        let root = root
            .map(|r| r.iter().map(|t| t.map(|v| v * self.root_scale)).collect())
            .unwrap_or_default();
        Baked {
            rotations: self.body.iter().copied().zip(tracks).collect(),
            root,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bone(name: &str, parent: Option<usize>, t: [f32; 3]) -> Bone {
        Bone {
            name: name.into(),
            parent,
            scale: [1.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            translation: t,
            id: 0,
            inverse_bind: [0.0; 16],
        }
    }

    /// A stick figure with Vol. 2's body bones, `s` times as large, the root
    /// `lift * s` over the origin.
    fn figure(s: f32, lift: f32, names: &[&str]) -> Skeleton {
        let all: [(&str, Option<&str>, [f32; 3]); 22] = [
            ("Root", None, [0.0, lift, 0.0]),
            ("LThigh", Some("Root"), [0.3, -0.3, 0.0]),
            ("LLowLeg", Some("LThigh"), [0.0, -1.7, 0.0]),
            ("LFoot", Some("LLowLeg"), [0.0, -1.6, 0.0]),
            ("LToe", Some("LFoot"), [0.0, -0.35, 0.6]),
            ("RThigh", Some("Root"), [-0.3, -0.3, 0.0]),
            ("RLowLeg", Some("RThigh"), [0.0, -1.7, 0.0]),
            ("RFoot", Some("RLowLeg"), [0.0, -1.6, 0.0]),
            ("RToe", Some("RFoot"), [0.0, -0.35, 0.6]),
            ("LowerTorso", Some("Root"), [0.0, 0.0, 0.0]),
            ("MidTorso", Some("LowerTorso"), [0.0, 0.5, 0.0]),
            ("UpperTorso", Some("MidTorso"), [0.0, 0.5, 0.0]),
            ("LCollarBone", Some("UpperTorso"), [0.1, 0.3, 0.0]),
            ("LUpperArm", Some("LCollarBone"), [0.5, 0.0, 0.0]),
            ("LLowArm", Some("LUpperArm"), [1.1, 0.0, 0.0]),
            ("LHand", Some("LLowArm"), [1.0, 0.0, 0.0]),
            ("RCollarBone", Some("UpperTorso"), [-0.1, 0.3, 0.0]),
            ("RUpperArm", Some("RCollarBone"), [-0.5, 0.0, 0.0]),
            ("RLowArm", Some("RUpperArm"), [-1.1, 0.0, 0.0]),
            ("RHand", Some("RLowArm"), [-1.0, 0.0, 0.0]),
            ("Neck", Some("UpperTorso"), [0.0, 0.5, 0.0]),
            ("Head", Some("Neck"), [0.0, 0.45, 0.0]),
        ];
        let mut bones: Vec<Bone> = Vec::new();
        for (name, parent, t) in all {
            if !names.is_empty() && !names.contains(&name) {
                continue;
            }
            // A missing parent hands the bone to the nearest ancestor kept,
            // with the offsets added up.
            let mut t = scaled(t, s);
            let mut parent = parent;
            let mut p = None;
            while let Some(name) = parent {
                if let Some(i) = bones.iter().position(|b| b.name == name) {
                    p = Some(i);
                    break;
                }
                let (_, up, more) = all.iter().find(|(n, _, _)| *n == name).unwrap();
                t = add(t, scaled(*more, s));
                parent = *up;
            }
            bones.push(bone(name, p, t));
        }
        let mut s = Skeleton { bones };
        let world = character::pose_from_locals(&s.bones, &[]);
        for (b, m) in s.bones.iter_mut().zip(&world) {
            b.inverse_bind = character::invert_affine(m);
        }
        s
    }

    fn body(s: &Skeleton, floor: f32) -> Character {
        // One vertex per bone, on it, and one at the soles.
        let world = character::pose_from_locals(&s.bones, &[]);
        let mut positions: Vec<[f32; 3]> = world.iter().map(pos).collect();
        positions.push([0.0, floor, 0.0]);
        let n = positions.len();
        let mut joints: Vec<[u16; 4]> = (0..s.bones.len()).map(|i| [i as u16, 0, 0, 0]).collect();
        joints.push([0; 4]);
        Character {
            id: "test:a".into(),
            name: "A".into(),
            height_in: Some(70.0),
            skeleton: s.clone(),
            meshes: vec![Mesh {
                name: "m".into(),
                positions,
                normals: vec![[0.0, 1.0, 0.0]; n],
                uvs: vec![[0.0; 2]; n],
                joints,
                weights: vec![[1.0, 0.0, 0.0, 0.0]; n],
                indices: vec![0, 1, 2],
                texture: None,
            }],
            textures: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn a_larger_rig_lands_on_vol2_sizes_and_keeps_its_rest_pose() {
        let vol2 = figure(1.0, 0.0, &[]);
        let big = figure(3.0, 11.0 / 3.0, &[]);
        // Soles 3.75 under the root at Vol. 2's size.
        let c = body(&big, 11.0 - 3.0 * VOL2_HIP);
        let r = rebind(&c, &vol2).unwrap();
        assert!((r.scale - 1.0 / 3.0).abs() < 1e-4, "{}", r.scale);
        assert!((r.hip - VOL2_HIP).abs() < 1e-3);
        assert_eq!(r.matched, 22);
        let s = &r.character.skeleton;
        let world = character::pose_from_locals(&s.bones, &[]);
        let want = character::pose_from_locals(&vol2.bones, &[]);
        for (a, b) in world.iter().zip(&want) {
            for k in 12..15 {
                assert!((a[k] - b[k]).abs() < 1e-3, "{a:?} {b:?}");
            }
        }
        // The rest pose draws the bind pose.
        let posed = r.character.skin(&r.character.bind_matrices());
        for (p, q) in posed[0].iter().zip(&r.character.meshes[0].positions) {
            assert!((0..3).all(|k| (p[k] - q[k]).abs() < 1e-3));
        }
    }

    #[test]
    fn missing_bones_sit_between_their_neighbours() {
        let vol2 = figure(1.0, 0.0, &[]);
        // Mario-like: no lower or mid torso, no collarbones, no head or toes.
        let names = [
            "Root",
            "LThigh",
            "LLowLeg",
            "LFoot",
            "RThigh",
            "RLowLeg",
            "RFoot",
            "UpperTorso",
            "LUpperArm",
            "LLowArm",
            "LHand",
            "RUpperArm",
            "RLowArm",
            "RHand",
            "Neck",
        ];
        let small = figure(0.5, 0.0, &names);
        let c = body(&small, -0.5 * VOL2_HIP);
        let r = rebind(&c, &vol2).unwrap();
        assert_eq!(r.matched, 15);
        let s = &r.character.skeleton;
        let world = character::pose_from_locals(&s.bones, &[]);
        let at = |n: &str| pos(&world[find(s, n).unwrap()]);
        let collar = at("LCollarBone");
        let (torso, arm) = (at("UpperTorso"), at("LUpperArm"));
        assert!(collar[0] > torso[0] && collar[0] < arm[0], "{collar:?}");
        // Upper arm where the source had it.
        assert!((arm[0] - 0.6 * 2.0 * 0.5).abs() < 1e-3, "{arm:?}");
        // Weights stay on the bones of the same name.
        let m = &r.character.meshes[0];
        let neck = find(&small, "Neck").unwrap();
        assert_eq!(m.joints[neck][0] as usize, find(s, "Neck").unwrap());
    }

    #[test]
    fn triangles_turn_to_face_their_normals() {
        let p = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let n = [[0.0, 0.0, 1.0]; 3];
        assert_eq!(wound_by_normals(&p, &n, &[0, 1, 2]), [0, 1, 2]);
        assert_eq!(wound_by_normals(&p, &n, &[0, 2, 1]), [0, 1, 2]);
    }

    #[test]
    fn baking_a_rigs_rest_gives_vol2_rest_and_scales_root_motion() {
        let vol2 = figure(1.0, 0.0, &[]);
        let big = figure(3.0, 0.0, &[]);
        let baker = Baker::new(&big, &vol2, 3.0 * VOL2_HIP);
        let rest: Vec<[f32; 4]> = big.bones.iter().map(|b| b.rotation).collect();
        let bones: Vec<u8> = (0..big.bones.len() as u8).collect();
        let b = baker.bake(&bones, &[rest], Some(&[[0.0, 3.0 * VOL2_HIP, 3.0]]));
        assert_eq!(b.rotations.len(), 22);
        for (j, keys) in &b.rotations {
            let d: f32 = (0..4)
                .map(|k| keys[0][k] * vol2.bones[*j].rotation[k])
                .sum();
            assert!((d.abs() - 1.0).abs() < 1e-4);
        }
        assert!((b.root[0][1] - VOL2_HIP).abs() < 1e-4);
        assert!((b.root[0][2] - 1.0).abs() < 1e-4);
    }
}
