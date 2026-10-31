//! The players' gloss: the second pass the PS2's player microprograms draw.
//!
//! `Texture_ToonShade_Skin` (skin) and `Texture_ToonShade2_Skin` (clothes),
//! in `/XF/XF000007`, light nothing: every model's light block holds three
//! black lights and a white ambient, and its material (VU `0x79..0x7b`) a
//! diffuse of 0, an ambient of 128 and no emission, so the first pass is the
//! texture at vertex colour 0x80, as the GS dump of a played match shows on
//! every player strip. Each batch then draws again with the court's specular
//! map (`SpecularMap1` for skin, `SpecularMap2` for clothes;
//! [`street_assets::court::specular_maps`]) at vertex colour 0x7f:
//!
//! - the coordinate is a sphere map: the normal through the blended bone's
//!   model-view-projection rows, divided by the projection's diagonal (the
//!   view-space normal), `v = (0.49 x, -0.49 y, z)`, then `0.49 + 0.49 v / |v|`
//!   for s and t (instructions `0x0398..0x0498` set the constants, the
//!   vertex loop at `0x06f0` applies them);
//! - the blend (`GpState1`/`GpState2`, ALPHA_1 `A=Cd B=0 C=As D=Cs`) is
//!   `Cd * As / 128 + Cs` on the frame's 8-bit values, the texel modulated by
//!   0x7f, so a map's alpha above 0x80 brightens what is under it and its
//!   colour adds.
//!
//! [`GlossMaterial`] does both passes in one fragment: the unlit texture,
//! then that blend in display space. The coordinate is taken per fragment
//! from the interpolated normal, where the original interpolates the
//! vertices' coordinates (the rewrite's own, on triangles a few pixels wide).

use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat},
    shader::ShaderRef,
};
use street_assets::court::SpecularMap;

/// The second pass's map, beside the first pass's [`StandardMaterial`].
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Gloss {
    #[texture(100)]
    #[sampler(101)]
    pub map: Handle<Image>,
}

impl MaterialExtension for Gloss {
    fn fragment_shader() -> ShaderRef {
        "embedded://street_render/gloss.wesl".into()
    }
}

pub type GlossMaterial = ExtendedMaterial<StandardMaterial, Gloss>;

/// The gloss materials made so far, by first-pass material and map.
type Made = HashMap<(AssetId<StandardMaterial>, u8), Handle<GlossMaterial>>;

/// The court's `SpecularMap1` and `SpecularMap2`, as images. Players get
/// their gloss while this resource exists.
#[derive(Resource, Clone, Debug)]
pub struct SpecularMaps(pub [Handle<Image>; 2]);

impl SpecularMaps {
    pub fn new(maps: &[SpecularMap; 2], images: &mut Assets<Image>) -> Self {
        Self(maps.each_ref().map(|m| images.add(image(m))))
    }
}

/// Which specular map a player mesh's second pass samples (1 or 2,
/// `eagl::Mesh::specular`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlossKind(pub u8);

/// A specular map as the GS samples it: the stored texels, unconverted (the
/// shader reads alpha past 0x80 and blends in display space).
pub fn image(map: &SpecularMap) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: map.width,
            height: map.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        map.texels.clone(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "gloss.wesl");
    app.add_plugins(MaterialPlugin::<GlossMaterial>::default())
        .add_systems(
            PostUpdate,
            (follow_maps, apply)
                .chain()
                .run_if(resource_exists::<SpecularMaps>),
        );
}

/// Gives each marked mesh its gloss material: its first pass's material and
/// the map of its kind, one material per pair.
fn apply(
    mut commands: Commands,
    maps: Res<SpecularMaps>,
    meshes: Query<(Entity, &GlossKind, &MeshMaterial3d<StandardMaterial>)>,
    standard: Res<Assets<StandardMaterial>>,
    mut glossy: ResMut<Assets<GlossMaterial>>,
    mut made: Local<Made>,
) {
    for (entity, &GlossKind(kind), material) in &meshes {
        let Some(map) = kind.checked_sub(1).and_then(|k| maps.0.get(usize::from(k))) else {
            continue;
        };
        let Some(base) = standard.get(&material.0) else {
            continue;
        };
        let handle = made
            .entry((material.0.id(), kind))
            .or_insert_with(|| {
                glossy.add(GlossMaterial {
                    base: base.clone(),
                    extension: Gloss { map: map.clone() },
                })
            })
            .clone();
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));
    }
}

/// A new court's maps replace the old ones in every gloss material.
fn follow_maps(
    maps: Res<SpecularMaps>,
    meshes: Query<(&GlossKind, &MeshMaterial3d<GlossMaterial>)>,
    mut glossy: ResMut<Assets<GlossMaterial>>,
) {
    if !maps.is_changed() {
        return;
    }
    for (&GlossKind(kind), material) in &meshes {
        let Some(map) = kind.checked_sub(1).and_then(|k| maps.0.get(usize::from(k))) else {
            continue;
        };
        if let Some(mut m) = glossy.get_mut(&material.0)
            && m.extension.map != *map
        {
            m.extension.map = map.clone();
        }
    }
}
