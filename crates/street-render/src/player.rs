//! Loading and spawning a skinned player: a database player assembled the
//! way the game does, or create-a-player kit parts.

use std::{
    collections::{BTreeMap, HashMap},
    io::{Read, Seek},
};

use bevy::{
    camera::visibility::NoFrustumCulling,
    mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    prelude::*,
};
use street_assets::{db, eagl, iso::Iso, player, ssh};

use crate::{
    animation::{Rig, native_joint_translation},
    convert,
};

/// Which player to build.
#[derive(Clone, Debug)]
pub enum Choice {
    /// A database record (0 is Shareef Abdur-Rahim; 0..145 are the league
    /// players, then streetballers, legends and generic ballers), in the
    /// team's road or home uniform.
    Record { index: usize, side: player::Side },
    /// A database player as given (a created baller's edited record), for
    /// league team `team` when one (a created player wears its uniform).
    Player {
        record: Box<db::Player>,
        side: player::Side,
        team: Option<usize>,
    },
    /// Create-a-player kit parts by model name, dressed in the textures of
    /// league player `textures`.
    Kit {
        textures: usize,
        body: String,
        head: String,
    },
}

impl Default for Choice {
    fn default() -> Self {
        Self::Record {
            index: 0,
            side: player::Side::Road,
        }
    }
}

/// `19d6f4..19d744` uses the head's applied X scale for its complete offset.
fn native_head_translation(translation: [f32; 3], head: Option<[f32; 3]>, size: f32) -> Vec3 {
    Vec3::from_array(translation) * (head.unwrap_or([1.0; 3])[0] * size)
}

/// Everything needed to spawn one player.
pub struct PlayerAssets {
    pub name: String,
    pub models: Vec<eagl::Model>,
    pub textures: BTreeMap<String, ssh::Texture>,
    pub skeleton: eagl::Skeleton,
    /// Optional draw groups this player shows (headband, dreadlocks).
    pub extra_groups: Vec<String>,
    /// Uniform scale the game gives the model for the player's height.
    pub scale: f32,
    /// Native skeleton scale before the outer uniform size option.
    pub native_body_scale: Option<[f32; 3]>,
    /// Native per-axis head transform, before the scalar head-size option.
    pub native_head_scale: Option<[f32; 3]>,
    /// Original skeleton head transform multiplier (joint 21).
    pub head_scale: f32,
}

impl PlayerAssets {
    pub fn load<R: Read + Seek>(
        iso: &mut Iso<R>,
        choice: &Choice,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let skeleton = player::skeleton(iso)?;
        match choice {
            Choice::Record { index, side } => {
                let record = db::players(iso)?
                    .into_iter()
                    .nth(*index)
                    .ok_or(format!("no player record {index}"))?;
                if let Some(gc) = Self::gamecube(&record, *side, &skeleton) {
                    return Ok(gc);
                }
                let look = player::look(iso, &record, *side)?;
                Ok(Self {
                    name: record.name(),
                    models: look.models,
                    textures: look.textures,
                    skeleton,
                    extra_groups: look.extra_groups,
                    scale: look.scale,
                    native_body_scale: look.native_body_scale,
                    native_head_scale: look.native_head_scale,
                    head_scale: 1.0,
                })
            }
            Choice::Player { record, side, team } => {
                if let Some(gc) = Self::gamecube(record, *side, &skeleton) {
                    return Ok(gc);
                }
                let look = player::look_for_team(iso, record, *side, *team)?;
                Ok(Self {
                    name: record.name(),
                    models: look.models,
                    textures: look.textures,
                    skeleton,
                    extra_groups: look.extra_groups,
                    scale: look.scale,
                    native_body_scale: look.native_body_scale,
                    native_head_scale: look.native_head_scale,
                    head_scale: 1.0,
                })
            }
            Choice::Kit {
                textures,
                body,
                head,
            } => {
                let parts = player::Parts::load(iso)?;
                Ok(Self {
                    name: format!("{body} + {head}"),
                    models: vec![parts.model(iso, body)?, parts.model(iso, head)?],
                    textures: player::player_textures(iso, *textures)?,
                    skeleton,
                    extra_groups: Vec::new(),
                    scale: 1.0,
                    native_body_scale: None,
                    native_head_scale: None,
                    head_scale: 1.0,
                })
            }
        }
    }

    /// The referee of the opening toss (`street_assets::player::referee`).
    pub fn referee<R: Read + Seek>(iso: &mut Iso<R>) -> Result<Self, Box<dyn std::error::Error>> {
        let look = player::referee(iso)?;
        Ok(Self {
            name: "Referee".into(),
            models: look.models,
            textures: look.textures,
            skeleton: player::skeleton(iso)?,
            extra_groups: look.extra_groups,
            scale: look.scale,
            native_body_scale: None,
            native_head_scale: None,
            head_scale: 1.0,
        })
    }

    /// A roster.json player drawn as a GameCube character
    /// (`roster_edits::gamecube_model`), when its disc was given
    /// (`street_assets::gc_discs`): the character on Vol. 2's skeleton
    /// (`street_assets::canonical`), so Vol. 2's clips drive it as they
    /// drive any player, sized to the record's height. `None` keeps the
    /// Vol. 2 stand-in.
    fn gamecube(record: &db::Player, side: player::Side, vol2: &eagl::Skeleton) -> Option<Self> {
        use street_assets::{gc_character::Look, gc_discs};
        let id = street_assets::roster_edits::gamecube_model(record.index)?;
        if !gc_discs::has(&id) {
            return None;
        }
        let look = match side {
            player::Side::Home => Look::Home,
            player::Side::Road => Look::Away,
        };
        match gc_discs::load(&id, look, vol2) {
            Ok(loaded) => Some(Self::from_gamecube(&loaded, record)),
            Err(e) => {
                warn!("{id}: {e}; drawing the Vol. 2 stand-in");
                None
            }
        }
    }

    fn from_gamecube(loaded: &street_assets::gc_discs::Loaded, record: &db::Player) -> Self {
        let r = &loaded.rebound;
        let c = &r.character;
        let name = |i: usize| format!("gc{i}");
        let textures = c
            .textures
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (
                    name(i),
                    ssh::Texture {
                        name: t.name.clone(),
                        width: t.width,
                        height: t.height,
                        rgba: t.rgba.clone(),
                        indices: None,
                    },
                )
            })
            .collect();
        let meshes = c
            .meshes
            .iter()
            .map(|m| {
                let n = m.positions.len();
                eagl::Mesh {
                    group: None,
                    texture: m.texture.map(name),
                    positions: m.positions.clone(),
                    uvs: if m.uvs.len() == n {
                        m.uvs.clone()
                    } else {
                        vec![[0.0; 2]; n]
                    },
                    colors: vec![[128; 4]; n],
                    normals: m.normals.clone(),
                    joints: m.joints.clone(),
                    weights: m.weights.clone(),
                    sources: Vec::new(),
                    indices: m.indices.clone(),
                }
            })
            .collect();
        let model = eagl::Model {
            name: loaded.info.id.clone(),
            meshes,
            bbox: c.bounds(),
            ..Default::default()
        };
        // The record's height (edits included) over the table's, on the
        // height the character is drawn at.
        let table = loaded.info.height_in.unwrap_or(0) as f32;
        let edited = f32::from(record.model_height_in);
        let height = if table > 0.0 && edited > 0.0 {
            loaded.model_height_in * edited / table
        } else {
            loaded.model_height_in
        };
        Self {
            name: record.name(),
            models: vec![model],
            textures,
            skeleton: c.skeleton.clone(),
            extra_groups: Vec::new(),
            // Game units are 10 inches.
            scale: height / 10.0 / r.height.max(1e-3),
            native_body_scale: None,
            native_head_scale: None,
            head_scale: 1.0,
        }
    }

    /// Height of the hips above the feet in the rest pose, scaled.
    pub fn hip_height(&self) -> f32 {
        self.scale * self.models[0].bbox.map_or(0.0, |(lo, _)| -lo[1])
    }

    fn shows(&self, group: &str) -> bool {
        player::default_groups(group) || self.extra_groups.iter().any(|g| g == group)
    }
}

/// The image for a texture slot: the image of that name, as database players
/// are bound. Kit bodies have slots the league sheets lack: their jersey slots
/// get the jersey (as the game does), and `arms` and `legs` use the right-arm
/// and right-leg images as stand-ins; the rest (socks, hair) stay untextured.
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

/// Spawn a player at `transform` (origin on the floor under the hips when an
/// animation drives the root; see [`crate::animation`]). Returns the root
/// entity, which carries a [`Rig`].
pub fn spawn(
    commands: &mut Commands,
    assets: &PlayerAssets,
    transform: Transform,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
) -> Entity {
    let transform = transform.with_scale(transform.scale * assets.scale);
    let root = commands
        .spawn((
            Name::new(assets.name.clone()),
            transform,
            Visibility::default(),
        ))
        .id();

    // Joints in the skeleton's parents-first order, each under its parent.
    let mut joints: Vec<Entity> = Vec::with_capacity(assets.skeleton.bones.len());
    for (index, bone) in assets.skeleton.bones.iter().enumerate() {
        let joint = commands
            .spawn((
                Name::new(bone.name.clone()),
                Transform {
                    translation: if index == 21 {
                        native_head_translation(
                            bone.translation,
                            assets.native_head_scale,
                            assets.head_scale,
                        )
                    } else {
                        native_joint_translation(index, bone.translation, assets.native_body_scale)
                    },
                    rotation: Quat::from_array(bone.rotation),
                    scale: Vec3::from_array(bone.scale)
                        * if index == 21 {
                            Vec3::from_array(assets.native_head_scale.unwrap_or([1.0; 3]))
                                * assets.head_scale
                        } else {
                            Vec3::ONE
                        },
                },
            ))
            .id();
        let parent = bone.parent.map_or(root, |p| joints[p]);
        commands.entity(parent).add_child(joint);
        joints.push(joint);
    }
    let inverse_bind: Vec<Mat4> = assets
        .skeleton
        .bones
        .iter()
        .map(|b| Mat4::from_cols_array(&b.inverse_bind))
        .collect();
    let inverse_bindposes = bindposes.add(inverse_bind.clone());
    let shadow = crate::shadow::ShadowSource::new(
        assets.models.iter().flat_map(|model| {
            model
                .meshes
                .iter()
                .filter(|m| m.group.as_deref().is_none_or(|g| assets.shows(g)))
        }),
        inverse_bind,
    );

    let mut material_for: HashMap<Option<String>, Handle<StandardMaterial>> = HashMap::new();
    for model in &assets.models {
        for mesh in model
            .meshes
            .iter()
            .filter(|m| m.group.as_deref().is_none_or(|g| assets.shows(g)))
        {
            let material = material_for
                .entry(mesh.texture.clone())
                .or_insert_with(|| {
                    let texture = mesh
                        .texture
                        .as_deref()
                        .and_then(|t| texture_for(t, &assets.textures));
                    if let (Some(slot), None) = (&mesh.texture, texture) {
                        warn!("{}: no image for texture slot {slot}", assets.name);
                    }
                    materials.add(StandardMaterial {
                        base_color: if texture.is_some() {
                            Color::WHITE
                        } else {
                            Color::srgb(0.35, 0.35, 0.38)
                        },
                        base_color_texture: texture.map(|t| images.add(convert::image(t))),
                        perceptual_roughness: 0.85,
                        // The game toon-shades players with no specular term. Bevy's
                        // Fresnel sheen would turn dark hair and jerseys grey at
                        // grazing angles; zero reflectance turns it off.
                        reflectance: 0.0,
                        // Thin parts (braids, jersey edges) are seen from both sides.
                        // The decoder winds triangles to agree with their normals, so
                        // Bevy's double-sided mode flips the normal on true back faces.
                        double_sided: true,
                        cull_mode: None,
                        alpha_mode: match texture {
                            Some(t) if t.has_alpha() => AlphaMode::Mask(0.5),
                            _ => AlphaMode::Opaque,
                        },
                        ..default()
                    })
                })
                .clone();
            let skinned = mesh.joints.len() == mesh.positions.len();
            let mut child = commands.spawn((
                Name::new(format!(
                    "{}:{}",
                    model.name,
                    mesh.texture.as_deref().unwrap_or("-")
                )),
                Mesh3d(meshes.add(convert::mesh(mesh, skinned))),
                MeshMaterial3d(material),
            ));
            if skinned {
                // The bounds Bevy culls with come from the rest pose, which the
                // animated mesh leaves (feet 4 ft below an origin on the floor).
                child.insert((
                    SkinnedMesh {
                        inverse_bindposes: inverse_bindposes.clone(),
                        joints: joints.clone(),
                    },
                    NoFrustumCulling,
                ));
            }
            let child = child.id();
            commands.entity(root).add_child(child);
        }
    }
    commands
        .entity(root)
        .insert((Rig::new(joints, &assets.skeleton), shadow));
    root
}

#[cfg(test)]
mod native_body_tests {
    use super::{native_head_translation, native_joint_translation};
    use bevy::prelude::Vec3;

    #[test]
    fn head_origin_uses_applied_x_scale_on_all_offset_components() {
        // Native19d6f4 branches past the width exceptions to multiply the
        // entire head offset by its already permuted X scale. Head size
        // options affect that factor before both origin and scale are built.
        assert_eq!(
            native_head_translation([2.0, 3.0, 4.0], Some([0.5, 2.0, 1.25]), 1.5),
            Vec3::new(1.5, 2.25, 3.0)
        );
        assert_eq!(
            native_head_translation([2.0, 3.0, 4.0], None, 1.0),
            Vec3::new(2.0, 3.0, 4.0)
        );
    }

    #[test]
    fn width_changes_only_native_limb_components_and_preserves_uniform_outer_scale() {
        let point = [2.0, 3.0, 4.0];
        let body = Some([1.0, 1.5, 1.5]);
        for index in [1, 5] {
            assert_eq!(
                native_joint_translation(index, point, body),
                Vec3::new(2.0, 3.0, 6.0)
            );
        }
        for index in [13, 17] {
            assert_eq!(
                native_joint_translation(index, point, body),
                Vec3::new(3.0, 3.0, 4.0)
            );
        }
        for index in [0, 2, 12, 15, 19, 21, 22, 42] {
            assert_eq!(
                native_joint_translation(index, point, body),
                Vec3::from_array(point)
            );
        }
        assert_eq!(
            native_joint_translation(1, point, Some([1.2; 3])),
            Vec3::from_array(point)
        );
        assert_eq!(
            native_joint_translation(1, point, None),
            Vec3::from_array(point)
        );
        // Size cheats change the outer scale, keeping the width ratio intact.
        assert_eq!(
            native_joint_translation(1, point, body) * 0.5,
            Vec3::new(1.0, 1.5, 3.0)
        );
    }
}
