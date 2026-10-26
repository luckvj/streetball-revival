//! Spawning a court.

use std::collections::HashMap;

use bevy::prelude::*;
use street_assets::{court, ssh};

use crate::convert;

/// The draw group a court mesh belongs to (see `street_assets::eagl`), so a
/// program can show and hide groups the way the game does.
#[derive(Component, Debug, Clone)]
pub struct DrawGroup(pub String);

/// How a bone moved from its rest pose: a point that rode it goes to
/// `to + rot (p − from)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneMove {
    pub rot: Quat,
    pub from: Vec3,
    pub to: Vec3,
}

impl BoneMove {
    pub const STILL: BoneMove = BoneMove {
        rot: Quat::IDENTITY,
        from: Vec3::ZERO,
        to: Vec3::ZERO,
    };

    pub fn apply(&self, p: Vec3) -> Vec3 {
        self.to + self.rot * (p - self.from)
    }
}

/// A hoop mesh whose vertices ride the hoop's bones (`<court>d` the -x
/// hoop, side 0, and `<court>b` the +x one, side 1), weighted as the
/// model's skin gives them, for the rims' give ([`HoopSkin::pose`]).
#[derive(Component, Debug, Clone)]
pub struct HoopSkin {
    pub side: usize,
    pub mesh: Handle<Mesh>,
    rest: Vec<[f32; 3]>,
    joints: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
    shown: [BoneMove; 5],
}

impl HoopSkin {
    /// Bones 0..4's moves in the hoop model's space.
    pub fn pose(&mut self, moves: [BoneMove; 5], meshes: &mut Assets<Mesh>) {
        if moves == self.shown {
            return;
        }
        self.shown = moves;
        let positions: Vec<[f32; 3]> = self
            .rest
            .iter()
            .zip(self.joints.iter().zip(&self.weights))
            .map(|(p, (j, w))| {
                let p = Vec3::from_array(*p);
                let mut out = Vec3::ZERO;
                for (b, weight) in j.iter().zip(w) {
                    out += *weight * moves[usize::from(*b).min(4)].apply(p);
                }
                out.to_array()
            })
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&self.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
    }
}

/// Spawn every model of `court` (except draw groups the game hides) at the
/// placement the game gives it. Returns the model entities.
pub fn spawn(
    commands: &mut Commands,
    court: &court::Court,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Vec<Entity> {
    let mut material_for: HashMap<(Option<String>, i32), Handle<StandardMaterial>> = HashMap::new();
    let mut spawned = Vec::with_capacity(court.models.len());
    let id = court.id.to_ascii_lowercase();
    for model in &court.models {
        let layer = ground_layer(&id, &model.name);
        let transform = court
            .placements
            .get(&model.name)
            .map_or(Transform::default(), |p| Transform {
                translation: Vec3::from_array(p.translation),
                rotation: Quat::from_array(p.rotation),
                ..default()
            });
        let parent = commands
            .spawn((
                Name::new(model.name.clone()),
                transform,
                Visibility::default(),
            ))
            .id();
        for (k, mesh) in model
            .meshes
            .iter()
            .filter(|m| !m.group.as_deref().is_some_and(court::hidden_group))
            .enumerate()
        {
            let bias = ground_bias(layer, k);
            let material = material_for
                .entry((mesh.texture.clone(), bias))
                .or_insert_with(|| {
                    let texture = mesh.texture.as_ref().and_then(|t| court.textures.get(t));
                    let mut m = material(texture, images);
                    m.depth_bias = bias as f32;
                    materials.add(m)
                })
                .clone();
            let handle = meshes.add(convert::mesh(mesh, false));
            let mut child = commands.spawn((
                Name::new(format!(
                    "{}:{}",
                    model.name,
                    mesh.texture.as_deref().unwrap_or("-")
                )),
                Mesh3d(handle.clone()),
                MeshMaterial3d(material),
            ));
            if let Some(side) = hoop_side(&id, &model.name)
                && mesh.joints.len() == mesh.positions.len()
            {
                child.insert(HoopSkin {
                    side,
                    mesh: handle,
                    rest: mesh.positions.clone(),
                    joints: mesh.joints.clone(),
                    weights: mesh.weights.clone(),
                    shown: [BoneMove::STILL; 5],
                });
            }
            if let Some(group) = &mesh.group {
                child.insert(DrawGroup(group.clone()));
            }
            let child = child.id();
            commands.entity(parent).add_child(child);
        }
        // The hoops (`<court>b`, `<court>d`) cast shadows, as their
        // `ArbitraryShadow` copies (`c`, `c2`) do in the game.
        let hoop = hoop_side(&id, &model.name).is_some();
        if hoop {
            commands
                .entity(parent)
                .insert(crate::shadow::ShadowSource::rigid(
                    model
                        .meshes
                        .iter()
                        .filter(|m| !m.group.as_deref().is_some_and(court::hidden_group)),
                ));
        }
        spawned.push(parent);
    }
    spawned
}

/// The hoop a court model is: `<court>d` the -x hoop (0), `<court>b` the
/// +x one (1).
fn hoop_side(court: &str, model: &str) -> Option<usize> {
    match model.strip_prefix(court)? {
        "d" => Some(0),
        "b" => Some(1),
        _ => None,
    }
}

/// Where a model sits among the court's ground layers: every court has its
/// floor (`<court>f`) and over it, in the same plane, its lines and centre
/// logo (`<court>l`). The PS2 draws them in that order and the later one
/// wins on equal depth; on the PC the two fight (the stripes through
/// Rucker's centre logo, Vj 2026-10-21), so each gets a depth bias by its
/// place in that order.
fn ground_layer(court: &str, model: &str) -> Option<u8> {
    let rest = model.strip_prefix(court)?;
    match rest {
        "f" => Some(0),
        "l" => Some(1),
        _ => None,
    }
}

/// The depth bias (wgpu's constant units, towards the camera) of the `k`th
/// mesh of a ground layer, in draw order: the floor's meshes first, then
/// the lines and logo's, each above everything drawn before it.
fn ground_bias(layer: Option<u8>, k: usize) -> i32 {
    const STEP: i32 = 4;
    const LAYER: i32 = 64;
    match layer {
        None => 0,
        Some(l) => (i32::from(l) * LAYER + k.min(15) as i32 + 1) * STEP,
    }
}

/// Unlit, double-sided: the PS2 courts carry baked lighting in their vertex colours.
fn material(texture: Option<&ssh::Texture>, images: &mut Assets<Image>) -> StandardMaterial {
    // The shared court GS packet disables alpha testing (TEST_1=0x50003)
    // and uses source-alpha blending (ALPHA_1=0x44). Binary-alpha textures
    // also blend: filtering creates fractional coverage at their edges.
    let alpha_mode = match texture {
        Some(t) if t.has_alpha() => AlphaMode::Blend,
        _ => AlphaMode::Opaque,
    };
    StandardMaterial {
        base_color_texture: texture.map(|t| images.add(convert::image(t))),
        unlit: true,
        double_sided: true,
        cull_mode: None,
        alpha_mode,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_and_logo_sit_over_the_floor_in_draw_order() {
        assert_eq!(ground_layer("rucker", "ruckerf"), Some(0));
        assert_eq!(ground_layer("rucker", "ruckerl"), Some(1));
        assert_eq!(ground_layer("rucker", "ruckerb"), None);
        assert_eq!(ground_bias(None, 3), 0);
        // Every floor mesh under every lines mesh, each mesh over the last.
        let floor: Vec<i32> = (0..16).map(|k| ground_bias(Some(0), k)).collect();
        let lines: Vec<i32> = (0..16).map(|k| ground_bias(Some(1), k)).collect();
        assert!(floor.windows(2).all(|w| w[0] < w[1]));
        assert!(floor.iter().max() < lines.iter().min());
    }
}
