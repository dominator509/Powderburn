@group(0) @binding(0) var backdrop: texture_2d<f32>;
@group(0) @binding(1) var backdrop_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let source = textureSample(backdrop, backdrop_sampler, input.uv);
    let edge = distance(input.uv, vec2<f32>(0.5, 0.48));
    let vignette = 1.0 - smoothstep(0.35, 0.78, edge) * 0.42;
    let graded = source.rgb * vec3<f32>(1.02, 0.97, 0.88) * vignette;
    return vec4<f32>(graded, 1.0);
}
