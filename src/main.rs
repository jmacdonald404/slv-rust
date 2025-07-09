use winit::event_loop::EventLoop;
use winit::event::{Event, WindowEvent};
use tracing::info;
use slv_rust::utils::logging::init_logging;
use wgpu::{Instance, Backends, SurfaceConfiguration, TextureUsages, PresentMode, CompositeAlphaMode};
use winit::window::Window;
use std::sync::Arc;

struct AppState<'a> {
    window: Arc<Window>,
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    egui_ctx: eframe::egui::Context,
}

impl<'a> AppState<'a> {
    async fn new(window: Window) -> Self {
        let window = Arc::new(window);
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let egui_ctx = eframe::egui::Context::default();

        // The window needs to be part of the state to ensure the surface is valid.
        // However, the surface has a lifetime tied to the window. To solve this,
        // we can transmute the surface to a 'static lifetime. This is safe as long
        // as the window is owned by the AppState and dropped along with the surface.
        Self {
            window,
            surface,
            device,
            queue,
            config,
            egui_ctx,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output_frame = self.surface.get_current_texture()?;
        let output_view = output_frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let full_output = self.egui_ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Hello from egui!");
                if ui.button("Click me!").clicked() {
                    info!("Button clicked!");
                }
            });
        });

        let clipped_primitives = self.egui_ctx.tessellate(full_output.shapes, self.window.scale_factor() as f32);

        for (id, image_delta) in &full_output.textures_delta.set {
            // self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }
        for id in &full_output.textures_delta.free {
            // self.egui_renderer.free_texture(id);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("egui UI Encoder"),
        });

        // self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &clipped_primitives, &screen_descriptor);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.2, b: 0.8, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // self.egui_renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        }

        self.queue.submit(Some(encoder.finish()));
        output_frame.present();

        Ok(())
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "My egui App",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

#[derive(Default)]
struct MyApp;

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Hello from eframe + egui!");
            if ui.button("Click me!").clicked() {
                println!("Button clicked!");
            }
        });
    }
}
