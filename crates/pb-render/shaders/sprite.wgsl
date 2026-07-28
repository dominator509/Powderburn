// Sprite vertex shader
// Takes position, tex_coord, and color with MVP matrix

struct Uniforms {
    mvp: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@group(0) @binding(1) var texture: texture_2d<f32>;
@group(0) @binding(2) var sampler_state: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.mvp * vec4<f32>(input.position, 1.0);
    output.tex_coord = input.tex_coord;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(texture, sampler_state, input.tex_coord);
    let highest = max(tex_color.r, max(tex_color.g, tex_color.b));
    let lowest = min(tex_color.r, min(tex_color.g, tex_color.b));
    let neutral = 1.0 - smoothstep(0.04, 0.13, highest - lowest);
    let bright = smoothstep(0.72, 0.92, highest);
    let white_key = neutral * bright;
    let green_dominance = tex_color.g - max(tex_color.r, tex_color.b);
    let chroma_key =
        smoothstep(0.01, 0.08, green_dominance) * smoothstep(0.16, 0.45, tex_color.g);
    let keyed_alpha = 1.0 - max(white_key, chroma_key);

    if keyed_alpha < 0.04 {
        discard;
    }

    let tint = mix(vec3<f32>(1.0), input.color.rgb, 0.28);
    return vec4<f32>(tex_color.rgb * tint, tex_color.a * input.color.a * keyed_alpha);
}
