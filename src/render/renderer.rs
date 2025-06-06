use super::{align, RenderContext};
use crate::config::{Config, Size};
use anyhow::Result;
use indicatif::ProgressBar;
use log::{debug, trace};
use std::{cell::RefCell, default::Default, error::Error, rc::Rc, sync::Arc};
use wgpu::{Texture, TextureUsages};

pub struct Renderer {
    context: Rc<RefCell<RenderContext>>,
    width: u32,
    height: u32,
    samples: u32,
    target_bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    render_target: wgpu::Texture,
}

#[derive(Clone)]
pub struct BindGroupLayoutSet {
    pub scene: wgpu::BindGroupLayout,
    pub primitive: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    pub texture: wgpu::BindGroupLayout,
}

#[derive(Clone)]
pub struct BindGroupSet {
    pub scene: wgpu::BindGroup,
    pub primitive: wgpu::BindGroup,
    pub material: wgpu::BindGroup,
    pub texture: wgpu::BindGroup,
}

impl Renderer {
    pub fn new(
        context: Rc<RefCell<RenderContext>>,
        config: &Config,
        bind_group_layout_set: BindGroupLayoutSet,
    ) -> Self {
        let bcontext = context.borrow();
        let device = bcontext.device();

        let Size { width, height } = config.size;
        let width = align(width, 16);
        let height = align(height, 16);

        let target_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &target_bind_group_layout,
                &bind_group_layout_set.scene,
                &bind_group_layout_set.primitive,
                &bind_group_layout_set.material,
                &bind_group_layout_set.texture,
            ],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::COMPUTE,
                range: 0..4,
            }],
        });

        let shader_module =
            device.create_shader_module(wgpu::include_wgsl!("../../shaders-generated/render.wgsl"));

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &[("MAX_DEPTH", config.depth as f64)],
                zero_initialize_workgroup_memory: true,
            },
            cache: None,
        });

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("RayTracing renderer render target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        std::mem::drop(bcontext);
        let mut mcontext = context.borrow_mut();
        mcontext.rt_render_target.replace(render_target.clone());

        Self {
            context: context.clone(),
            width,
            height,
            samples: config.samples,
            target_bind_group_layout,
            pipeline,
            render_target,
        }
    }

    pub fn render(&self, bind_group_set: BindGroupSet) -> Result<()> {
        let context = self.context.borrow();
        let device = context.device();
        let queue = context.queue();

        let view = self
            .render_target
            .create_view(&wgpu::TextureViewDescriptor::default());

        let output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.target_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });

        // let progress_bar = Arc::new(ProgressBar::new(self.samples as u64));

        for sample in 0..self.samples {
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

            {
                let sample: [u8; 4] = sample.to_ne_bytes();

                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.pipeline);
                compute_pass.set_bind_group(0, &output_bind_group, &[]);
                compute_pass.set_bind_group(1, &bind_group_set.scene, &[]);
                compute_pass.set_bind_group(2, &bind_group_set.primitive, &[]);
                compute_pass.set_bind_group(3, &bind_group_set.material, &[]);
                compute_pass.set_bind_group(4, &bind_group_set.texture, &[]);
                compute_pass.set_push_constants(0, &sample);
                compute_pass.dispatch_workgroups(self.width / 16, self.height / 16, 1);
            }

            // let progress_bar = progress_bar.clone();
            queue.submit(Some(encoder.finish()));
            // queue.on_submitted_work_done(move || progress_bar.inc(1));
        }

        device.poll(wgpu::MaintainBase::Wait)?;
        // progress_bar.finish_and_clear();

        Ok(())
    }

    pub fn render_target(&self) -> &wgpu::Texture {
        &self.render_target
    }
}
