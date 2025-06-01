use wgpu::{Device, Queue, Trace};

pub struct RenderContext {
    device: wgpu::Device,
    queue: wgpu::Queue,

    pub rt_render_target: Option<wgpu::Texture>,
    pub postprocess_target: Option<wgpu::Texture>,
}

impl RenderContext {
    pub async fn new() -> Result<Self, wgpu::RequestDeviceError> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();

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

        Ok(Self {
            device,
            queue,
            rt_render_target: None,
            postprocess_target: None,
        })
    }

    pub fn from_existing(device: Device, queue: Queue) -> Self {
        Self {
            device,
            queue,
            rt_render_target: None,
            postprocess_target: None,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
