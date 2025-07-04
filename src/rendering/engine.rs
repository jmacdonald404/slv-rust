use winit::{
    event::{Event, WindowEvent, StartCause},
    event_loop::{EventLoop, ControlFlow, ActiveEventLoop},
    window::{Window, WindowBuilder, WindowId},
    application::ApplicationHandler,
};

use wgpu::{Adapter, Device, Instance, Queue, Surface, util::DeviceExt};
use crate::rendering::camera::{Camera, CameraController};
use crate::rendering::camera_uniform::CameraUniform;
use cgmath::prelude::*;
use wgpu::VertexBufferLayout;

use crate::assets::manager::ResourceManager;
use crate::assets::mesh::Mesh;

use crate::rendering::light::{Light, LightUniform};

use tracing::{info, error};

pub struct Renderer<'a> {
    pub render_pipeline: &'a wgpu::RenderPipeline,
}

impl<'a> Renderer<'a> {
    pub fn new(render_pipeline: &'a wgpu::RenderPipeline) -> Self {
        Self { render_pipeline }
    }

    pub fn render_frame(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface: &wgpu::Surface,
        size: winit::dpi::PhysicalSize<u32>,
        mesh: &Mesh,
        bind_group: &wgpu::BindGroup,
        texture_bind_group: &wgpu::BindGroup,
        light_bind_group: &wgpu::BindGroup,
    ) {
        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost) => {
                // Surface lost, resizing should be handled by caller
                return;
            },
            Err(wgpu::SurfaceError::OutOfMemory) => {
                // Out of memory, exit should be handled by caller
                return;
            },
            Err(_) => {
                // Other errors, skip frame
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
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            render_pass.set_pipeline(self.render_pipeline);
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
    instance: Instance,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    surface: Surface<'a>,
    window: winit::window::Window,
    size: winit::dpi::PhysicalSize<u32>,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    camera: Camera,
    camera_controller: CameraController,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    resource_manager: ResourceManager<'a>,
    texture_bind_group: wgpu::BindGroup,
    mesh: Mesh,
    light: Light,
    light_uniform_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    renderer: Renderer<'a>,
}

impl<'a> RenderEngine<'a> {
    pub async fn new(event_loop: &EventLoop<()>) -> Self {
        let window = WindowBuilder::new()
            .with_title("slv-rust")
            .build(event_loop)
            .unwrap();

        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
            flags: wgpu::InstanceFlags::empty(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });

        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .filter(|f| f.is_srgb())
            .next()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let camera = Camera {
            eye: (0.0, 1.0, 2.0).into(),
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
                        min_binding_size: None,
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

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[crate::assets::mesh::Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
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
        });

        let resource_manager = ResourceManager::new(&device, &queue);
        let texture_path = std::path::Path::new("assets/textures/happy-tree.png");
        let mesh_path = std::path::Path::new("assets/meshes/default.obj");
        let mut resource_manager = resource_manager;
        resource_manager.load_texture(texture_path).await.unwrap();
        resource_manager.load_mesh(mesh_path).await.unwrap();
        let texture_ref = resource_manager.get_texture(texture_path.to_str().unwrap()).unwrap();
        let mesh_ref = resource_manager.get_mesh(mesh_path.to_str().unwrap()).unwrap();

        let light = Light {
            position: cgmath::Point3::new(0.0, 0.0, 0.0),
            color: cgmath::Vector3::new(1.0, 1.0, 1.0),
        };
        // TODO: Integrate lighting calculations into the render loop and update shader logic as needed.

        let light_uniform = light.to_uniform();

        let light_uniform_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Light Uniform Buffer"),
                contents: bytemuck::cast_slice(&[light_uniform]),
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
                        resource: wgpu::BindingResource::TextureView(&texture_ref.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&texture_ref.sampler),
                    }
                ],
                label: Some("texture_bind_group"),
            }
        );

        let renderer = Renderer::new(&render_pipeline);

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
            resource_manager,
            texture_bind_group,
            mesh: mesh_ref.clone(),
            light,
            light_uniform_buffer,
            light_bind_group,
            renderer,
        }
    }

    pub fn run(&mut self) {
        use winit::event::{Event, WindowEvent};
        use winit::event_loop::ControlFlow;
        use winit::event_loop::EventLoop;

        let event_loop = EventLoop::new();
        let mut last_light_position = self.light.position;
        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                Event::WindowEvent { event, window_id } if window_id == self.window.id() => {
                    if !self.camera_controller.process_events(&event) {
                        match event {
                            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                            WindowEvent::Resized(physical_size) => {
                                self.resize(physical_size);
                            },
                            _ => {},
                        }
                    }
                },
                Event::MainEventsCleared => {
                    self.camera_controller.update_camera(&mut self.camera);
                    self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[self.camera.build_view_projection_matrix().into()]));
                    // Update light position or color if needed
                    if self.light.position != last_light_position {
                        let light_uniform = self.light.to_uniform();
                        self.queue.write_buffer(&self.light_uniform_buffer, 0, bytemuck::cast_slice(&[light_uniform]));
                        last_light_position = self.light.position;
                    }
                    self.window.request_redraw();
                },
                Event::RedrawRequested(window_id) if window_id == self.window.id() => {
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
                },
                _ => {}
            }
        });
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        // TODO: implement resize logic
        // TODO: Integrate error handling and logging (see progress.md)
    }
}


