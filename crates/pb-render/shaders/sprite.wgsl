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
    return tex_color * input.color;
}
