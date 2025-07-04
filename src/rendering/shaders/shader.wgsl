@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4 {
    let x = f32(in_vertex_index) * 0.5 - 0.5;
    let y = f32(in_vertex_index % 2) * 0.5 - 0.5;
    return vec4(x, y, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4 {
    return vec4(1.0, 0.0, 0.0, 1.0);
}
