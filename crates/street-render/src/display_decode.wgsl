// The 3D picture is drawn in the GS's own values (crate::screen_fx::DisplaySpace):
// every texel, vertex colour and blend as the display shows it. This turns
// those values into linear light for the passes after it and the 2D.

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct DisplaySpace {
    unused: vec4<f32>,
};

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: DisplaySpace;

fn decode(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(screen, screen_sampler, in.uv);
    return vec4<f32>(decode(clamp(sample.rgb, vec3<f32>(0.0), vec3<f32>(1.0))), sample.a);
}
