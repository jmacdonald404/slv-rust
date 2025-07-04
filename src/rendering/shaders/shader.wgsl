struct Uniforms {
    view_proj: mat4x4<f32>,
};

struct LightUniform {
    position: vec3<f32>,
    color: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(2) @binding(0)
var<uniform> light: LightUniform;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32, @location(0) position: vec3<f32>, @location(1) tex_coords: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4(position, 1.0);
    out.tex_coords = tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4 {
    // TODO: Refine/complete lighting logic as needed for correct shading
    let light_dir = normalize(light.position - in.world_position.xyz);
    let diffuse = max(dot(in.normal, light_dir), 0.0);
    let final_color = textureSample(t_diffuse, s_diffuse, in.tex_coords).rgb * (light.color * diffuse);
    return vec4(final_color, 1.0);
}
