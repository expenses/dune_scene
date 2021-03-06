use primitives::{Sun, Vec3A, Vertex};
use std::collections::HashMap;
use ultraviolet::{Mat4, Vec3};
use wgpu::util::DeviceExt;

pub struct Scene {
    camera_y_fov: f32,
    camera_z_near: f32,
    camera_view: Mat4,
    camera_eye: Vec3,
    pub texture_bind_group: wgpu::BindGroup,
    pub sun_buffer: wgpu::Buffer,
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub num_indices: u32,
}

impl Scene {
    pub fn load(
        bytes: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_bgl: &wgpu::BindGroupLayout,
    ) -> anyhow::Result<Self> {
        let gltf = gltf::Gltf::from_slice(bytes)?;

        let buffer_blob = gltf.blob.as_ref().unwrap();

        let node_tree = NodeTree::new(&gltf);

        let (camera_node_index, camera) = gltf
            .nodes()
            .find_map(|node| node.camera().map(|camera| (node.index(), camera)))
            .unwrap();

        let camera_perspective = match camera.projection() {
            gltf::camera::Projection::Perspective(perspective) => perspective,
            _ => panic!(),
        };

        let camera_transform = node_tree.transform_of(camera_node_index);

        let camera_eye = camera_transform.extract_translation();
        let camera_rotor = camera_transform.extract_rotation();
        let camera_view_direction = camera_rotor * -Vec3::unit_z();
        let camera_up = camera_rotor * Vec3::unit_y();
        let camera_view = Mat4::look_at(camera_eye, camera_eye + camera_view_direction, camera_up);

        let mut image_map = HashMap::new();

        for image in gltf.images() {
            image_map.insert(
                image.name().unwrap(),
                load_image(&image, buffer_blob, device, queue)?,
            );
        }

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture bind group"),
            layout: texture_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &image_map.get("normals").unwrap(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &image_map.get("details").unwrap(),
                    ),
                },
            ],
        });

        let (sun_node_index, sun) = gltf
            .nodes()
            .find_map(|node| node.light().map(|light| (node.index(), light)))
            .unwrap();
        let sun_rotor = node_tree.transform_of(sun_node_index).extract_rotation();

        let sun = Sun {
            // Lighting uses the -Z axis.
            // https://github.com/KhronosGroup/glTF/blob/master/extensions/2.0/Khronos/KHR_lights_punctual/README.md#directional
            facing: Vec3A::new(sun_rotor * Vec3::unit_z()),
            output: Vec3::from(sun.color()) * sun.intensity(),
        };

        let sun_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sun buffer"),
            usage: wgpu::BufferUsage::UNIFORM,
            contents: bytemuck::bytes_of(&sun),
        });

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for mesh in gltf.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| {
                    assert_eq!(buffer.index(), 0);
                    Some(buffer_blob)
                });

                let num_vertices = vertices.len() as u32;

                indices.extend(
                    reader
                        .read_indices()
                        .unwrap()
                        .into_u32()
                        .map(|index| index + num_vertices),
                );

                let positions = reader.read_positions().unwrap();
                let uvs = reader.read_tex_coords(0).unwrap().into_f32();
                let normals = reader.read_normals().unwrap();
                let tangents = reader.read_tangents().unwrap();

                positions.zip(uvs).zip(normals).zip(tangents).for_each(
                    |(((position, uv), normal), tangent)| {
                        vertices.push(Vertex {
                            position: position.into(),
                            uv: uv.into(),
                            normal: normal.into(),
                            tangent: tangent.into(),
                        });
                    },
                )
            }
        }

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            usage: wgpu::BufferUsage::VERTEX,
            contents: bytemuck::cast_slice(&vertices),
        });

        let num_indices = indices.len() as u32;

        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indices"),
            usage: wgpu::BufferUsage::INDEX,
            contents: bytemuck::cast_slice(&indices),
        });

        Ok(Self {
            camera_y_fov: camera_perspective.yfov(),
            camera_z_near: camera_perspective.znear(),
            camera_view,
            texture_bind_group,
            sun_buffer,
            vertices,
            indices,
            num_indices,
            camera_eye,
        })
    }

    pub fn create_camera(&self, width: u32, height: u32) -> primitives::Camera {
        let perspective = ultraviolet::projection::perspective_infinite_z_wgpu_dx(
            self.camera_y_fov,
            width as f32 / height as f32,
            self.camera_z_near,
        );

        let perspective_view = perspective * self.camera_view;

        primitives::Camera {
            perspective_view,
            position: self.camera_eye,
        }
    }
}

fn load_image(
    image: &gltf::Image,
    buffer_blob: &[u8],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<wgpu::TextureView> {
    let image_view = match image.source() {
        gltf::image::Source::View { view, .. } => view,
        _ => panic!(),
    };

    let image_start = image_view.offset();
    let image_end = image_start + image_view.length();
    let image_bytes = &buffer_blob[image_start..image_end];

    let name = image.name().unwrap();

    let image = image::load_from_memory_with_format(image_bytes, image::ImageFormat::Png)?;

    let image = match image {
        image::DynamicImage::ImageRgba8(image) => image,
        _ => panic!(),
    };

    Ok(device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some(name),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsage::COPY_DST | wgpu::TextureUsage::SAMPLED,
            },
            &*image,
        )
        .create_view(&wgpu::TextureViewDescriptor::default()))
}

struct NodeTree {
    inner: Vec<(Mat4, usize)>,
}

impl NodeTree {
    fn new(gltf: &gltf::Gltf) -> Self {
        let mut inner = vec![(Mat4::identity(), usize::max_value()); gltf.nodes().count()];

        for node in gltf.nodes() {
            inner[node.index()].0 = node.transform().matrix().into();
            for child in node.children() {
                inner[child.index()].1 = node.index();
            }
        }

        Self { inner }
    }

    fn transform_of(&self, mut index: usize) -> Mat4 {
        let mut transform_sum = Mat4::identity();

        while index != usize::max_value() {
            let (transform, parent_index) = self.inner[index];
            transform_sum = transform * transform_sum;
            index = parent_index;
        }

        transform_sum
    }
}

pub struct Ship {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub num_indices: u32,
}

impl Ship {
    pub fn load(bytes: &[u8], device: &wgpu::Device, queue: &wgpu::Queue) -> anyhow::Result<Self> {
        let gltf = gltf::Gltf::from_slice(bytes)?;

        let buffer_blob = gltf.blob.as_ref().unwrap();

        let node_tree = NodeTree::new(&gltf);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for (node, mesh) in gltf
            .nodes()
            .filter_map(|node| node.mesh().map(|mesh| (node, mesh)))
        {
            let transform = node_tree.transform_of(node.index());

            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| {
                    assert_eq!(buffer.index(), 0);
                    Some(buffer_blob)
                });

                let num_vertices = vertices.len() as u32;

                indices.extend(
                    reader
                        .read_indices()
                        .unwrap()
                        .into_u32()
                        .map(|index| index + num_vertices),
                );

                let positions = reader.read_positions().unwrap();
                let uvs = reader.read_tex_coords(0).unwrap().into_f32();
                let normals = reader.read_normals().unwrap();
                let tangents = reader.read_tangents().unwrap();

                positions.zip(uvs).zip(normals).zip(tangents).for_each(
                    |(((position, uv), normal), tangent)| {
                        let position: Vec3 = position.into();

                        vertices.push(Vertex {
                            position: transform.transform_vec3(position),
                            uv: uv.into(),
                            normal: normal.into(),
                            tangent: tangent.into(),
                        });
                    },
                )
            }
        }

        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            usage: wgpu::BufferUsage::VERTEX,
            contents: bytemuck::cast_slice(&vertices),
        });

        let num_indices = indices.len() as u32;

        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indices"),
            usage: wgpu::BufferUsage::INDEX,
            contents: bytemuck::cast_slice(&indices),
        });

        Ok(Self {
            vertices,
            indices,
            num_indices,
        })
    }
}
