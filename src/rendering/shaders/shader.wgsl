struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
};

struct LightUniform {
    position: vec3<f32>,
    color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) normal: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;

@group(1) @binding(1)
var s_diffuse: sampler;

@group(2) @binding(0)
var<uniform> light: LightUniform;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>
) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = uniforms.model * vec4(position, 1.0);
    let world_normal = (uniforms.model * vec4(normal, 0.0)).xyz;
    out.world_position = world_pos.xyz;
    out.normal = normalize(world_normal);
    out.tex_coords = tex_coords;
    out.clip_position = uniforms.view_proj * world_pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(light.position - in.world_position);
    let diffuse = max(dot(in.normal, light_dir), 0.0);
    let texture_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    let final_color = texture_color.rgb * (light.color * diffuse);
    return vec4<f32>(final_color, texture_color.a);
}
