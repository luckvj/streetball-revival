//! Players' shadows, the way the game draws them: the skinned body flattened
//! onto the floor along the court's sun (`SUN_DIRECTION` in `/COURTFX`) and
//! filled with the court's `SHADOW_COLOUR` (the game's `ArbitraryShadow`
//! microprogram does this projection on VU1).
//!
//! The body is skinned on the CPU each frame (a few thousand vertices a
//! player) and projected to the floor. Where the body overlaps itself in the
//! projection, a plain transparent mesh would darken twice; the PS2 avoids
//! that with its destination-alpha test. Here the shadow writes depth and
//! every triangle sits a hair lower than the one before it, so a later
//! triangle never draws over an earlier one: one coat of shadow per pixel.
//!
//! Insert a [`ShadowStyle`] resource to turn shadows on; players spawned by
//! [`crate::player::spawn`] carry a [`ShadowSource`] and get theirs on the
//! next frame.

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::{MeshVertexBufferLayoutRef, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
};

use crate::animation::Rig;

/// Where shadows fall and their colour: from the court's effects script.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ShadowStyle {
    /// The light's direction (pointing down, towards the floor).
    pub direction: Vec3,
    /// RGBA as the PS2 takes it (gamma space, 0..1); alpha is the opacity.
    pub colour: [f32; 4],
    /// Height of the floor the shadows lie on.
    pub floor: f32,
}

impl ShadowStyle {
    /// From a court's `SUN_DIRECTION` and `SHADOW_COLOUR`.
    pub fn from_court(fx: &street_assets::courtfx::CourtFx) -> Self {
        let mut direction = Vec3::from_array(fx.sun_direction).normalize_or(Vec3::NEG_Y);
        // A sun at or below the horizon would throw shadows to infinity.
        if direction.y > -0.2 {
            direction.y = -0.2;
            direction = direction.normalize();
        }
        Self {
            direction,
            colour: fx.shadow_colour,
            floor: 0.0,
        }
    }
}

/// A skinned body to cast a shadow: every vertex of the shown meshes, one
/// copy per triangle corner (so each triangle can sit at its own height).
#[derive(Component, Clone)]
pub struct ShadowSource {
    pub positions: Vec<Vec3>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub inverse_bind: Vec<Mat4>,
    /// No skeleton: the vertices follow the entity's own transform (the
    /// ball, the hoops).
    pub rigid: bool,
    /// The shadow mesh, once spawned.
    pub mesh: Option<Handle<Mesh>>,
}

impl ShadowSource {
    /// Gathers the triangles of `meshes` (skinned meshes only).
    pub fn new<'a>(
        meshes: impl IntoIterator<Item = &'a street_assets::eagl::Mesh>,
        inverse_bind: Vec<Mat4>,
    ) -> Self {
        let mut s = ShadowSource {
            positions: Vec::new(),
            joints: Vec::new(),
            weights: Vec::new(),
            inverse_bind,
            rigid: false,
            mesh: None,
        };
        for m in meshes {
            if m.joints.len() != m.positions.len() {
                continue;
            }
            for &i in &m.indices {
                let i = i as usize;
                s.positions.push(Vec3::from_array(m.positions[i]));
                s.joints.push(m.joints[i]);
                s.weights.push(m.weights[i]);
            }
        }
        s
    }

    /// Gathers the triangles of `meshes` as they are (the ball, the hoops in
    /// their rest pose); the entity's transform places them.
    pub fn rigid<'a>(meshes: impl IntoIterator<Item = &'a street_assets::eagl::Mesh>) -> Self {
        let mut s = ShadowSource {
            positions: Vec::new(),
            joints: Vec::new(),
            weights: Vec::new(),
            inverse_bind: Vec::new(),
            rigid: true,
            mesh: None,
        };
        for m in meshes {
            s.positions.extend(
                m.indices
                    .iter()
                    .map(|&i| Vec3::from_array(m.positions[i as usize])),
            );
        }
        s
    }
}

/// A transparent material that still writes depth (see the module docs).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct WritesDepth {}

impl MaterialExtension for WritesDepth {
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(true);
        }
        Ok(())
    }
}

pub type ShadowMaterial = ExtendedMaterial<StandardMaterial, WritesDepth>;

/// Height step between consecutive shadow triangles (court units). It must
/// survive the depth buffer at the camera's distance (a 32-bit reversed depth
/// resolves about 1e-5 units there); 6,000 triangles stack 0.18 units high,
/// which nobody sees.
const LAYER: f32 = 3.0e-5;
/// The shadow floats this far above the floor.
const LIFT: f32 = 0.03;

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<ShadowMaterial>::default())
        .add_systems(
            PostUpdate,
            (attach, project)
                .chain()
                .after(TransformSystems::Propagate)
                .run_if(resource_exists::<ShadowStyle>),
        );
}

fn attach(
    mut commands: Commands,
    style: Res<ShadowStyle>,
    mut sources: Query<(Entity, &mut ShadowSource), Added<ShadowSource>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ShadowMaterial>>,
) {
    if sources.is_empty() {
        return;
    }
    let [r, g, b, a] = style.colour;
    let material = materials.add(ShadowMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(r, g, b, a.clamp(0.0, 1.0)),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        },
        extension: WritesDepth {},
    });
    for (entity, mut source) in &mut sources {
        let mesh = meshes.add(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_POSITION,
                vec![[0.0f32; 3]; source.positions.len()],
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_NORMAL,
                vec![[0.0f32, 1.0, 0.0]; source.positions.len()],
            ),
        );
        source.mesh = Some(mesh.clone());
        let shadow = commands
            .spawn((
                Name::new("shadow"),
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                NoFrustumCulling,
            ))
            .id();
        commands.entity(entity).add_child(shadow);
    }
}

/// Skins every shadow on the CPU and lays it on the floor.
fn project(
    style: Res<ShadowStyle>,
    sources: Query<(
        &ShadowSource,
        Option<&Rig>,
        &GlobalTransform,
        &InheritedVisibility,
    )>,
    joints: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let d = style.direction;
    for (source, rig, root, visible) in &sources {
        let Some(handle) = &source.mesh else { continue };
        if !visible.get() {
            continue;
        }
        let skin: Vec<Mat4> = match rig {
            Some(rig) if !source.rigid => rig
                .joints
                .iter()
                .zip(&source.inverse_bind)
                .map(|(&j, inv)| {
                    joints
                        .get(j)
                        .map_or(Mat4::IDENTITY, |g| g.to_matrix() * *inv)
                })
                .collect(),
            _ => Vec::new(),
        };
        let root_matrix = root.to_matrix();
        // The shadow is the root's child: express it in the root's space.
        let to_local = root.to_matrix().inverse();
        let Some(mut mesh) = meshes.get_mut(handle) else {
            continue;
        };
        let mut out = Vec::with_capacity(source.positions.len());
        let tris = source.positions.len() / 3;
        let none = [[0u16; 4]];
        let zero = [[0f32; 4]];
        let (joints_of, weights_of): (&[[u16; 4]], &[[f32; 4]]) = if source.rigid {
            (&none, &zero)
        } else {
            (&source.joints, &source.weights)
        };
        for (k, p) in source.positions.iter().enumerate() {
            let j = joints_of[k.min(joints_of.len() - 1)];
            let w = weights_of[k.min(weights_of.len() - 1)];
            let mut world = Vec3::ZERO;
            if source.rigid {
                world = root_matrix.transform_point3(*p);
            } else {
                for n in 0..4 {
                    if w[n] > 0.0
                        && let Some(m) = skin.get(j[n] as usize)
                    {
                        world += m.transform_point3(*p) * w[n];
                    }
                }
            }
            // Along the light to the floor; earlier triangles a hair higher.
            let height = style.floor + LIFT + (tris - k / 3) as f32 * LAYER;
            let t = (world.y - height) / -d.y;
            let flat = Vec3::new(world.x + d.x * t, height, world.z + d.z * t);
            out.push(to_local.transform_point3(flat).to_array());
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_arbitrary_shadow_plane_projection_matches_formula() {
        let sun_dir = Vec3::new(0.0, -20.0, -12.0).normalize();
        let floor = 0.0;
        let p = Vec3::new(5.0, 10.0, -3.0);
        let t = (p.y - floor) / -sun_dir.y;
        let proj = Vec3::new(p.x + sun_dir.x * t, floor, p.z + sun_dir.z * t);
        assert!((proj.y - floor).abs() < 1e-6);
        assert!((proj.x - p.x).abs() < 1e-6);
        assert!(proj.z < p.z);
    }
}
