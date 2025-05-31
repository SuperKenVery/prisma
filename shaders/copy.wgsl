// Copy the output of compute shader onto screen

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) tex_coords: vec2f,
}

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    // A quad rectangle
    // 1    2
    // 3    4
    //
    // x: -1, 1, -1, 1
    // y: 1, 1, -1, -1
    var out: VertexOutput;
    let x = f32(in_vertex_index & 1u) * 2 - 1;
    let y = - f32((in_vertex_index & 2u) * 2 - 1);

    out.clip_position = vec4f(x, y, 0.0, 1.0);
    out.tex_coords = vec2f((x+1)/2, (y+1)/2);
    return out;
}

@group(0) @binding(0) var t_compute: texture_2d<f32>;
@group(0) @binding(1) var s_compute: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    return textureSample(t_compute, s_compute, in.tex_coords);
}
