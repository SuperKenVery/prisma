use self::bvh::Bvh;
use crate::{
    config::Config,
    core::{Aabb3, Triangle},
    materials::Materials,
    primitives::Primitives,
    render::{BindGroupLayoutSet, BindGroupSet, RenderContext},
    textures::Textures,
};
use anyhow::Result;
use encase::{ShaderType, StorageBuffer, UniformBuffer};
use glam::{Mat4, Quat, Vec3};
use gltf::{buffer, camera::Projection, image, scene, Node};
use std::{error::Error, rc::Rc};

mod bvh;
mod camera;

pub use camera::{Camera, CameraBuilder};

pub struct Scene {
    pub primitives: Primitives,
    pub materials: Materials,
    pub textures: Textures,
    pub uniform: Uniform,
    triangle_infos: Vec<TriangleInfo>,

    context: Rc<RenderContext>,
    uniform_buffer: wgpu::Buffer,
    pub bind_group_layout: BindGroupLayoutSet,
    pub bind_group: BindGroupSet,
}

#[derive(Default, ShaderType)]
pub struct Uniform {
    pub camera: Camera,
    hdri: u32,
}

pub struct Transform {
    pub transform: Mat4,
    pub inv_trans: Mat4,
}

pub struct TriangleInfo {
    pub triangle: Triangle,
    pub aabb: Aabb3,
    pub centroid: Vec3,
}

impl Transform {
    pub fn new(transform: Mat4) -> Self {
        Self {
            transform,
            inv_trans: transform.inverse().transpose(),
        }
    }
}

impl Scene {
    pub fn new(
        context: Rc<RenderContext>,
        config: &Config,
        scene: &gltf::Scene,
        buffers: &[buffer::Data],
        images: &[image::Data],
    ) -> Result<Self> {
        let mut primitives = Primitives::new();
        let mut materials = Materials::new();
        let mut textures = Textures::new(context.clone());
        let mut uniform = Uniform::default();
        let mut triangle_infos = Vec::new();

        Self::load_images(&mut textures, images);
        for node in scene.nodes() {
            Self::load_nodes(
                &mut materials,
                &mut triangle_infos,
                &mut primitives,
                &mut uniform,
                node,
                config,
                buffers,
                &Mat4::IDENTITY,
            );
        }

        let hdri = textures.load_texture_hdr(&config.hdri)?;
        uniform.hdri = hdri;

        let (bind_group_layout, bind_group, uniform_buffer) =
            Self::build(&uniform, &primitives, &mut triangle_infos, &context.clone())?;

        let (scene_bind_group_layout, scene_bind_group) = (bind_group_layout, bind_group);
        let (primitive_bind_group_layout, primitive_bind_group) = primitives.build(&context)?;
        let (material_bind_group_layout, material_bind_group) = materials.build(&context)?;
        let (texture_bind_group_layout, texture_bind_group) = textures.build();

        Ok(Self {
            primitives,
            materials,
            textures,
            uniform,
            triangle_infos,

            context: context.clone(),
            uniform_buffer,
            bind_group: BindGroupSet {
                scene: scene_bind_group,
                primitive: primitive_bind_group,
                material: material_bind_group,
                texture: texture_bind_group,
            },
            bind_group_layout: BindGroupLayoutSet {
                scene: scene_bind_group_layout,
                primitive: primitive_bind_group_layout,
                material: material_bind_group_layout,
                texture: texture_bind_group_layout,
            },
        })
    }

    fn load_images(textures: &mut Textures, images: &[image::Data]) {
        for image in images {
            textures.add_texture(image);
        }
    }

    fn load_nodes(
        materials: &mut Materials,
        triangle_infos: &mut Vec<TriangleInfo>,
        primitives: &mut Primitives,
        uniform: &mut Uniform,
        node: Node,
        config: &Config,
        buffers: &[buffer::Data],
        parent_transform: &Mat4,
    ) {
        let transform_matrix = *parent_transform * transform_to_matrix(&node.transform());
        let transform = Transform::new(transform_matrix);

        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                let material_idx = materials.add(&primitive.material()).unwrap();
                triangle_infos.append(
                    &mut primitives
                        .add(buffers, &primitive, &transform, material_idx)
                        .unwrap()
                        .into_iter()
                        .map(|triangle| {
                            let aabb = triangle.aabb(primitives);
                            TriangleInfo {
                                triangle,
                                aabb,
                                centroid: aabb.centroid(),
                            }
                        })
                        .collect(),
                );
            }
        }

        if let Some(camera) = node.camera() {
            match camera.projection() {
                Projection::Perspective(perspective) => {
                    let mut camera_builder = CameraBuilder::new();
                    camera_builder
                        .transform(transform_matrix)
                        .yfov(perspective.yfov());
                    uniform.camera = camera_builder.build(config.size.width, config.size.height);
                }
                _ => todo!(),
            }
        }

        for child in node.children() {
            Self::load_nodes(
                materials,
                triangle_infos,
                primitives,
                uniform,
                child,
                config,
                buffers,
                &transform_matrix,
            );
        }
    }

    pub fn build(
        uniform: &Uniform,
        primitives: &Primitives,
        triangle_infos: &mut Vec<TriangleInfo>,
        context: &RenderContext,
    ) -> encase::internal::Result<(wgpu::BindGroupLayout, wgpu::BindGroup, wgpu::Buffer)> {
        let device = context.device();
        let queue = context.queue();

        let mut wgsl_bytes = UniformBuffer::new(Vec::new());
        wgsl_bytes.write(uniform)?;
        let wgsl_bytes = wgsl_bytes.into_inner();

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: wgsl_bytes.len() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uniform_buffer, 0, &wgsl_bytes);

        let bvh = Bvh::new(primitives, triangle_infos);
        let triangles: Vec<_> = triangle_infos
            .iter()
            .map(|triangle_info| triangle_info.triangle)
            .collect();

        let mut wgsl_bytes = StorageBuffer::new(Vec::new());
        wgsl_bytes.write(&triangles)?;
        let wgsl_bytes = wgsl_bytes.into_inner();

        let triangle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: wgsl_bytes.len() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        queue.write_buffer(&triangle_buffer, 0, &wgsl_bytes);

        let mut wgsl_bytes = StorageBuffer::new(Vec::new());
        wgsl_bytes.write(&bvh.flatten())?;
        let wgsl_bytes = wgsl_bytes.into_inner();

        let bvh_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: wgsl_bytes.len() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        queue.write_buffer(&bvh_buffer, 0, &wgsl_bytes);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: triangle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: bvh_buffer.as_entire_binding(),
                },
            ],
        });

        Ok((bind_group_layout, bind_group, uniform_buffer))
    }

    pub fn set_camera(&mut self, new_cam: Camera) -> Result<(), Box<dyn Error>> {
        self.uniform.camera = new_cam;
        let mut wgsl_bytes = UniformBuffer::new(Vec::new());
        wgsl_bytes.write(&self.uniform)?;
        let wgsl_bytes = wgsl_bytes.into_inner();

        let queue = self.context.queue();
        queue.write_buffer(&self.uniform_buffer, 0, &wgsl_bytes);

        Ok(())
    }
}

fn transform_to_matrix(transform: &scene::Transform) -> Mat4 {
    match transform {
        scene::Transform::Matrix { matrix } => Mat4::from_cols_array_2d(matrix),
        scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => Mat4::from_scale_rotation_translation(
            Vec3::from_array(*scale),
            Quat::from_array(*rotation),
            Vec3::from_array(*translation),
        ),
    }
}
