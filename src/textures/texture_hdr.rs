use super::Texture2;
use crate::render::RenderContext;
use anyhow::Result;
use std::{cell::RefCell, error::Error, rc::Rc, slice};

pub struct TextureHdr {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl TextureHdr {
    pub fn new(
        context: Rc<RefCell<RenderContext>>,
        data: &[f32],
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let bcontext = context.borrow();
        let device = bcontext.device();
        let queue = bcontext.queue();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            unsafe { slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) },
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 16),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Self { texture, view })
    }
}

impl Texture2 for TextureHdr {
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}
