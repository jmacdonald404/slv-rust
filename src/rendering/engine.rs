use winit::{
    event::*,
    window::Window,
};
use wgpu::util::DeviceExt;
use crate::rendering::camera::{Camera, CameraController};
use crate::rendering::camera_uniform::CameraUniform;
use crate::assets::manager::{ResourceManager, AssetLoader};
use crate::assets::mesh::{Mesh, Vertex, MeshLoader};
use crate::assets::texture::Texture;
use crate::rendering::light::{Light};
use crate::utils::logging::{handle_wgpu_result, log_adapter_info, log_device_info};
use std::sync::Arc;
use std::fmt;
use tracing::{info, warn, error, debug};
use cgmath::SquareMatrix;

pub struct State<'a> {
    pub renderer: Option<RenderEngine<'a>>,
    pub last_light_position: cgmath::Point3<f32>,
    pub window: Option<Arc<winit::window::Window>>,
}

pub struct Renderer {
    pub render_pipeline: Arc<wgpu::RenderPipeline>,
}

impl Renderer {
    pub fn new(render_pipeline: Arc<wgpu::RenderPipeline>) -> Self {
        Self { render_pipeline }
    }

    pub fn render_frame(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface: &wgpu::Surface,
        _size: winit::dpi::PhysicalSize<u32>,
        mesh: &Mesh,
        bind_group: &wgpu::BindGroup,
        texture_bind_group: &wgpu::BindGroup,
        light_bind_group: &wgpu::BindGroup,
    ) {
        debug!("Starting frame render");
        let frame = match surface.get_current_texture() {
            Ok(frame) => {
                debug!("Surface texture acquired successfully");
                frame
            },
            Err(wgpu::SurfaceError::Lost) => {
                warn!("Surface lost, skipping frame");
                return;
            },
            Err(wgpu::SurfaceError::OutOfMemory) => {
                error!("Surface out of memory, skipping frame");
                return;
            },
            Err(e) => {
                error!("Surface error: {:?}, skipping frame", e);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, bind_group, &[]); // Camera
            render_pass.set_bind_group(1, texture_bind_group, &[]); // Texture
            render_pass.set_bind_group(2, light_bind_group, &[]); // Light
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
        }
        queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

pub struct RenderEngine<'a> {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub surface: wgpu::Surface<'a>,
    pub window: Arc<winit::window::Window>,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub config: wgpu::SurfaceConfiguration,
    render_pipeline: Arc<wgpu::RenderPipeline>,
    pub camera: Camera,
    pub camera_controller: CameraController,
    pub uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    texture_bind_group: wgpu::BindGroup,
    mesh: Mesh,
    pub light: Light,
    pub light_uniform_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    renderer: Renderer,
}

impl<'a> RenderEngine<'a> {
    pub async fn new(window: Arc<winit::window::Window>) -> Self {
        info!("Initializing WGPU render engine");
        
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        info!("WGPU instance created successfully");
        
        let surface = handle_wgpu_result(
            instance.create_surface(window.clone()),
            "create_surface"
        ).expect("Failed to create surface");
        
        info!("WGPU surface created successfully");
        
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");
            
        info!("WGPU adapter selected");
        log_adapter_info(&adapter);
        
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None
            )
            .await
            .expect("Failed to create device");
            
        info!("WGPU device and queue created successfully");
        log_device_info(&device);
        let device = Arc::new(device);
        let queue = Arc::new(queue);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        info!("Configuring WGPU surface with format: {:?}", config.format);
        surface.configure(&device, &config);
        info!("WGPU surface configured successfully");

        info!("Creating shader module");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });
        info!("Shader module created successfully");

        let camera = Camera {
            eye: (0.0, 0.0, 3.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: config.width as f32 / config.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };
        let camera_controller = CameraController::new(0.2);

        let camera_uniform = CameraUniform {
            view_proj: camera.build_view_projection_matrix().into(),
            model: cgmath::Matrix4::<f32>::identity().into(),
        };

        let uniform_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("camera_bind_group_layout"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }
            ],
            label: Some("camera_bind_group"),
        });

        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            label: Some("texture_bind_group_layout"),
        });

        let light_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(std::num::NonZeroU64::new((std::mem::size_of::<crate::rendering::light::LightUniform>() * 2) as u64).unwrap()),
                    },
                    count: None,
                }
            ],
            label: Some("light_bind_group_layout"),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &texture_bind_group_layout,
                &light_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        info!("Creating render pipeline");
        let render_pipeline = Arc::new(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        }));
        info!("Render pipeline created successfully");

        info!("Creating resource manager");
        let resource_manager = ResourceManager::new(Arc::clone(&device), Arc::clone(&queue));
        let mut resource_manager = resource_manager;
        
        // Create a fallback texture since assets don't exist
        info!("Creating fallback texture");
        
        // Create a simple 32x32 checkerboard pattern directly as a texture
        let texture_size = wgpu::Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 1,
        };
        let mut fallback_texture_data = Vec::new();
        for y in 0..32 {
            for x in 0..32 {
                let is_white = ((x / 4) + (y / 4)) % 2 == 0;
                if is_white {
                    fallback_texture_data.extend_from_slice(&[255, 255, 255, 255]); // White
                } else {
                    fallback_texture_data.extend_from_slice(&[128, 128, 128, 255]); // Gray
                }
            }
        }
        
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("fallback_texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &fallback_texture_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * 32), // 32 pixels * 4 bytes per pixel
                rows_per_image: Some(32),
            },
            texture_size,
        );
        
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        
        let fallback_texture = Texture { texture, view, sampler };
        
        info!("Fallback texture created successfully");
        
        // Create a simple mesh (already handled by the mesh loader)
        info!("Creating cube mesh");
        let mesh_ref = MeshLoader::new(Arc::clone(&device))
            .load(std::path::Path::new("cube"))
            .await
            .expect("Failed to create cube mesh");
        info!("Cube mesh created successfully");

        // Two lights: blue (back-left) and red (back-right)
        let lights = [
            Light {
                position: cgmath::Point3::new(-2.0, 2.0, 5.0), // back-left
                color: cgmath::Vector3::new(0.0, 0.0, 1.0),    // blue
            },
            Light {
                position: cgmath::Point3::new(2.0, 2.0, 5.0),  // back-right
                color: cgmath::Vector3::new(1.0, 0.0, 0.0),    // red
            },
        ];
        let light_uniforms = [lights[0].to_uniform(), lights[1].to_uniform()];
        let light_uniform_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Light Uniform Buffer"),
                contents: bytemuck::cast_slice(&light_uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );
        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: light_uniform_buffer.as_entire_binding(),
                }
            ],
            label: Some("light_bind_group"),
        });

        let texture_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&fallback_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&fallback_texture.sampler),
                    }
                ],
                label: Some("texture_bind_group"),
            }
        );

        let renderer = Renderer::new(Arc::clone(&render_pipeline));

        Self {
            instance,
            adapter,
            device,
            queue,
            surface,
            window,
            size,
            config,
            render_pipeline,
            camera,
            camera_controller,
            uniform_buffer,
            bind_group,
            texture_bind_group,
            mesh: mesh_ref,
            light: lights[0].clone(),
            light_uniform_buffer,
            light_bind_group,
            renderer,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn render_frame(&mut self) {
        self.renderer.render_frame(
            &self.device,
            &self.queue,
            &self.surface,
            self.size,
            &self.mesh,
            &self.bind_group,
            &self.texture_bind_group,
            &self.light_bind_group,
        );
    }
}




