//! Disc data to Bevy assets.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
};
use street_assets::{eagl, ssh};

/// A decoded SSH image as a repeating, filtered sRGB texture (the HD
/// pack's image instead when one is set: [`crate::hires`]).
pub fn image(t: &ssh::Texture) -> Image {
    let t = crate::hires::swap(t);
    let mut image = Image::new(
        Extent3d {
            width: t.width,
            height: t.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        t.rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// An EAGL mesh as a Bevy triangle list. `skinned` adds the joint attributes,
/// for meshes drawn with a [`SkinnedMesh`](bevy::mesh::skinning::SkinnedMesh).
pub fn mesh(m: &eagl::Mesh, skinned: bool) -> Mesh {
    // PS2 vertex colour 0x80 is 1.0; values above it brighten the texture.
    let colors: Vec<[f32; 4]> = m
        .colors
        .iter()
        .map(|c| {
            [
                c[0] as f32 / 128.0,
                c[1] as f32 / 128.0,
                c[2] as f32 / 128.0,
                (c[3] as f32 / 128.0).min(1.0),
            ]
        })
        .collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, m.uvs.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(m.indices.clone()));
    if m.normals.len() == m.positions.len() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone());
    }
    if skinned && m.joints.len() == m.positions.len() {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(m.joints.clone()),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, m.weights.clone());
    }
    mesh
}
