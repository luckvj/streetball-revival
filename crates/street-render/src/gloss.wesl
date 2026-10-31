// The players' second pass (`Texture_ToonShade_Skin`/`Texture_ToonShade2_Skin`
// on VU1, see crate::gloss): the court's specular map at a sphere-map
// coordinate from the view-space normal, blended over the unlit first pass by
// the GS on the frame's 8-bit values.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var gloss_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var gloss_sampler: sampler;

fn encode(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return round(clamp(select(high, low, c <= vec3<f32>(0.0031308)), vec3(0.0), vec3(1.0)) * 255.0);
}

fn decode(v: vec3<f32>) -> vec3<f32> {
    let c = v / 255.0;
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );
    let base = pbr_input.material.base_color;

    // The program turns the normal by the model-view-projection rows and
    // divides each axis by the projection's diagonal (VU 263..265), which
    // leaves the view-space normal; x and y are then scaled by 0.49 (y
    // negated, the texture's rows run down) before the length is taken.
    let n = (view.view_from_world * vec4(pbr_input.world_normal, 0.0)).xyz;
    let v = vec3(0.49 * n.x, -0.49 * n.y, n.z);
    let st = vec2(0.49) + 0.49 * v.xy * inverseSqrt(max(dot(v, v), 1e-12));
    let t = textureSample(gloss_texture, gloss_sampler, st) * 255.0;

    // Vertex colour 0x7f, TFX modulate, TCC on: (texel * 0x7f) >> 7. ALPHA_1
    // (A Cd, B 0, C As, D Cs): Cd * As >> 7, plus Cs, clamped (COLCLAMP).
    let a_s = floor(t.a * 127.0 / 128.0);
    let c_s = floor(t.rgb * 127.0 / 128.0);
    let c_d = encode(base.rgb);
    let c = min(floor(c_d * a_s / 128.0) + c_s, vec3(255.0));

    var out: FragmentOutput;
    out.color = vec4(decode(c), base.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
