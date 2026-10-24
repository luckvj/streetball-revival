// The frame copy (`crate::screen_fx::ScreenCopy`): the frame just drawn,
// its colour modulated, blended over the picture shown before it, as the
// original copies its drawn buffer into the shown one.

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct ScreenCopy {
    alpha: f32,
    colour: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var shown: texture_2d<f32>;
@group(0) @binding(2) var screen_sampler: sampler;
@group(0) @binding(3) var<uniform> copy: ScreenCopy;

fn encode(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return round(select(high, low, c <= vec3<f32>(0.0031308)) * 255.0);
}

fn decode(v: vec3<f32>) -> vec3<f32> {
    let c = v / 255.0;
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(screen, screen_sampler, in.uv);
    // An identity copy: the frame as drawn.
    if copy.alpha >= 128.0 && copy.colour == 128.0 {
        return sample;
    }
    let s = encode(clamp(sample.rgb, vec3<f32>(0.0), vec3<f32>(1.0)));
    let h = encode(clamp(textureSample(shown, screen_sampler, in.uv).rgb, vec3<f32>(0.0), vec3<f32>(1.0)));
    // The sprite's texture modulated by its colour (>> 7), clamped.
    let c = min(floor(s * copy.colour / 128.0), vec3<f32>(255.0));
    // (Cs - Cd) * As >> 7 + Cd
    let d = clamp(floor((c - h) * copy.alpha / 128.0) + h, vec3<f32>(0.0), vec3<f32>(255.0));
    return vec4<f32>(decode(d), sample.a);
}
