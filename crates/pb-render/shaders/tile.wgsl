// Tile vertex shader
// Takes position, color, and MVP matrix, outputs color to fragment shader

struct Uniforms {
    mvp: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var terrain_atlas: texture_2d<f32>;
@group(0) @binding(2) var terrain_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coord: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_position: vec2<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) elevation: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.mvp * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.world_position = input.position.xy;
    output.tex_coord = input.tex_coord;
    output.elevation = input.position.z;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let grit_seed =
        fract(input.world_position.x * 0.1031 + input.world_position.y * 0.11369);
    let grit = (grit_seed - 0.5) * 0.034;

    let diagonal_a = abs(fract((input.world_position.x + input.world_position.y * 2.0) / 64.0) - 0.5);
    let diagonal_b = abs(fract((input.world_position.x - input.world_position.y * 2.0) / 64.0) - 0.5);
    let seam = 1.0 - smoothstep(0.455, 0.5, max(diagonal_a, diagonal_b));
    let bevel = mix(0.93, 1.02, seam);

    // The CPU uploads a triangle-filtered presentation atlas, so one sample
    // retains the authored texture without a nine-tap blur in every fragment.
    let material = textureSample(terrain_atlas, terrain_sampler, input.tex_coord);

    let column = min(u32(input.tex_coord.x * 4.0), 3u);
    let row = min(u32(input.tex_coord.y * 2.0), 1u);
    let material_index = row * 4u + column;
    var base = vec3<f32>(0.38, 0.43, 0.24);
    switch material_index {
        case 1u: { base = vec3<f32>(0.45, 0.34, 0.22); }
        case 2u: { base = vec3<f32>(0.25, 0.48, 0.56); }
        case 3u: { base = vec3<f32>(0.31, 0.42, 0.25); }
        case 4u: { base = vec3<f32>(0.25, 0.31, 0.18); }
        case 5u: { base = vec3<f32>(0.75, 0.80, 0.82); }
        case 6u: { base = vec3<f32>(0.58, 0.38, 0.22); }
        case 7u: { base = vec3<f32>(0.37, 0.37, 0.34); }
        default: {}
    }
    // The authored atlas is the battlefield's primary surface, not a faint
    // tint beneath a flat procedural color.
    let graded = mix(base, material.rgb, 0.74);
    let relief_light = clamp(1.0 + input.elevation * 0.035, 0.84, 1.14);
    return vec4<f32>(
        graded * input.color.rgb * (bevel + grit) * relief_light,
        material.a * input.color.a
    );
}
