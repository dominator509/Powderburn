// Overlay vertex shader with alpha blending
// Matches the smoke shader structure

struct Uniforms {
    mvp: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) local: vec2<f32>,
    @location(3) pattern: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local: vec2<f32>,
    @location(2) pattern: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.mvp * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.local = input.local;
    output.pattern = input.pattern;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if input.pattern < 0.5 {
        return input.color;
    }
    var ink = 1.0;
    if input.pattern < 1.5 {
        let primary = step(0.48, fract((input.local.x + input.local.y) * 9.0));
        let cross = step(0.58, fract((input.local.x - input.local.y) * 9.0));
        ink = max(primary, cross);
    } else if input.pattern < 2.5 {
        ink = step(0.48, fract((input.local.x + input.local.y) * 9.0));
    } else if input.pattern < 3.5 {
        let cell = fract(input.local * 8.0) - vec2<f32>(0.5);
        ink = 1.0 - step(0.16, length(cell));
    } else if input.pattern < 4.5 {
        let diamond = abs(input.local.x - 0.5) + abs(input.local.y - 0.5);
        ink = 1.0 - step(0.055, abs(diamond - 0.32));
    } else if input.pattern < 5.5 {
        ink = step(0.58, fract(input.local.y * 10.0));
    } else {
        let cell = floor(input.local * 10.0);
        ink = select(0.15, 1.0, i32(cell.x + cell.y) % 2 == 0);
    }
    return vec4<f32>(input.color.rgb, input.color.a * mix(0.18, 1.0, ink));
}
