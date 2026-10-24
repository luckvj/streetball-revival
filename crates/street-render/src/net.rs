//! The nets: a cloth of 7 rings of 10 points hanging from each rim, the way
//! the game builds it (0x31af58 sets it up, 0x31b2d8 places the points,
//! 0x31a900 links them, 0x31ac88 steps it). Ring `r`, point `i` rests at
//! angle `i * 2pi / 10`, radius `RADII[r]`, `DROPS[r]` below the rim; the top
//! ring is fixed to the rim. The texture is the court's `<court>n.ssh`
//! (`net1`), else the shared `net.ssh`.
//!
//! The points move as a Verlet cloth held to their rest distances, and a
//! [`NetCollider`] (the ball) pushes them out of its way, so a basket swishes.

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

const GRAVITY: f32 = -38.0;
const DAMPING: f32 = 0.96;
const ITERATIONS: usize = 6;

/// Puts its entity's position in the nets' way (give it to the ball).
#[derive(Component, Clone, Copy)]
pub struct NetCollider {
    pub radius: f32,
}

#[derive(Component)]
pub struct Net {
    points: Vec<Vec3>,
    previous: Vec<Vec3>,
    rest: Vec<Vec3>,
    links: Vec<(usize, usize, f32)>,
    mesh: Handle<Mesh>,
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
    let mut links = Vec::new();
    let link = |a: usize, b: usize, links: &mut Vec<(usize, usize, f32)>| {
        links.push((a, b, rest[a].distance(rest[b])));
    };
    for r in 0..RINGS {
        for i in 0..AROUND {
            link(index(r, i), index(r, i + 1), &mut links);
            if r + 1 < RINGS {
                link(index(r, i), index(r + 1, i), &mut links);
                link(index(r, i), index(r + 1, i + 1), &mut links);
            }
        }
    }
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
                points: rest.clone(),
                previous: rest.clone(),
                rest,
                links,
                mesh,
            },
        ))
        .id()
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
fn step(
    time: Res<Time<Fixed>>,
    mut nets: Query<&mut Net>,
    colliders: Query<(&GlobalTransform, &NetCollider)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().max(1.0 / 240.0);
    let balls: Vec<(Vec3, f32)> = colliders
        .iter()
        .map(|(g, c)| (g.translation(), c.radius))
        .collect();
    for mut net in &mut nets {
        let net = &mut *net;
        // Only nets the ball is near need the work; the rest hang still.
        let busy = net
            .points
            .iter()
            .zip(&net.rest)
            .any(|(p, r)| p.distance_squared(*r) > 1e-6)
            || balls
                .iter()
                .any(|(b, rad)| b.distance(net.rest[0]) < 2.5 + rad);
        if !busy {
            continue;
        }
        for k in AROUND..net.points.len() {
            let p = net.points[k];
            let v = (p - net.previous[k]) * DAMPING;
            net.previous[k] = p;
            net.points[k] = p + v + Vec3::Y * GRAVITY * dt * dt;
        }
        for _ in 0..ITERATIONS {
            for &(a, b, len) in &net.links {
                let (pa, pb) = (net.points[a], net.points[b]);
                let d = pb - pa;
                let dist = d.length().max(1e-5);
                let fix = d * ((dist - len) / dist * 0.5);
                if a >= AROUND {
                    net.points[a] += fix;
                }
                if b >= AROUND {
                    net.points[b] -= fix;
                }
            }
            for &(centre, radius) in &balls {
                for p in net.points.iter_mut().skip(AROUND) {
                    let d = *p - centre;
                    let reach = radius + 0.04;
                    if d.length_squared() < reach * reach {
                        *p = centre + d.normalize_or(Vec3::NEG_Y) * reach;
                    }
                }
            }
            // The top ring stays on the rim.
            for k in 0..AROUND {
                net.points[k] = net.rest[k];
            }
        }
        // Settled: snap to rest so the net can go quiet.
        let moving = net
            .points
            .iter()
            .zip(&net.previous)
            .any(|(p, q)| p.distance_squared(*q) > 1e-7);
        if !moving {
            net.points = net.rest.clone();
            net.previous = net.rest.clone();
        }
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
}
