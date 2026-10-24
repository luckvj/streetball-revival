// The original's full-screen untextured quads over its 3D picture, before
// the 2D (`crate::screen_fx::ScreenLayers`): each layer's GS blend on the
// frame's 8-bit values, in order.

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct ScreenLayers {
    colour: array<vec4<f32>, 4>,
    blend: vec4<f32>,
};

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> layers: ScreenLayers;

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
    var d = encode(clamp(sample.rgb, vec3<f32>(0.0), vec3<f32>(1.0)));
    for (var k = 0; k < 4; k += 1) {
        let s = layers.colour[k];
        // The quad's alpha, (A + 1) >> 1 on the GS's 0..128.
        let a = floor((s.a + 1.0) / 2.0);
        switch i32(layers.blend[k]) {
            // (Cs - Cd) * As >> 7 + Cd
            case 1: { d = floor((s.rgb - d) * a / 128.0) + d; }
            // Cs * As >> 7 + Cd
            case 2: { d = floor(s.rgb * a / 128.0) + d; }
            // Cd - Cs (FIX 0x80)
            case 3: { d = d - s.rgb; }
            default: {}
        }
        d = clamp(d, vec3<f32>(0.0), vec3<f32>(255.0));
    }
    return vec4<f32>(decode(d), sample.a);
}
