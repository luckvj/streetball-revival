//! Vol. 2 (PS2) players as neutral [`Character`]s, assembled the way the
//! game's model loader does (`street_assets::player::look`) and posed the way
//! `street-render`'s `player::spawn` poses them: the 43-bone body skeleton,
//! the native width adjustment on the hips and shoulders, the head joint's
//! own scale, and the uniform `height * 1.2 / 78` scale on the armature.

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek};

use street_assets::{db, eagl, iso::Iso, player, ssh};

use crate::hires::{Hires, TextureKey};
use crate::model::{Character, Image, Joint, Material, Primitive};

/// Metres per skeleton unit: a unit is 10 inches. The game scales a player
/// `model height * 1.2 / 78`, and the rest body stands 6.5 units from the
/// soles to the crown (`Medium1Body` with its morph: feet at -4.05, crown
/// at 2.47), so a 78 in player is 7.8 units = 78 in; the rim stands 11.91
/// units (119 in). The contract (`docs/PLAYER_EDITOR.md`) wants metres.
pub const METRES_PER_UNIT: f32 = 0.254;

/// The skeleton's head joint, which carries its own scale (`19d59c`).
const HEAD: usize = 21;

/// Width relative to the height scale, as the native limb offsets use it
/// (`street-render`'s `native_body_width_ratio`).
fn width_ratio(body: Option<[f32; 3]>) -> f32 {
    match body {
        Some([height, width, _]) if width > height && height > 0.0 => width / height,
        _ => 1.0,
    }
}

/// `19d4c8`: the width scale on four joint translation components
/// (`street-render`'s `native_joint_translation`).
fn joint_translation(index: usize, t: [f32; 3], body: Option<[f32; 3]>) -> [f32; 3] {
    let ratio = width_ratio(body);
    match index {
        1 | 5 => [t[0], t[1], t[2] * ratio],
        13 | 17 => [t[0] * ratio, t[1], t[2]],
        _ => t,
    }
}

/// The skeleton as `player` wears it. `hip` lifts the root so the rest pose
/// stands on the floor, as every animation's root track does.
pub fn joints(
    skeleton: &eagl::Skeleton,
    body: Option<[f32; 3]>,
    head: Option<[f32; 3]>,
    hip: f32,
) -> Vec<Joint> {
    skeleton
        .bones
        .iter()
        .enumerate()
        .map(|(i, bone)| {
            let (translation, scale) = if i == HEAD {
                // `19d6f4`: the head's offset takes its applied X scale.
                let h = head.unwrap_or([1.0; 3]);
                (
                    bone.translation.map(|v| v * h[0]),
                    std::array::from_fn(|k| bone.scale[k] * h[k]),
                )
            } else {
                (joint_translation(i, bone.translation, body), bone.scale)
            };
            // The root stands on the floor and, like every clip's root
            // track, faces the other way from the bind pose (a half turn
            // about y: `[0, 1, 0, 0] * q`).
            let (translation, rotation) = if bone.parent.is_none() {
                let q = bone.rotation;
                (
                    [translation[0], translation[1] + hip, translation[2]],
                    [q[2], q[3], -q[0], -q[1]],
                )
            } else {
                (translation, bone.rotation)
            };
            Joint {
                name: bone.name.clone(),
                parent: bone.parent,
                translation,
                rotation,
                scale,
                inverse_bind: bone.inverse_bind,
            }
        })
        .collect()
}

/// The image a texture slot shows: the image of that name; kit bodies'
/// jersey slots take the jersey and `arms`/`legs` the right arm and leg
/// (`street-render`'s `texture_for`).
fn texture_for<'a>(
    slot: &str,
    textures: &'a BTreeMap<String, ssh::Texture>,
) -> Option<&'a ssh::Texture> {
    textures.get(slot).or_else(|| {
        let name = if player::JERSEY_SLOTS.contains(&slot) {
            "jers"
        } else {
            match slot {
                "arms" => "rarm",
                "legs" => "rleg",
                _ => return None,
            }
        };
        textures.get(name)
    })
}

/// Which way the game's character faces in glTF terms: the game's clips
/// face -z, so the armature gets a half turn and the rest pose and every
/// clip face +z (glTF's front).
pub const TURN: bool = true;

/// What kind of record this is, for the catalog.
pub fn kind_name(p: &db::Player) -> &'static str {
    match p.kind() {
        db::PlayerKind::League => "league",
        db::PlayerKind::Streetball => "streetball",
        db::PlayerKind::Legend => "legend",
        db::PlayerKind::Generic => "generic",
        db::PlayerKind::Created => "created",
    }
}

/// Record `p` as a character in its road uniform.
pub fn character<R: Read + Seek>(
    iso: &mut Iso<R>,
    p: &db::Player,
    skeleton: &eagl::Skeleton,
    hires: &dyn Hires,
) -> street_assets::Result<Character> {
    let id = format!("vol2:{}", p.index);
    let look = player::look(iso, p, player::Side::Road)?;
    let hip = look
        .models
        .first()
        .and_then(|m| m.bbox)
        .map_or(0.0, |(lo, _)| -lo[1]);
    let joints = joints(skeleton, p.native_body_scale(), p.native_head_scale(), hip);

    let mut images: Vec<Image> = Vec::new();
    let mut image_of: HashMap<(String, u32, u32, u64), usize> = HashMap::new();
    let mut materials: Vec<Material> = Vec::new();
    let mut material_of: HashMap<Option<String>, usize> = HashMap::new();
    let mut primitives = Vec::new();
    for model in &look.models {
        for mesh in model
            .meshes
            .iter()
            .filter(|m| m.group.as_deref().is_none_or(|g| look.shows(g)))
        {
            if mesh.indices.is_empty() {
                continue;
            }
            let material = *material_of.entry(mesh.texture.clone()).or_insert_with(|| {
                let slot = mesh.texture.clone().unwrap_or_default();
                let texture = mesh
                    .texture
                    .as_deref()
                    .and_then(|t| texture_for(t, &look.textures));
                let image = texture.map(|t| {
                    let key = TextureKey {
                        character: &id,
                        slot: &slot,
                        image: &t.name,
                    };
                    let image = hires.replace(&key, t).unwrap_or_else(|| Image {
                        name: t.name.clone(),
                        width: t.width,
                        height: t.height,
                        rgba: t.rgba.clone(),
                    });
                    let k = (
                        image.name.clone(),
                        image.width,
                        image.height,
                        hash(&image.rgba),
                    );
                    *image_of.entry(k).or_insert_with(|| {
                        images.push(image);
                        images.len() - 1
                    })
                });
                let alpha = image.is_some_and(|i| images[i].has_alpha());
                materials.push(Material {
                    name: if slot.is_empty() { "-".into() } else { slot },
                    image,
                    color: if image.is_some() {
                        [1.0; 4]
                    } else {
                        [0.35, 0.35, 0.38, 1.0]
                    },
                    alpha_mask: alpha,
                });
                materials.len() - 1
            });
            primitives.push(Primitive {
                name: format!(
                    "{}:{}:{}",
                    model.name,
                    mesh.group.as_deref().unwrap_or("-"),
                    mesh.texture.as_deref().unwrap_or("-")
                ),
                positions: mesh.positions.clone(),
                normals: mesh.normals.clone(),
                uvs: mesh.uvs.clone(),
                // PS2 vertex colour 0x80 is 1.0.
                colors: mesh
                    .colors
                    .iter()
                    .map(|c| c.map(|v| (f32::from(v) / 128.0).min(1.0)))
                    .collect(),
                joints: mesh.joints.clone(),
                weights: mesh.weights.clone(),
                indices: mesh.indices.clone(),
                material,
            });
        }
    }
    let mut extras = serde_json::Map::new();
    extras.insert("heightIn".into(), p.height_in.into());
    extras.insert("modelHeightIn".into(), p.model_height_in.into());
    extras.insert("record".into(), p.index.into());
    extras.insert("name".into(), p.name().into());
    extras.insert(
        "groups".into(),
        look.extra_groups
            .iter()
            .cloned()
            .map(serde_json::Value::from)
            .collect::<Vec<_>>()
            .into(),
    );
    Ok(Character {
        id,
        name: p.name(),
        skeleton: "vol2".into(),
        joints,
        primitives,
        materials,
        images,
        scale: look.scale * METRES_PER_UNIT,
        turn: TURN,
        extras,
    })
}

/// FNV-1a, to share identical images between slots.
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_goes_on_the_hips_and_shoulders_only() {
        let body = Some([1.0, 1.5, 1.5]);
        assert_eq!(joint_translation(1, [2.0, 3.0, 4.0], body), [2.0, 3.0, 6.0]);
        assert_eq!(
            joint_translation(17, [2.0, 3.0, 4.0], body),
            [3.0, 3.0, 4.0]
        );
        assert_eq!(joint_translation(9, [2.0, 3.0, 4.0], body), [2.0, 3.0, 4.0]);
        assert_eq!(joint_translation(1, [2.0, 3.0, 4.0], None), [2.0, 3.0, 4.0]);
    }

    #[test]
    fn a_78_inch_player_is_1_98_m() {
        assert!((6.5 * 1.2 * METRES_PER_UNIT - 1.9812).abs() < 1e-5);
    }
}
