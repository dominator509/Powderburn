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
        // Movement: fill the entire diamond so the pointer reads as one tile.
        // Dots and a perimeter remain as secondary non-color cues.
        let diamond = abs(input.local.x - 0.5) + abs(input.local.y - 0.5);
        let border = smoothstep(0.38, 0.49, diamond);
        let cell = fract(input.local * 8.0) - vec2<f32>(0.5);
        let dots = 1.0 - step(0.16, length(cell));
        ink = max(border, dots);
        return vec4<f32>(input.color.rgb, input.color.a * mix(0.72, 1.0, ink));
    } else if input.pattern < 4.5 {
        // Targeting: fill the same single-tile diamond and retain an aiming
        // ring plus perimeter as secondary non-color cues.
        let diamond = abs(input.local.x - 0.5) + abs(input.local.y - 0.5);
        let border = smoothstep(0.38, 0.49, diamond);
        let aiming_ring = 1.0 - step(0.045, abs(diamond - 0.24));
        ink = max(border, aiming_ring);
        return vec4<f32>(input.color.rgb, input.color.a * mix(0.76, 1.0, ink));
    } else if input.pattern < 5.5 {
        ink = step(0.58, fract(input.local.y * 10.0));
    } else if input.pattern < 6.5 {
        let cell = floor(input.local * 10.0);
        ink = select(0.15, 1.0, i32(cell.x + cell.y) % 2 == 0);
    } else if input.pattern < 7.5 {
        // Legal one-tile move: calm blue fill with a clear diamond perimeter.
        let diamond = abs(input.local.x - 0.5) + abs(input.local.y - 0.5);
        let border = smoothstep(0.40, 0.49, diamond);
        return vec4<f32>(input.color.rgb, input.color.a * mix(0.58, 1.0, border));
    } else {
        // Two-tile sprint: red hatch communicates the farther reach and that
        // choosing this destination commits the actor's remaining turn.
        let diamond = abs(input.local.x - 0.5) + abs(input.local.y - 0.5);
        let border = smoothstep(0.40, 0.49, diamond);
        let hatch = step(0.64, fract((input.local.x + input.local.y) * 8.0));
        ink = max(border, hatch);
        return vec4<f32>(input.color.rgb, input.color.a * mix(0.48, 1.0, ink));
    }
    return vec4<f32>(input.color.rgb, input.color.a * mix(0.18, 1.0, ink));
}
