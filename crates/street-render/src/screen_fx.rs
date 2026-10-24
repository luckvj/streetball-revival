//! The original's screen passes over its 3D picture (docs/TRACE.md "The
//! Level 2 look"). Its frame starts by copying the buffer it drew last into
//! the one shown, as a 640x448 sprite blended `(Cs - Cd) * As + Cd` at the
//! display's alpha and colour (`FUN_00376ac0`, `FUN_00376ad8`); the scene is
//! drawn, then full-screen untextured quads go over it before the 2D.
//!
//! - [`ScreenLayers`]: those quads, as a pass after tonemapping and before
//!   the UI.
//! - [`ScreenCopy`]: the copy, as a pass after the UI blending the frame
//!   over the last one shown (kept per view). At its usual 0x7f, 0x7f the
//!   rewrite shows the frame as drawn; the 16-bit shown buffer's dither is
//!   not drawn.

use std::collections::HashMap;

use bevy::{
    asset::embedded_asset,
    core_pipeline::{
        FullscreenShader,
        fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin},
        schedule::{Core3d, Core3dSystems},
        tonemapping::tonemapping,
        upscaling::upscaling,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_resource::{
            BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, Extent3d, FragmentState,
            Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
            ShaderType, TexelCopyTextureInfo, Texture, TextureDescriptor, TextureDimension,
            TextureFormat, TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor,
            binding_types::{sampler, texture_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        sync_world::MainEntity,
        view::ViewTarget,
    },
    shader::ShaderRef,
};

/// Layers in a [`ScreenLayers`].
pub const LAYERS: usize = 4;

/// Full-screen quads over the 3D picture, in order: each a colour on the
/// original's 0..255 (its alpha the layer's, halved to the GS's 0..128 by
/// the pass) and a blend: 0 none, 1 `(Cs - Cd) * As + Cd`, 2
/// `Cs * As + Cd`, 3 `Cd - Cs`.
#[derive(Component, Clone, Copy, Default, Debug, PartialEq, ShaderType, ExtractComponent)]
#[extract_app(RenderApp)]
pub struct ScreenLayers {
    pub colour: [Vec4; LAYERS],
    pub blend: Vec4,
}

impl ScreenLayers {
    /// Whether any layer draws.
    pub fn any(&self) -> bool {
        self.blend
            .to_array()
            .iter()
            .any(|&b| (1.0..=3.0).contains(&b))
    }
}

impl FullscreenMaterial for ScreenLayers {
    fn fragment_shader() -> ShaderRef {
        "embedded://street_render/screen_layers.wgsl".into()
    }

    fn schedule_configs(
        system: bevy::ecs::schedule::ScheduleConfigs<bevy::ecs::system::BoxedSystem>,
    ) -> bevy::ecs::schedule::ScheduleConfigs<bevy::ecs::system::BoxedSystem> {
        system.in_set(Core3dSystems::PostProcess).after(tonemapping)
    }
}

/// The copy into the shown buffer: its alpha and colour on the GS's 0..128
/// (0x80 is one). [`ScreenCopy::SHOWN`] shows the frame as drawn.
#[derive(Component, Clone, Copy, Debug, PartialEq, ShaderType, ExtractComponent)]
#[extract_app(RenderApp)]
pub struct ScreenCopy {
    pub alpha: f32,
    pub colour: f32,
    pub _pad: Vec2,
}

impl ScreenCopy {
    pub const SHOWN: Self = Self::new(128, 128);

    pub const fn new(alpha: u8, colour: u8) -> Self {
        Self {
            alpha: alpha as f32,
            colour: colour as f32,
            _pad: Vec2::ZERO,
        }
    }
}

impl Default for ScreenCopy {
    fn default() -> Self {
        Self::SHOWN
    }
}

pub struct ScreenFxPlugin;

impl Plugin for ScreenFxPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "screen_layers.wgsl");
        embedded_asset!(app, "screen_copy.wgsl");
        app.add_plugins((
            FullscreenMaterialPlugin::<ScreenLayers>::default(),
            ExtractComponentPlugin::<ScreenCopy>::default(),
            UniformComponentPlugin::<ScreenCopy>::default(),
        ));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<ShownFrames>()
            .add_systems(RenderStartup, init_copy_pipeline)
            .add_systems(Render, queue_copy_pipelines.in_set(RenderSystems::Prepare))
            .add_systems(
                Core3d,
                copy_pass
                    .after(bevy::ui_render::render_pass::ui_pass)
                    .before(upscaling),
            );
    }
}

#[derive(Resource)]
struct CopyPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    descriptor: RenderPipelineDescriptor,
    by_format: HashMap<TextureFormat, CachedRenderPipelineId>,
}

fn init_copy_pipeline(
    mut commands: Commands,
    device: Res<RenderDevice>,
    assets: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "screen_copy_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<ScreenCopy>(true),
            ),
        ),
    );
    let descriptor = RenderPipelineDescriptor {
        label: Some("screen_copy".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: assets.load("embedded://street_render/screen_copy.wgsl"),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    };
    commands.insert_resource(CopyPipeline {
        layout,
        sampler: device.create_sampler(&SamplerDescriptor::default()),
        descriptor,
        by_format: HashMap::new(),
    });
}

/// A pipeline for each view format a [`ScreenCopy`] draws to.
fn queue_copy_pipelines(
    cache: Res<PipelineCache>,
    mut pipeline: ResMut<CopyPipeline>,
    views: Query<&ViewTarget, With<ScreenCopy>>,
) {
    for target in &views {
        let format = target.main_texture_format();
        if pipeline.by_format.contains_key(&format) {
            continue;
        }
        let mut descriptor = pipeline.descriptor.clone();
        if let Some(fragment) = descriptor.fragment.as_mut()
            && let Some(Some(target)) = fragment.targets.first_mut()
        {
            target.format = format;
        }
        let id = cache.queue_render_pipeline(descriptor);
        pipeline.by_format.insert(format, id);
    }
}

/// The picture each view showed last (the original's shown buffer).
#[derive(Resource, Default)]
struct ShownFrames(HashMap<MainEntity, Shown>);

struct Shown {
    texture: Texture,
    view: TextureView,
    size: Extent3d,
    format: TextureFormat,
}

fn copy_pass(
    view: ViewQuery<(&MainEntity, &ViewTarget, &DynamicUniformIndex<ScreenCopy>)>,
    pipeline: Res<CopyPipeline>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<ScreenCopy>>,
    device: Res<RenderDevice>,
    mut frames: ResMut<ShownFrames>,
    mut ctx: RenderContext,
) {
    let (main, target, index) = view.into_inner();
    let format = target.main_texture_format();
    let Some(render_pipeline) = pipeline
        .by_format
        .get(&format)
        .and_then(|&id| cache.get_render_pipeline(id))
    else {
        return;
    };
    let Some(binding) = uniforms.uniforms().binding() else {
        return;
    };
    let write = target.post_process_write();
    let size = write.source_texture.size();
    let fresh = frames
        .0
        .get(main)
        .is_none_or(|s| s.size != size || s.format != format);
    if fresh {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("screen_shown"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        frames.0.insert(
            *main,
            Shown {
                texture,
                view,
                size,
                format,
            },
        );
    }
    let shown = &frames.0[main];
    let encoder = ctx.command_encoder();
    // A new shown buffer starts as the frame itself.
    if fresh {
        encoder.copy_texture_to_texture(texel(write.source_texture), texel(&shown.texture), size);
    }
    let bind_group = device.create_bind_group(
        "screen_copy",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((write.source, &shown.view, &pipeline.sampler, binding)),
    );
    {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("screen_copy"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: write.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(render_pipeline);
        pass.set_bind_group(0, &bind_group, &[index.index()]);
        pass.draw(0..3, 0..1);
    }
    // What is shown now is the next frame's shown buffer.
    encoder.copy_texture_to_texture(
        texel(write.destination_texture),
        texel(&shown.texture),
        size,
    );
}

fn texel(texture: &Texture) -> TexelCopyTextureInfo<'_> {
    TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: Default::default(),
        aspect: Default::default(),
    }
}
