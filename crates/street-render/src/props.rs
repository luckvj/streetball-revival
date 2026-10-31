//! Spawning a stand-alone model such as the ball.

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;
use street_assets::{eagl, ssh};

use crate::convert;

/// Spawn every mesh of `model` under one entity at `transform`, textured from
/// `textures` by slot name. A mesh of the toon programs (the ball's) draws
/// its texture unlit and takes the court's gloss ([`crate::gloss`]); other
/// models with normals are lit, and the rest are unlit and rely on their
/// vertex colours, as the courts do.
pub fn spawn(
    commands: &mut Commands,
    model: &eagl::Model,
    textures: &BTreeMap<String, ssh::Texture>,
    transform: Transform,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Entity {
    let root = commands
        .spawn((
            Name::new(model.name.clone()),
            transform,
            Visibility::default(),
        ))
        .id();
    let mut material_for: HashMap<Option<String>, Handle<StandardMaterial>> = HashMap::new();
    for mesh in &model.meshes {
        let lit = mesh.specular == 0 && mesh.normals.len() == mesh.positions.len();
        let material = material_for
            .entry(mesh.texture.clone())
            .or_insert_with(|| {
                let texture = mesh.texture.as_ref().and_then(|t| textures.get(t));
                materials.add(StandardMaterial {
                    base_color_texture: texture.map(|t| images.add(convert::display_image(t))),
                    unlit: !lit,
                    perceptual_roughness: 0.7,
                    cull_mode: None,
                    ..default()
                })
            })
            .clone();
        let mut child = commands.spawn((
            Mesh3d(meshes.add(convert::mesh(mesh, false))),
            MeshMaterial3d(material),
        ));
        if mesh.specular != 0 {
            child.insert(crate::gloss::GlossKind(mesh.specular));
        }
        let child = child.id();
        commands.entity(root).add_child(child);
    }
    root
}
