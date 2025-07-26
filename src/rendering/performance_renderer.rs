use std::sync::Arc;
use wgpu::util::DeviceExt;
use crate::config::{
    PerformanceSettings, PerformanceSettingsHandle, RenderingSettings,
    TextureQuality, ShaderQuality, ShadowQuality
};
use crate::rendering::camera::{Camera, CameraController};
use crate::rendering::camera_uniform::CameraUniform;
use crate::assets::mesh::{Mesh, Vertex};
use crate::assets::texture::Texture;
use crate::rendering::light::Light;
use crate::rendering::shaders::{ShaderCache, ShaderKey, ShaderType};
use crate::utils::logging::{handle_wgpu_result, log_adapter_info, log_device_info};
use tracing::{info, warn, error, debug};
use cgmath::SquareMatrix;

/// Performance-aware renderer implementing dynamic quality scaling
pub struct PerformanceRenderer {
    // Core wgpu resources
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,

    // Performance configuration
    settings: PerformanceSettingsHandle,

    // Pipeline cache for different quality levels
    pipeline_cache: PipelineCache,

    // Rendering components
    pub camera: Camera,
    pub camera_controller: CameraController,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,

    // Hierarchical-Z Buffer (HZB) resources
    hzb_texture: Option<wgpu::Texture>,
    hzb_pipeline: Option<wgpu::ComputePipeline>,

    // Clustered forward shading resources
    cluster_buffer: Option<wgpu::Buffer>,
    light_indices_buffer: Option<wgpu::Buffer>,
    cluster_dimensions: (u32, u32, u32),

    // Performance monitoring
    frame_times: std::collections::VecDeque<std::time::Duration>,
    last_frame_start: std::time::Instant,

    // Texture sampler variants for different quality levels
    samplers: SamplerCache,
}

/// Cache for different pipeline variants based on quality settings
pub struct PipelineCache {
    pipelines: std::collections::HashMap<PipelineKey, Arc<wgpu::RenderPipeline>>,
    bind_group_layouts: BindGroupLayouts,
    shader_cache: ShaderCache,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct PipelineKey {
    shader_quality: ShaderQuality,
    shadow_quality: ShadowQuality,
    hzb_enabled: bool,
    cluster_dimensions: (u32, u32, u32),
}

pub struct BindGroupLayouts {
    camera: wgpu::BindGroupLayout,
    texture: wgpu::BindGroupLayout,
    light: wgpu::BindGroupLayout,
    cluster: Option<wgpu::BindGroupLayout>,
}

/// Texture samplers for different quality levels
pub struct SamplerCache {
    low_quality: wgpu::Sampler,      // Nearest filtering
    medium_quality: wgpu::Sampler,   // Bilinear filtering
    high_quality: wgpu::Sampler,     // Trilinear/anisotropic filtering
}

impl PerformanceRenderer {
    /// Create a new performance-aware renderer
    pub async fn new(
        window: Arc<winit::window::Window>,
        settings: PerformanceSettingsHandle,
    ) -> anyhow::Result<Self> {
        info!("Initializing performance-aware WGPU renderer");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            dx12_shader_compiler: wgpu::Dx12Compiler::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });

        let surface = unsafe {
            instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::from_window(&*window)
                    .map_err(|e| anyhow::anyhow!("Failed to create surface target: {}", e))?
            )?
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to find suitable adapter"))?;

        log_adapter_info(&adapter);

        // Request device features based on performance settings
        let required_features = Self::get_required_features(&settings);
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Performance Renderer Device"),
                    required_features,
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);
        log_device_info(&device);

        let size = window.inner_size();
        let config = Self::create_surface_config(&settings, size);
        surface.configure(&device, &config);

        // Initialize camera system
        let camera = Camera {
            eye: (0.0, 0.0, 3.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: config.width as f32 / config.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: settings.read().unwrap().rendering.draw_distance,
        };
        let camera_controller = CameraController::new(0.2);

        // Create uniform buffer
        let camera_uniform = CameraUniform {
            view_proj: camera.build_view_projection_matrix().into(),
            model: cgmath::Matrix4::identity().into(),
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layouts
        let bind_group_layouts = Self::create_bind_group_layouts(&device, &settings)?;

        // Create bind group for camera
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layouts.camera,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("Camera Bind Group"),
        });

        // Create samplers for different quality levels
        let samplers = Self::create_sampler_cache(&device);

        // Initialize pipeline cache
        let pipeline_cache = PipelineCache {
            pipelines: std::collections::HashMap::new(),
            bind_group_layouts,
            shader_cache: ShaderCache::new(),
        };

        // Initialize HZB resources if enabled
        let (hzb_texture, hzb_pipeline) = if settings.read().unwrap().rendering.hzb_enabled {
            Self::create_hzb_resources(&device, &config)?
        } else {
            (None, None)
        };

        // Initialize cluster resources
        let cluster_resolution = settings.read().unwrap().rendering.cluster_resolution;
        let cluster_dimensions = (cluster_resolution.0 as u32, cluster_resolution.1 as u32, cluster_resolution.2 as u32);
        let (cluster_buffer, light_indices_buffer) = 
            Self::create_cluster_resources(&device, cluster_dimensions)?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface,
            config,
            settings,
            pipeline_cache,
            camera,
            camera_controller,
            uniform_buffer,
            bind_group,
            hzb_texture,
            hzb_pipeline,
            cluster_buffer: Some(cluster_buffer),
            light_indices_buffer: Some(light_indices_buffer),
            cluster_dimensions,
            frame_times: std::collections::VecDeque::with_capacity(60),
            last_frame_start: std::time::Instant::now(),
            samplers,
        })
    }

    /// Get required wgpu features based on performance settings
    fn get_required_features(settings: &PerformanceSettingsHandle) -> wgpu::Features {
        let settings = settings.read().unwrap();
        let mut features = wgpu::Features::empty();

        // Enable additional features based on quality settings
        if settings.rendering.hzb_enabled {
            features |= wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        }

        // Enable compute shaders for clustered forward shading
        // Compute shaders are enabled by default in modern WGPU
        // features |= wgpu::Features::COMPUTE;

        features
    }

    /// Create surface configuration based on performance settings
    fn create_surface_config(
        settings: &PerformanceSettingsHandle,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> wgpu::SurfaceConfiguration {
        let settings = settings.read().unwrap();
        
        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width,
            height: size.height,
            present_mode: if settings.rendering.vsync_enabled {
                wgpu::PresentMode::Fifo
            } else {
                wgpu::PresentMode::Immediate
            },
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: if settings.rendering.target_fps > 60.0 { 1 } else { 2 },
        }
    }

    /// Create bind group layouts for different quality levels
    fn create_bind_group_layouts(
        device: &wgpu::Device,
        settings: &PerformanceSettingsHandle,
    ) -> anyhow::Result<BindGroupLayouts> {
        let camera = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Camera Bind Group Layout"),
        });

        let texture = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Texture Bind Group Layout"),
        });

        let light = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Light Bind Group Layout"),
        });

        // Create cluster bind group layout if clustering is enabled
        let cluster = if settings.read().unwrap().rendering.cluster_resolution != (0, 0, 0) {
            Some(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("Cluster Bind Group Layout"),
            }))
        } else {
            None
        };

        Ok(BindGroupLayouts {
            camera,
            texture,
            light,  
            cluster,
        })
    }

    /// Create sampler cache for different quality levels
    fn create_sampler_cache(device: &wgpu::Device) -> SamplerCache {
        let low_quality = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            label: Some("Low Quality Sampler"),
            ..Default::default()
        });

        let medium_quality = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            label: Some("Medium Quality Sampler"),
            ..Default::default()
        });

        let high_quality = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 16,
            label: Some("High Quality Sampler"),
            ..Default::default()
        });

        SamplerCache {
            low_quality,
            medium_quality,
            high_quality,
        }
    }

    /// Create HZB (Hierarchical-Z Buffer) resources
    fn create_hzb_resources(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> anyhow::Result<(Option<wgpu::Texture>, Option<wgpu::ComputePipeline>)> {
        // Create HZB texture with multiple mip levels
        let mip_levels = (config.width.max(config.height) as f32).log2() as u32 + 1;
        
        let hzb_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HZB Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Create compute pipeline for HZB generation (placeholder for now)
        let hzb_pipeline = None; // TODO: Implement HZB compute shader

        Ok((Some(hzb_texture), hzb_pipeline))
    }

    /// Create clustered forward shading resources
    fn create_cluster_resources(
        device: &wgpu::Device,
        cluster_dimensions: (u32, u32, u32),
    ) -> anyhow::Result<(wgpu::Buffer, wgpu::Buffer)> {
        let (x, y, z) = cluster_dimensions;
        let total_clusters = x * y * z;

        // Create cluster data buffer
        let cluster_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cluster Buffer"),
            size: (total_clusters * 32) as u64, // 32 bytes per cluster
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create light indices buffer
        let light_indices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Indices Buffer"),
            size: (total_clusters * 1024) as u64, // Up to 256 lights per cluster * 4 bytes
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok((cluster_buffer, light_indices_buffer))
    }

    /// Get appropriate sampler based on texture quality setting
    pub fn get_sampler_for_quality(&self, quality: TextureQuality) -> &wgpu::Sampler {
        match quality {
            TextureQuality::Low => &self.samplers.low_quality,
            TextureQuality::Medium => &self.samplers.medium_quality,
            TextureQuality::High | TextureQuality::Ultra => &self.samplers.high_quality,
        }
    }

    /// Update performance settings and recreate resources if needed
    pub fn update_settings(&mut self, new_settings: PerformanceSettings) -> anyhow::Result<()> {
        let current_settings = self.settings.read().unwrap();
        let needs_pipeline_rebuild = current_settings.rendering.shader_quality != new_settings.rendering.shader_quality
            || current_settings.rendering.shadow_quality != new_settings.rendering.shadow_quality
            || current_settings.rendering.hzb_enabled != new_settings.rendering.hzb_enabled
            || current_settings.rendering.cluster_resolution != new_settings.rendering.cluster_resolution;

        let needs_surface_reconfigure = current_settings.rendering.vsync_enabled != new_settings.rendering.vsync_enabled
            || current_settings.rendering.target_fps != new_settings.rendering.target_fps;

        drop(current_settings);
        *self.settings.write().unwrap() = new_settings;

        if needs_pipeline_rebuild {
            info!("Performance settings changed, rebuilding pipelines");
            self.pipeline_cache.pipelines.clear();
        }

        if needs_surface_reconfigure {
            info!("Surface settings changed, reconfiguring surface");
            let size = winit::dpi::PhysicalSize::new(self.config.width, self.config.height);
            self.config = Self::create_surface_config(&self.settings, size);
            self.surface.configure(&self.device, &self.config);
        }

        // Update camera far plane based on draw distance
        self.camera.zfar = self.settings.read().unwrap().rendering.draw_distance;

        Ok(())
    }

    /// Record frame time for performance monitoring
    pub fn record_frame_time(&mut self) {
        let now = std::time::Instant::now();
        let frame_time = now.duration_since(self.last_frame_start);
        self.last_frame_start = now;

        self.frame_times.push_back(frame_time);
        if self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }
    }

    /// Get current performance metrics
    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        if self.frame_times.is_empty() {
            return PerformanceMetrics::default();
        }

        let total_time: std::time::Duration = self.frame_times.iter().sum();
        let avg_frame_time = total_time / self.frame_times.len() as u32;
        let min_frame_time = *self.frame_times.iter().min().unwrap();
        let max_frame_time = *self.frame_times.iter().max().unwrap();

        PerformanceMetrics {
            avg_frame_time_ms: avg_frame_time.as_secs_f32() * 1000.0,
            min_frame_time_ms: min_frame_time.as_secs_f32() * 1000.0,
            max_frame_time_ms: max_frame_time.as_secs_f32() * 1000.0,
            avg_fps: 1000.0 / (avg_frame_time.as_secs_f32() * 1000.0),
        }
    }

    /// Resize the renderer
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) -> anyhow::Result<()> {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            // Update camera aspect ratio
            self.camera.aspect = new_size.width as f32 / new_size.height as f32;

            // Recreate HZB texture if enabled
            if self.settings.read().unwrap().rendering.hzb_enabled {
                let (hzb_texture, _) = Self::create_hzb_resources(&self.device, &self.config)?;
                self.hzb_texture = hzb_texture;
            }

            info!("Renderer resized to {}x{}", new_size.width, new_size.height);
        }
        Ok(())
    }

    /// Get or create a render pipeline for the current settings
    pub fn get_or_create_pipeline(&mut self) -> anyhow::Result<Arc<wgpu::RenderPipeline>> {
        let settings = self.settings.read().unwrap();
        let key = PipelineKey {
            shader_quality: settings.rendering.shader_quality,
            shadow_quality: settings.rendering.shadow_quality,
            hzb_enabled: settings.rendering.hzb_enabled,
            cluster_dimensions: (
                settings.rendering.cluster_resolution.0 as u32,
                settings.rendering.cluster_resolution.1 as u32,
                settings.rendering.cluster_resolution.2 as u32
            ),
        };
        drop(settings);

        if let Some(pipeline) = self.pipeline_cache.pipelines.get(&key) {
            return Ok(Arc::clone(pipeline));
        }

        // Create new pipeline
        let pipeline = self.create_render_pipeline(&key)?;
        let pipeline_arc = Arc::new(pipeline);
        self.pipeline_cache.pipelines.insert(key, Arc::clone(&pipeline_arc));
        
        Ok(pipeline_arc)
    }

    /// Create a render pipeline for the given settings
    fn create_render_pipeline(&mut self, key: &PipelineKey) -> anyhow::Result<wgpu::RenderPipeline> {
        let cluster_enabled = key.cluster_dimensions != (0, 0, 0);

        // Get or create vertex shader
        let vertex_shader_key = ShaderKey {
            shader_quality: key.shader_quality,
            shadow_quality: key.shadow_quality,
            hzb_enabled: key.hzb_enabled,
            cluster_enabled,
            shader_type: ShaderType::Vertex,
        };
        
        // Get or create fragment shader
        let fragment_shader_key = ShaderKey {
            shader_quality: key.shader_quality,
            shadow_quality: key.shadow_quality,
            hzb_enabled: key.hzb_enabled,
            cluster_enabled,
            shader_type: ShaderType::Fragment,
        };

        // Create shaders
        let vertex_shader = self.pipeline_cache.shader_cache
            .get_or_create_shader(&self.device, vertex_shader_key);
        let fragment_shader = self.pipeline_cache.shader_cache
            .get_or_create_shader(&self.device, fragment_shader_key);

        // Create pipeline layout
        let mut bind_group_layouts = vec![
            &self.pipeline_cache.bind_group_layouts.camera,
            &self.pipeline_cache.bind_group_layouts.texture,
            &self.pipeline_cache.bind_group_layouts.light,
        ];

        if cluster_enabled {
            if let Some(ref cluster_layout) = self.pipeline_cache.bind_group_layouts.cluster {
                bind_group_layouts.push(cluster_layout);
            }
        }

        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Performance Render Pipeline Layout"),
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("Performance Render Pipeline {:?}", key.shader_quality)),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        info!("Created render pipeline for quality: {:?}", key.shader_quality);
        Ok(pipeline)
    }

    /// Render a frame with the performance-aware pipeline
    pub fn render_frame(
        &mut self,
        meshes: &[&Mesh],
        textures: &[&Texture],
        lights: &[Light],
    ) -> anyhow::Result<()> {
        self.record_frame_time();

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost) => {
                warn!("Surface lost, skipping frame");
                return Ok(());
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                error!("Surface out of memory, skipping frame");
                return Ok(());
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Surface error: {:?}", e));
            }
        };

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Performance Render Encoder"),
        });

        // Update camera uniform
        let camera_uniform = CameraUniform {
            view_proj: self.camera.build_view_projection_matrix().into(),
            model: cgmath::Matrix4::identity().into(),
        };
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        // Get or create the appropriate pipeline
        let pipeline = self.get_or_create_pipeline()?;

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Performance Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None, // TODO: Add depth buffer
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);

            // Render meshes
            for (i, mesh) in meshes.iter().enumerate() {
                if i < textures.len() {
                    let texture = textures[i];
                    let texture_quality = self.settings.read().unwrap().rendering.texture_quality;
                    let sampler = self.get_sampler_for_quality(texture_quality);
                    
                    // Create texture bind group on the fly (TODO: cache these)
                    let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &self.pipeline_cache.bind_group_layouts.texture,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&texture.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(sampler),
                            },
                        ],
                        label: Some("Texture Bind Group"),
                    });

                    render_pass.set_bind_group(1, &texture_bind_group, &[]);
                }

                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        Ok(())
    }
}

impl PipelineCache {
    /// Clear the pipeline cache
    pub fn clear(&mut self) {
        self.pipelines.clear();
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> PipelineCacheStats {
        PipelineCacheStats {
            pipeline_count: self.pipelines.len(),
            shader_count: self.shader_cache.len(),
        }
    }
}

/// Pipeline cache statistics
#[derive(Debug, Clone)]
pub struct PipelineCacheStats {
    pub pipeline_count: usize,
    pub shader_count: usize,
}

/// Performance metrics for monitoring
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub avg_frame_time_ms: f32,
    pub min_frame_time_ms: f32,
    pub max_frame_time_ms: f32,
    pub avg_fps: f32,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            avg_frame_time_ms: 16.67, // 60 FPS
            min_frame_time_ms: 16.67,
            max_frame_time_ms: 16.67,
            avg_fps: 60.0,
        }
    }
}