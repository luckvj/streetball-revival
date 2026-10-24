//! A whole, skinned character in a neutral form, for tools that export or
//! show players from any of the games (the player editor's exporter turns
//! it into glTF): meshes with joints and weights, one skeleton, and the
//! finished RGBA textures the meshes use.
//!
//! Conventions: Y up, game units (a V3 player is about 4.4 units per 78 in,
//! see [`Character::height_units`]), triangles counter-clockwise, joint
//! indices into [`Character::skeleton`], weights normalised. The skeleton's
//! inverse bind matrices are column-major and map model space to bone space
//! in the bind pose the vertices are stored in.

use crate::{
    eagl::{Bone, Skeleton},
    ssh,
};

/// One drawable part: a model's meshes for one texture.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    /// Where it came from, e.g. `Tops_M_Jersey/tops`.
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Up to four bones per vertex (indices into the skeleton) and their
    /// weights; unused entries weigh 0. One per position.
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    /// Index into [`Character::textures`]; `None` for an untextured part.
    pub texture: Option<usize>,
}

/// A finished texture: every layer the game composes for it already drawn.
#[derive(Clone, Debug)]
pub struct Texture {
    /// The model's texture slot, e.g. `head` or `tops`.
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA8, top row first.
    pub rgba: Vec<u8>,
    /// Whether any pixel is less than fully opaque (draw with alpha).
    pub alpha: bool,
}

impl Texture {
    pub fn from_ssh(name: impl Into<String>, t: &ssh::Texture) -> Self {
        Self {
            name: name.into(),
            width: t.width,
            height: t.height,
            alpha: t.has_alpha(),
            rgba: t.rgba.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Character {
    /// `<game>:<key>`, e.g. `v3gc:mario` (see docs/PLAYER_EDITOR.md).
    pub id: String,
    pub name: String,
    /// Height in inches from the game's table, when it has one.
    pub height_in: Option<f32>,
    pub skeleton: Skeleton,
    pub meshes: Vec<Mesh>,
    pub textures: Vec<Texture>,
    /// Things the assembler could not find or left out, for reports.
    pub notes: Vec<String>,
}

impl Character {
    /// Bounds of every vertex in the bind pose (min, max).
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut it = self.meshes.iter().flat_map(|m| m.positions.iter());
        let first = *it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| {
            (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            )
        }))
    }

    /// Height of the bind pose in game units.
    pub fn height_units(&self) -> f32 {
        self.bounds().map_or(0.0, |(lo, hi)| hi[1] - lo[1])
    }

    pub fn triangles(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }

    /// Each bone's bind-pose model matrix (column-major): the inverse of its
    /// inverse bind matrix.
    pub fn bind_matrices(&self) -> Vec<[f32; 16]> {
        self.skeleton
            .bones
            .iter()
            .map(|b| invert_affine(&b.inverse_bind))
            .collect()
    }

    /// The vertices posed by one model matrix per bone (column-major), as
    /// linear blend skinning does it: `sum(w * pose[j] * inverse_bind[j] * v)`.
    /// With [`Character::bind_matrices`] this gives the bind pose back.
    pub fn skin(&self, pose: &[[f32; 16]]) -> Vec<Vec<[f32; 3]>> {
        let skin: Vec<[f32; 16]> = self
            .skeleton
            .bones
            .iter()
            .zip(pose)
            .map(|(b, p)| mul(p, &b.inverse_bind))
            .collect();
        self.meshes
            .iter()
            .map(|m| {
                m.positions
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        if m.joints.is_empty() {
                            return *p;
                        }
                        let mut out = [0.0; 3];
                        for k in 0..4 {
                            let w = m.weights[i][k];
                            if w == 0.0 {
                                continue;
                            }
                            let Some(s) = skin.get(m.joints[i][k] as usize) else {
                                continue;
                            };
                            let q = transform(s, *p);
                            for c in 0..3 {
                                out[c] += w * q[c];
                            }
                        }
                        out
                    })
                    .collect()
            })
            .collect()
    }
}

/// Model matrix of every bone from local TRS (scale, rotation xyzw,
/// translation), parents first. Column-major.
pub fn pose_from_locals(
    bones: &[Bone],
    locals: &[([f32; 3], [f32; 4], [f32; 3])],
) -> Vec<[f32; 16]> {
    let mut out: Vec<[f32; 16]> = Vec::with_capacity(bones.len());
    for (i, b) in bones.iter().enumerate() {
        let (s, r, t) = locals
            .get(i)
            .copied()
            .unwrap_or((b.scale, b.rotation, b.translation));
        let local = trs(s, r, t);
        let m = match b.parent {
            Some(p) if p < out.len() => mul(&out[p], &local),
            _ => local,
        };
        out.push(m);
    }
    out
}

/// Largest difference between the bind pose the bones' local transforms
/// build and the one their inverse bind matrices hold.
pub fn bind_mismatch(skeleton: &Skeleton) -> f32 {
    let from_locals = pose_from_locals(&skeleton.bones, &[]);
    skeleton
        .bones
        .iter()
        .zip(&from_locals)
        .map(|(b, m)| {
            invert_affine(&b.inverse_bind)
                .iter()
                .zip(m)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0f32, f32::max)
        })
        .fold(0.0, f32::max)
}

/// Makes the skeleton rigid where its bind pose is not: each bone keeps the
/// position and orientation its inverse bind matrix gives it, without scale
/// (a mirrored axis is turned back), and its inverse bind matrix and local
/// transform are rewritten to match. The vertices' bind pose is unchanged.
///
/// V3's Peach needs it: her skeleton file pairs the stock locals with
/// scaled, partly mirrored bind matrices that no scale-rotate-translate
/// chain reproduces, which glTF and the game's rotation-only animations do
/// not need.
pub fn rigid_bind_pose(skeleton: &mut Skeleton) {
    let world: Vec<[f32; 16]> = skeleton
        .bones
        .iter()
        .map(|b| {
            let w = invert_affine(&b.inverse_bind);
            let (_, r, _) = decompose(&orthonormal(&w));
            trs([1.0; 3], r, [w[12], w[13], w[14]])
        })
        .collect();
    for (i, b) in skeleton.bones.iter_mut().enumerate() {
        let local = match b.parent {
            Some(p) => mul(&invert_affine(&world[p]), &world[i]),
            None => world[i],
        };
        let (_, r, t) = decompose(&local);
        b.scale = [1.0; 3];
        b.rotation = r;
        b.translation = t;
        b.inverse_bind = invert_affine(&world[i]);
    }
}

/// The rotation nearest an affine matrix's 3×3: columns normalised and made
/// orthogonal (Gram-Schmidt from x), right-handed.
fn orthonormal(m: &[f32; 16]) -> [f32; 16] {
    let col = |c: usize| [m[4 * c], m[4 * c + 1], m[4 * c + 2]];
    let norm = |v: [f32; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
        v.map(|x| x / l)
    };
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let x = norm(col(0));
    let y0 = col(1);
    let d = dot(y0, x);
    let y = norm([y0[0] - d * x[0], y0[1] - d * x[1], y0[2] - d * x[2]]);
    let z = cross(x, y);
    let mut out = *m;
    for (c, v) in [x, y, z].into_iter().enumerate() {
        out[4 * c..4 * c + 3].copy_from_slice(&v);
    }
    out
}

/// Scale, unit quaternion (x, y, z, w) and translation of a column-major
/// affine matrix without shear.
pub fn decompose(m: &[f32; 16]) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let col = |c: usize| [m[4 * c], m[4 * c + 1], m[4 * c + 2]];
    let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let s = [len(col(0)), len(col(1)), len(col(2))];
    let r: [[f32; 3]; 3] = std::array::from_fn(|c| {
        let v = col(c);
        v.map(|x| x / s[c].max(1e-12))
    });
    // r[c][row]: element (row, c).
    let e = |row: usize, c: usize| r[c][row];
    let trace = e(0, 0) + e(1, 1) + e(2, 2);
    let q = if trace > 0.0 {
        let k = (trace + 1.0).sqrt() * 2.0;
        [
            (e(2, 1) - e(1, 2)) / k,
            (e(0, 2) - e(2, 0)) / k,
            (e(1, 0) - e(0, 1)) / k,
            0.25 * k,
        ]
    } else if e(0, 0) > e(1, 1) && e(0, 0) > e(2, 2) {
        let k = (1.0 + e(0, 0) - e(1, 1) - e(2, 2)).sqrt() * 2.0;
        [
            0.25 * k,
            (e(0, 1) + e(1, 0)) / k,
            (e(0, 2) + e(2, 0)) / k,
            (e(2, 1) - e(1, 2)) / k,
        ]
    } else if e(1, 1) > e(2, 2) {
        let k = (1.0 + e(1, 1) - e(0, 0) - e(2, 2)).sqrt() * 2.0;
        [
            (e(0, 1) + e(1, 0)) / k,
            0.25 * k,
            (e(1, 2) + e(2, 1)) / k,
            (e(0, 2) - e(2, 0)) / k,
        ]
    } else {
        let k = (1.0 + e(2, 2) - e(0, 0) - e(1, 1)).sqrt() * 2.0;
        [
            (e(0, 2) + e(2, 0)) / k,
            (e(1, 2) + e(2, 1)) / k,
            0.25 * k,
            (e(1, 0) - e(0, 1)) / k,
        ]
    };
    let n = q.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-12);
    (s, q.map(|c| c / n), [m[12], m[13], m[14]])
}

/// Column-major 4×4 from scale, unit quaternion (x, y, z, w) and translation.
pub fn trs(s: [f32; 3], q: [f32; 4], t: [f32; 3]) -> [f32; 16] {
    let [x, y, z, w] = q;
    let r = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
        ],
        [
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
        ],
        [
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ];
    [
        r[0][0] * s[0],
        r[0][1] * s[0],
        r[0][2] * s[0],
        0.0,
        r[1][0] * s[1],
        r[1][1] * s[1],
        r[1][2] * s[1],
        0.0,
        r[2][0] * s[2],
        r[2][1] * s[2],
        r[2][2] * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ]
}

/// `a * b`, both column-major.
pub fn mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    std::array::from_fn(|i| {
        let (col, row) = (i / 4, i % 4);
        (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum()
    })
}

/// A point through a column-major affine matrix.
pub fn transform(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

/// Inverse of a column-major affine matrix (rotation/scale plus translation).
pub fn invert_affine(m: &[f32; 16]) -> [f32; 16] {
    let a = |r: usize, c: usize| m[c * 4 + r];
    let det = a(0, 0) * (a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1))
        - a(0, 1) * (a(1, 0) * a(2, 2) - a(1, 2) * a(2, 0))
        + a(0, 2) * (a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0));
    if det.abs() < 1e-12 {
        return *m;
    }
    let inv = |r: usize, c: usize| {
        // cofactor of (c, r) / det
        let (r0, r1) = match c {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let (c0, c1) = match r {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let minor = a(r0, c0) * a(r1, c1) - a(r0, c1) * a(r1, c0);
        let sign = if (r + c).is_multiple_of(2) { 1.0 } else { -1.0 };
        sign * minor / det
    };
    let mut out = [0.0; 16];
    for r in 0..3 {
        for c in 0..3 {
            out[c * 4 + r] = inv(r, c);
        }
    }
    for r in 0..3 {
        out[12 + r] = -(0..3).map(|c| out[c * 4 + r] * m[12 + c]).sum::<f32>();
    }
    out[15] = 1.0;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_inverse_round_trips() {
        let m = trs(
            [1.0, 2.0, 0.5],
            [0.0, 0.38268343, 0.0, 0.9238795],
            [3.0, -1.0, 2.0],
        );
        let i = invert_affine(&m);
        let id = mul(&m, &i);
        for (k, v) in id.iter().enumerate() {
            let want = if k % 5 == 0 { 1.0 } else { 0.0 };
            assert!((v - want).abs() < 1e-5, "{k}: {v}");
        }
        let p = transform(&m, [1.0, 1.0, 1.0]);
        let back = transform(&i, p);
        assert!(back.iter().all(|c| (c - 1.0).abs() < 1e-5));
        let (s, r, t) = decompose(&m);
        let again = trs(s, r, t);
        assert!(again.iter().zip(&m).all(|(a, b)| (a - b).abs() < 1e-5));
    }
}
