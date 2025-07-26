use crate::config::{ShaderQuality, ShadowQuality};

/// Generate shader source based on quality settings
pub struct ShaderGenerator;

impl ShaderGenerator {
    /// Generate vertex shader source based on quality settings
    pub fn generate_vertex_shader(
        shader_quality: ShaderQuality,
        shadow_quality: ShadowQuality,
        hzb_enabled: bool,
    ) -> String {
        let mut shader = Self::base_vertex_shader();

        // Add quality-specific features
        match shader_quality {
            ShaderQuality::Simplified => {
                // Use simplified calculations
                shader.push_str("\n// Simplified quality mode\n");
            }
            ShaderQuality::Standard => {
                // Standard vertex processing
                shader.push_str("\n// Standard quality mode\n");
            }
            ShaderQuality::Enhanced => {
                // Enhanced vertex processing with additional features
                shader.push_str("\n// Enhanced quality mode\n");
                shader.push_str("// Additional vertex attributes for enhanced quality\n");
            }
        }

        // Add shadow mapping support
        if shadow_quality != ShadowQuality::Off {
            shader.push_str(&Self::shadow_vertex_extensions(shadow_quality));
        }

        // Add HZB support
        if hzb_enabled {
            shader.push_str(&Self::hzb_vertex_extensions());
        }

        shader
    }

    /// Generate fragment shader source based on quality settings
    pub fn generate_fragment_shader(
        shader_quality: ShaderQuality,
        shadow_quality: ShadowQuality,
        hzb_enabled: bool,
        cluster_enabled: bool,
    ) -> String {
        let mut shader = Self::base_fragment_shader_header();

        // Add bindings based on features
        shader.push_str(&Self::texture_bindings());
        shader.push_str(&Self::light_bindings());

        if shadow_quality != ShadowQuality::Off {
            shader.push_str(&Self::shadow_bindings(shadow_quality));
        }

        if cluster_enabled {
            shader.push_str(&Self::cluster_bindings());
        }

        // Add main fragment function
        shader.push_str(&Self::fragment_main_start());

        // Generate lighting calculations based on quality
        match shader_quality {
            ShaderQuality::Simplified => {
                shader.push_str(&Self::simplified_lighting(shadow_quality, cluster_enabled));
            }
            ShaderQuality::Standard => {
                shader.push_str(&Self::standard_lighting(shadow_quality, cluster_enabled));
            }
            ShaderQuality::Enhanced => {
                shader.push_str(&Self::enhanced_lighting(shadow_quality, cluster_enabled));
            }
        }

        shader.push_str(&Self::fragment_main_end());
        shader
    }

    /// Base vertex shader structure
    fn base_vertex_shader() -> String {
        r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    view_pos: vec3<f32>,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) view_position: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let world_pos = uniforms.model * vec4(vertex.position, 1.0);
    let world_normal = normalize((uniforms.model * vec4(vertex.normal, 0.0)).xyz);
    
    out.world_position = world_pos.xyz;
    out.world_normal = world_normal;
    out.tex_coords = vertex.tex_coords;
    out.view_position = uniforms.view_pos;
    out.clip_position = uniforms.view_proj * world_pos;
    
    return out;
}
"#.to_string()
    }

    /// Base fragment shader header
    fn base_fragment_shader_header() -> String {
        r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) view_position: vec3<f32>,
};

struct LightUniform {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
};
"#.to_string()
    }

    /// Texture bindings
    fn texture_bindings() -> String {
        r#"
@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;

@group(1) @binding(1)
var s_diffuse: sampler;
"#.to_string()
    }

    /// Light bindings
    fn light_bindings() -> String {
        r#"
@group(2) @binding(0)
var<uniform> lights: array<LightUniform, 64>;
"#.to_string()
    }

    /// Shadow mapping bindings
    fn shadow_bindings(shadow_quality: ShadowQuality) -> String {
        match shadow_quality {
            ShadowQuality::Off => String::new(),
            ShadowQuality::Low => r#"
@group(3) @binding(0)
var t_shadow: texture_depth_2d;

@group(3) @binding(1)
var s_shadow: sampler_comparison;
"#.to_string(),
            ShadowQuality::Medium | ShadowQuality::High => r#"
@group(3) @binding(0)
var t_shadow: texture_depth_2d_array;

@group(3) @binding(1)
var s_shadow: sampler_comparison;

@group(3) @binding(2)
var<uniform> shadow_matrices: array<mat4x4<f32>, 4>;
"#.to_string(),
            ShadowQuality::Ultra => r#"
@group(3) @binding(0)
var t_shadow: texture_depth_2d_array;

@group(3) @binding(1)
var s_shadow: sampler_comparison;

@group(3) @binding(2)
var<uniform> shadow_matrices: array<mat4x4<f32>, 8>;

@group(3) @binding(3)
var t_shadow_variance: texture_2d_array<f32>;
"#.to_string(),
        }
    }

    /// Clustered forward shading bindings
    fn cluster_bindings() -> String {
        r#"
struct ClusterData {
    min_bounds: vec3<f32>,
    max_bounds: vec3<f32>,
    light_count: u32,
    light_offset: u32,
};

@group(4) @binding(0)
var<storage, read> clusters: array<ClusterData>;

@group(4) @binding(1)
var<storage, read> light_indices: array<u32>;
"#.to_string()
    }

    /// Fragment main function start
    fn fragment_main_start() -> String {
        r#"
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    var final_color = vec3<f32>(0.0);
    
    let normal = normalize(in.world_normal);
    let view_dir = normalize(in.view_position - in.world_position);
"#.to_string()
    }

    /// Simplified lighting calculation
    fn simplified_lighting(shadow_quality: ShadowQuality, cluster_enabled: bool) -> String {
        if cluster_enabled {
            r#"
    // Simplified clustered lighting
    let cluster_index = compute_cluster_index(in.clip_position);
    let cluster = clusters[cluster_index];
    
    for (var i = 0u; i < min(cluster.light_count, 4u); i = i + 1u) {
        let light_index = light_indices[cluster.light_offset + i];
        let light = lights[light_index];
        
        let light_dir = normalize(light.position - in.world_position);
        let diffuse = max(dot(normal, light_dir), 0.0);
        final_color += light.color * light.intensity * diffuse * albedo.rgb;
    }
"#.to_string()
        } else {
            r#"
    // Simplified forward lighting (max 8 lights)
    for (var i = 0u; i < 8u; i = i + 1u) {
        let light = lights[i];
        if (light.intensity <= 0.0) { break; }
        
        let light_dir = normalize(light.position - in.world_position);
        let diffuse = max(dot(normal, light_dir), 0.0);
        final_color += light.color * light.intensity * diffuse * albedo.rgb;
    }
"#.to_string()
        }
    }

    /// Standard lighting calculation
    fn standard_lighting(shadow_quality: ShadowQuality, cluster_enabled: bool) -> String {
        let mut lighting = if cluster_enabled {
            r#"
    // Standard clustered lighting with Blinn-Phong
    let cluster_index = compute_cluster_index(in.clip_position);
    let cluster = clusters[cluster_index];
    
    for (var i = 0u; i < cluster.light_count; i = i + 1u) {
        let light_index = light_indices[cluster.light_offset + i];
        let light = lights[light_index];
        
        let light_dir = normalize(light.position - in.world_position);
        let distance = length(light.position - in.world_position);
        let attenuation = 1.0 / (1.0 + 0.09 * distance + 0.032 * distance * distance);
        
        // Diffuse
        let diffuse = max(dot(normal, light_dir), 0.0);
        
        // Specular (Blinn-Phong)
        let half_dir = normalize(light_dir + view_dir);
        let specular = pow(max(dot(normal, half_dir), 0.0), 32.0);
        
        let light_contribution = (diffuse + specular * 0.3) * light.color * light.intensity * attenuation;
        final_color += light_contribution * albedo.rgb;
    }
"#.to_string()
        } else {
            r#"
    // Standard forward lighting with Blinn-Phong (max 32 lights)
    for (var i = 0u; i < 32u; i = i + 1u) {
        let light = lights[i];
        if (light.intensity <= 0.0) { break; }
        
        let light_dir = normalize(light.position - in.world_position);
        let distance = length(light.position - in.world_position);
        let attenuation = 1.0 / (1.0 + 0.09 * distance + 0.032 * distance * distance);
        
        // Diffuse
        let diffuse = max(dot(normal, light_dir), 0.0);
        
        // Specular (Blinn-Phong)
        let half_dir = normalize(light_dir + view_dir);
        let specular = pow(max(dot(normal, half_dir), 0.0), 32.0);
        
        let light_contribution = (diffuse + specular * 0.3) * light.color * light.intensity * attenuation;
        final_color += light_contribution * albedo.rgb;
    }
"#.to_string()
        };

        // Add shadow mapping for standard quality
        if shadow_quality != ShadowQuality::Off {
            lighting.push_str(&Self::shadow_sampling(shadow_quality));
        }

        lighting
    }

    /// Enhanced lighting calculation
    fn enhanced_lighting(shadow_quality: ShadowQuality, cluster_enabled: bool) -> String {
        let mut lighting = if cluster_enabled {
            r#"
    // Enhanced clustered lighting with PBR
    let cluster_index = compute_cluster_index(in.clip_position);
    let cluster = clusters[cluster_index];
    
    // Enhanced material properties
    let metallic = 0.0;
    let roughness = 0.5;
    let f0 = mix(vec3<f32>(0.04), albedo.rgb, metallic);
    
    for (var i = 0u; i < cluster.light_count; i = i + 1u) {
        let light_index = light_indices[cluster.light_offset + i];
        let light = lights[light_index];
        
        let light_dir = normalize(light.position - in.world_position);
        let distance = length(light.position - in.world_position);
        let attenuation = 1.0 / (distance * distance);
        
        // Cook-Torrance BRDF
        let halfway = normalize(view_dir + light_dir);
        let ndf = distribution_ggx(normal, halfway, roughness);
        let g = geometry_smith(normal, view_dir, light_dir, roughness);
        let f = fresnel_schlick(max(dot(halfway, view_dir), 0.0), f0);
        
        let numerator = ndf * g * f;
        let denominator = 4.0 * max(dot(normal, view_dir), 0.0) * max(dot(normal, light_dir), 0.0) + 0.0001;
        let specular = numerator / denominator;
        
        let ks = f;
        let kd = (1.0 - ks) * (1.0 - metallic);
        
        let ndotl = max(dot(normal, light_dir), 0.0);
        let light_contribution = (kd * albedo.rgb / 3.14159 + specular) * light.color * light.intensity * attenuation * ndotl;
        final_color += light_contribution;
    }
"#.to_string()
        } else {
            r#"
    // Enhanced forward lighting with PBR (max 64 lights)
    let metallic = 0.0;
    let roughness = 0.5;
    let f0 = mix(vec3<f32>(0.04), albedo.rgb, metallic);
    
    for (var i = 0u; i < 64u; i = i + 1u) {
        let light = lights[i];
        if (light.intensity <= 0.0) { break; }
        
        let light_dir = normalize(light.position - in.world_position);
        let distance = length(light.position - in.world_position);
        let attenuation = 1.0 / (distance * distance);
        
        // Cook-Torrance BRDF
        let halfway = normalize(view_dir + light_dir);
        let ndf = distribution_ggx(normal, halfway, roughness);
        let g = geometry_smith(normal, view_dir, light_dir, roughness);
        let f = fresnel_schlick(max(dot(halfway, view_dir), 0.0), f0);
        
        let numerator = ndf * g * f;
        let denominator = 4.0 * max(dot(normal, view_dir), 0.0) * max(dot(normal, light_dir), 0.0) + 0.0001;
        let specular = numerator / denominator;
        
        let ks = f;
        let kd = (1.0 - ks) * (1.0 - metallic);
        
        let ndotl = max(dot(normal, light_dir), 0.0);
        let light_contribution = (kd * albedo.rgb / 3.14159 + specular) * light.color * light.intensity * attenuation * ndotl;
        final_color += light_contribution;
    }
"#.to_string()
        };

        // Add PBR helper functions
        lighting.push_str(&Self::pbr_helper_functions());

        // Add shadow mapping for enhanced quality
        if shadow_quality != ShadowQuality::Off {
            lighting.push_str(&Self::shadow_sampling(shadow_quality));
        }

        lighting
    }

    /// Shadow sampling code
    fn shadow_sampling(shadow_quality: ShadowQuality) -> String {
        match shadow_quality {
            ShadowQuality::Off => String::new(),
            ShadowQuality::Low => r#"
    // Simple shadow sampling
    // TODO: Implement shadow coordinate transformation and sampling
"#.to_string(),
            ShadowQuality::Medium => r#"
    // PCF shadow sampling
    // TODO: Implement percentage-closer filtering
"#.to_string(),
            ShadowQuality::High => r#"
    // PCSS shadow sampling
    // TODO: Implement percentage-closer soft shadows
"#.to_string(),
            ShadowQuality::Ultra => r#"
    // Variance shadow mapping
    // TODO: Implement variance shadow mapping with VSM
"#.to_string(),
        }
    }

    /// PBR helper functions for enhanced lighting
    fn pbr_helper_functions() -> String {
        r#"
fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let ndoth = max(dot(n, h), 0.0);
    let ndoth2 = ndoth * ndoth;
    
    let num = a2;
    let denom = ndoth2 * (a2 - 1.0) + 1.0;
    return num / (3.14159 * denom * denom);
}

fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    
    let num = ndotv;
    let denom = ndotv * (1.0 - k) + k;
    
    return num / denom;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let ndotv = max(dot(n, v), 0.0);
    let ndotl = max(dot(n, l), 0.0);
    let ggx2 = geometry_schlick_ggx(ndotv, roughness);
    let ggx1 = geometry_schlick_ggx(ndotl, roughness);
    
    return ggx1 * ggx2;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}
"#.to_string()
    }

    /// Cluster index computation
    fn cluster_index_computation() -> String {
        r#"
fn compute_cluster_index(clip_pos: vec4<f32>) -> u32 {
    // TODO: Implement cluster index computation based on screen position and depth
    return 0u;
}
"#.to_string()
    }

    /// Fragment main function end
    fn fragment_main_end() -> String {
        r#"
    
    // Add ambient lighting
    final_color += albedo.rgb * 0.03;
    
    // Tone mapping and gamma correction
    final_color = final_color / (final_color + 1.0);
    final_color = pow(final_color, vec3<f32>(1.0/2.2));
    
    return vec4<f32>(final_color, albedo.a);
}
"#.to_string()
    }

    /// Shadow vertex extensions
    fn shadow_vertex_extensions(shadow_quality: ShadowQuality) -> String {
        match shadow_quality {
            ShadowQuality::Off => String::new(),
            _ => r#"
    // Shadow mapping vertex extensions
    @location(4) shadow_coord: vec4<f32>,
"#.to_string(),
        }
    }

    /// HZB vertex extensions
    fn hzb_vertex_extensions() -> String {
        r#"
    // HZB vertex extensions
    @location(5) hzb_coord: vec2<f32>,
"#.to_string()
    }
}

/// Shader compilation cache
pub struct ShaderCache {
    cache: std::cell::RefCell<std::collections::HashMap<ShaderKey, wgpu::ShaderModule>>,
}

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct ShaderKey {
    pub shader_quality: ShaderQuality,
    pub shadow_quality: ShadowQuality,
    pub hzb_enabled: bool,
    pub cluster_enabled: bool,
    pub shader_type: ShaderType,
}

#[derive(Hash, Eq, PartialEq, Clone)]
pub enum ShaderType {
    Vertex,
    Fragment,
}

impl ShaderCache {
    pub fn new() -> Self {
        Self {
            cache: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }

    pub fn get_or_create_shader(
        &self,
        device: &wgpu::Device,
        key: ShaderKey,
    ) -> wgpu::ShaderModule {
        // Generate the shader source
        let source = match key.shader_type {
            ShaderType::Vertex => ShaderGenerator::generate_vertex_shader(
                key.shader_quality,
                key.shadow_quality,
                key.hzb_enabled,
            ),
            ShaderType::Fragment => ShaderGenerator::generate_fragment_shader(
                key.shader_quality,
                key.shadow_quality,
                key.hzb_enabled,
                key.cluster_enabled,
            ),
        };

        // Create and return the shader module
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{:?} Shader {:?}", key.shader_type, key.shader_quality)),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        })
    }
}