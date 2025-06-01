// https://jinleili.github.io/learn-wgpu-zh/beginner/tutorial1-window
use crate::{
    config::Config,
    render::{CopyToScreen, PostProcessor, RenderContext, Renderer},
    scene::{build_scene, Scene},
};
use anyhow::Result;
use clap::Parser;
use indicatif::ProgressBar;
use log::{debug, error, info};
use parking_lot::Mutex;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use wgpu::{
    include_wgsl, BindGroup, Device, MultisampleState, PrimitiveState, RenderPipeline, Trace,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct WgpuApp {
    /// 避免窗口被释放
    #[allow(unused)]
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    size_changed: bool,

    renderer: Renderer,
    post_processor: PostProcessor,
    copy_processor: CopyToScreen,

    scene: Scene,
}

impl WgpuApp {
    async fn new(window: Arc<Window>) -> Result<Self> {
        // Setup windowing
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                    | wgpu::Features::PUSH_CONSTANTS
                    | wgpu::Features::TEXTURE_BINDING_ARRAY
                    | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
                required_limits: wgpu::Limits {
                    max_bind_groups: 5,
                    max_push_constant_size: 4,
                    max_texture_dimension_2d: 4096,
                    max_binding_array_elements_per_shader_stage: 100,
                    ..wgpu::Limits::downlevel_defaults()
                },
                memory_hints: wgpu::MemoryHints::Performance,
                trace: Trace::Off,
            })
            .await?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        debug!("Supported formats: {:?}", caps.formats);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Setup RayTracing
        let config = Config::parse();
        let context = Rc::new(RefCell::new(RenderContext::from_existing(
            device.clone(),
            queue.clone(),
        )));
        let scene = build_scene(context.clone(), &config)?;

        let renderer = Renderer::new(context.clone(), &config, scene.bind_group_layout.clone());
        let post_processor = PostProcessor::new(context.clone(), &config);
        let copy_processor = CopyToScreen::new(context.clone(), device.clone(), &config);

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config: surface_config,
            size,
            size_changed: false,
            renderer,
            scene,
            post_processor,
            copy_processor,
        })
    }

    fn set_window_resized(&mut self, new_size: PhysicalSize<u32>) {
        if new_size != self.size {
            self.size = new_size;
            self.size_changed = true;
        }
    }

    fn resize_surface_if_needed(&mut self) {
        if self.size_changed {
            self.config.width = self.size.width;
            self.config.height = self.size.height;
            self.surface.configure(&self.device, &self.config);
            self.size_changed = false;
        }
    }

    fn render_pure_color(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }

        // submit 命令能接受任何实现了 IntoIter trait 的参数
        self.queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        let output = self.surface.get_current_texture()?;

        self.renderer.render(self.scene.bind_group.clone())?;
        self.post_processor.post_process();
        self.copy_processor.copy_to_screen(&output);

        Ok(())
    }
}

#[derive(Default)]
struct WgpuAppHandler {
    app: Arc<Mutex<Option<WgpuApp>>>,
    /// 错失的窗口大小变化
    ///
    /// # NOTE：
    /// 在 web 端，app 的初始化是异步的，当收到 resized 事件时，初始化可能还没有完成从而错过窗口 resized 事件，
    /// 当 app 初始化完成后会调用 `set_window_resized` 方法来补上错失的窗口大小变化事件。
    #[allow(dead_code)]
    missed_resize: Arc<Mutex<Option<PhysicalSize<u32>>>>,
}

impl ApplicationHandler for WgpuAppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // 恢复事件
        if self.app.as_ref().lock().is_some() {
            return;
        }

        let window_attributes = Window::default_attributes().with_title("prisma render");
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        let wgpu_app = pollster::block_on(WgpuApp::new(window)).unwrap();
        self.app.lock().replace(wgpu_app);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // 暂停事件
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let mut app_mutex_guard = self.app.lock();
        let Some(app) = app_mutex_guard.as_mut() else {
            return;
        };
        // 窗口事件
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                // 窗口大小改变
                if physical_size.width == 0 || physical_size.height == 0 {
                    // 处理最小化窗口的事件
                } else {
                    app.set_window_resized(physical_size);
                }
            }
            WindowEvent::KeyboardInput { .. } => {
                // 键盘事件
            }
            WindowEvent::RedrawRequested => {
                // surface重绘事件
                app.window.pre_present_notify();

                match app.render() {
                    Ok(_) => {}
                    // Err(wgpu::SurfaceError::Lost) => error!("Surface is lost"),
                    Err(e) => error!("Error: {e:?}"),
                }

                app.window.request_redraw();
            }
            _ => (),
        }
    }
}

pub fn show_window() -> Result<()> {
    let events_loop = EventLoop::new().unwrap();
    let mut app = WgpuAppHandler::default();
    Ok(events_loop.run_app(&mut app)?)
}
