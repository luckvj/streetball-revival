//! The nets: a cloth of 7 rings of 10 points hanging from each rim, the way
//! the game builds it (0x31af58 sets it up, 0x31b2d8 places the points,
//! 0x31a900 links them, 0x31ac88 steps it). Ring `r`, point `i` rests at
//! angle `i * 2pi / 10`, radius `RADII[r]`, `DROPS[r]` below the rim; the top
//! ring is fixed to the rim. The texture is the court's `<court>n.ssh`
//! (`net1`), else the shared `net.ssh`.
//!
//! Each frame ([`step_net`], `FUN_0031a230` at 1/60) the top ring is put on
//! the rim, the other points take a Verlet step under gravity, one pass
//! over the 120 links pulls them back to their rest lengths, and the ball
//! ([`NetCollider`], attached by the hoop at `0x317190` and moved every
//! update at `0x317508`) pushes them out to 1.2 times its radius.

use bevy::{
    asset::RenderAssetUsages, camera::visibility::NoFrustumCulling, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use street_assets::ssh;

use crate::convert;

/// Ring radii and drops below the rim, from 0x31b2d8.
pub const RADII: [f32; 7] = [0.77, 0.5, 0.42, 0.38, 0.36, 0.33, 0.33];
pub const DROPS: [f32; 7] = [-0.1, -0.55, -0.83, -0.965, -1.1, -1.2, -1.3];
const AROUND: usize = 10;
const RINGS: usize = 7;

/// `FUN_0031a3e8`: the free points' acceleration (`0xc1f00000`).
const GRAVITY: f32 = -30.0;
/// `FUN_0031a230`'s step (`0x3c888889`), the same every frame.
const DT: f32 = 1.0 / 60.0;
/// `FUN_0031a340`: `new = P × 1.97 − prev × 0.97 + a·dt²`.
const KEEP: f32 = 0.97;
/// `FUN_0031a1d8`: the ball's sphere is its radius × 1.2 (`0x3f99999a`).
const BALL_SCALE: f32 = 1.2;

/// Puts its entity's position in the nets' way (give it to the ball).
#[derive(Component, Clone, Copy)]
pub struct NetCollider {
    pub radius: f32,
}

#[derive(Component)]
pub struct Net {
    /// The rim's move as it gives (`street_sim::rim`), which carries the
    /// top ring (`FUN_0031a5f8` puts it on the rim's matrix).
    pub rim: crate::court::BoneMove,
    points: Vec<Vec3>,
    previous: Vec<Vec3>,
    rest: Vec<Vec3>,
    links: Vec<(usize, usize, f32)>,
    mesh: Handle<Mesh>,
}

impl Net {
    /// The rim it hangs from: 0 the -x hoop's, 1 the +x one's
    /// (`street_sim::rim`).
    pub fn side(&self) -> usize {
        usize::from(self.rest[0].x >= 0.0)
    }
}

fn index(ring: usize, i: usize) -> usize {
    ring * AROUND + i % AROUND
}

/// The rest shape around a rim centre, in world space.
pub fn rest_points(rim: Vec3) -> Vec<Vec3> {
    let mut out = Vec::with_capacity(RINGS * AROUND);
    for r in 0..RINGS {
        for i in 0..AROUND {
            let a = i as f32 * std::f32::consts::TAU / AROUND as f32;
            out.push(rim + Vec3::new(RADII[r] * a.sin(), DROPS[r], RADII[r] * a.cos()));
        }
    }
    out
}

/// Spawns the net under the rim centred at `rim` (world space).
pub fn spawn(
    commands: &mut Commands,
    rim: Vec3,
    texture: Option<&ssh::Texture>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Entity {
    let rest = rest_points(rim);
    let links = links(&rest);
    // Vertices: one more column than points, so the texture can wrap.
    let mut uvs = Vec::new();
    for r in 0..RINGS {
        for i in 0..=AROUND {
            uvs.push([i as f32, r as f32]);
        }
    }
    let mut indices = Vec::new();
    let col = AROUND as u32 + 1;
    for r in 0..RINGS as u32 - 1 {
        for i in 0..AROUND as u32 {
            let (a, b, c, d) = (
                r * col + i,
                r * col + i + 1,
                (r + 1) * col + i,
                (r + 1) * col + i + 1,
            );
            indices.extend([a, c, b, b, c, d]);
        }
    }
    let mesh = meshes.add(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices(&rest))
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices)),
    );
    let material = materials.add(StandardMaterial {
        base_color_texture: texture.map(|t| images.add(convert::image(t))),
        base_color: if texture.is_some() {
            Color::WHITE
        } else {
            Color::srgb(0.9, 0.9, 0.9)
        },
        unlit: true,
        double_sided: true,
        cull_mode: None,
        alpha_mode: AlphaMode::Mask(0.5),
        ..default()
    });
    commands
        .spawn((
            Name::new("net"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            NoFrustumCulling,
            Net {
                rim: crate::court::BoneMove::STILL,
                points: rest.clone(),
                previous: rest.clone(),
                rest,
                links,
                mesh,
            },
        ))
        .id()
}

/// `FUN_0031a900`'s 120 links in their slots, each with its pinned point
/// first (`0x31aaa0`) and its length at setup: slots 0..10 hang ring 1 from
/// the top ring, 10..70 go round rings 1 to 6, 70..120 hang rings 2 to 6
/// from the ring above.
fn links(rest: &[Vec3]) -> Vec<(usize, usize, f32)> {
    let mut pinned = Vec::new();
    let mut free = Vec::new();
    let mut add = |a: usize, b: usize| {
        let to = if a < AROUND { &mut pinned } else { &mut free };
        to.push((a, b, rest[a].distance(rest[b])));
    };
    for r in 1..RINGS {
        for i in 0..AROUND {
            add(index(r, i), index(r, i + 1));
        }
    }
    for r in 0..RINGS - 1 {
        for i in 0..AROUND {
            add(index(r, i), index(r + 1, i));
        }
    }
    pinned.extend(free);
    pinned
}

/// One frame of a net (`FUN_0031a230`), the top ring already on the rim.
fn step_net(
    points: &mut [Vec3],
    previous: &mut [Vec3],
    links: &[(usize, usize, f32)],
    balls: &[(Vec3, f32)],
) {
    for k in AROUND..points.len() {
        let p = points[k];
        points[k] = p * (1.0 + KEEP) - previous[k] * KEEP + Vec3::Y * (GRAVITY * DT * DT);
        previous[k] = p;
    }
    for &(a, b, rest) in links {
        let d = points[b] - points[a];
        let len = d.length();
        let d = d * ((len - rest) / len * 0.5);
        if a < AROUND {
            points[b] -= d * 2.0;
        } else {
            points[a] += d;
            points[b] -= d;
        }
    }
    for &(centre, radius) in balls {
        let reach = radius * BALL_SCALE;
        for p in points.iter_mut().skip(AROUND) {
            let v = *p - centre;
            let len = v.length();
            if len < reach {
                *p = centre + v * (reach / len);
            }
        }
    }
}

/// The mesh's vertices: the points, with the seam column repeated.
fn vertices(points: &[Vec3]) -> Vec<[f32; 3]> {
    let mut out = Vec::with_capacity(RINGS * (AROUND + 1));
    for r in 0..RINGS {
        for i in 0..=AROUND {
            out.push(points[index(r, i)].to_array());
        }
    }
    out
}

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, step);
}

/// One step of every net.
pub fn step(
    mut nets: Query<&mut Net>,
    colliders: Query<(&GlobalTransform, &NetCollider)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let balls: Vec<(Vec3, f32)> = colliders
        .iter()
        .map(|(g, c)| (g.translation(), c.radius))
        .collect();
    for mut net in &mut nets {
        let net = &mut *net;
        // `FUN_0031a5f8`: the top ring on the rim.
        for k in 0..AROUND {
            net.points[k] = net.rim.apply(net.rest[k]);
        }
        step_net(&mut net.points, &mut net.previous, &net.links, &balls);
        if let Some(mut mesh) = meshes.get_mut(&net.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices(&net.points));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rest_shape_tapers_down_from_the_rim() {
        let p = rest_points(Vec3::new(0.0, 10.0, 0.0));
        assert_eq!(p.len(), 70);
        // The top ring circles the rim just under it.
        assert!((p[0] - Vec3::new(0.0, 9.9, 0.77)).length() < 1e-5);
        // The bottom ring is narrower and 1.3 below.
        let bottom = &p[60..];
        assert!(bottom.iter().all(|q| (q.y - 8.7).abs() < 1e-5));
        assert!(bottom.iter().all(|q| (q.x * q.x + q.z * q.z).sqrt() < 0.34));
    }

    #[test]
    fn the_links_sit_in_the_originals_slots() {
        let l = links(&rest_points(Vec3::ZERO));
        assert_eq!(l.len(), 120);
        assert!(l[..10].iter().all(|&(a, b, _)| a < 10 && b == a + 10));
        assert_eq!((l[10].0, l[10].1), (10, 11));
        assert_eq!((l[19].0, l[19].1), (19, 10));
        assert_eq!((l[70].0, l[70].1), (10, 20));
        assert_eq!((l[119].0, l[119].1), (59, 69));
    }

    #[test]
    fn the_net_hangs_still_and_a_ball_through_it_swishes() {
        let rest = rest_points(Vec3::new(0.0, 3.0, 0.0));
        let l = links(&rest);
        let (mut p, mut q) = (rest.clone(), rest.clone());
        for _ in 0..600 {
            step_net(&mut p, &mut q, &l, &[]);
        }
        // Gravity against one pass of links: it sags a little and settles.
        let sag = p
            .iter()
            .zip(&rest)
            .map(|(a, b)| a.distance(*b))
            .fold(0.0, f32::max);
        assert!(sag < 0.2, "{sag}");
        let moving = p
            .iter()
            .zip(&q)
            .map(|(a, b)| a.distance(*b))
            .fold(0.0, f32::max);
        assert!(moving < 1e-3, "{moving}");
        let hung = p.clone();
        // A ball dropping down the middle pushes the bottom ring out.
        for k in 0..40 {
            let ball = (Vec3::new(0.0, 3.2 - k as f32 * 0.05, 0.0), 0.35);
            step_net(&mut p, &mut q, &l, &[ball]);
        }
        let out = p[60..]
            .iter()
            .zip(&hung[60..])
            .map(|(a, b)| a.distance(*b))
            .fold(0.0, f32::max);
        assert!(out > 0.05, "{out}");
    }
}
