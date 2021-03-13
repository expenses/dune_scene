use crate::RenderResources;
use primitives::{AnimatedVertex, Sun, Vec3A, Vertex};
use std::collections::HashMap;
use ultraviolet::{Mat4, Vec2, Vec3};
use wgpu::util::DeviceExt;

pub struct Orbit {
    pub longitude: f32,
    pub latitude: f32,
    distance: f32,
}

impl Orbit {
    fn from_vector(vector: Vec3) -> Self {
        let latitude = vector.x.atan2(vector.z);
        let horizontal_length = (vector.x * vector.x + vector.z * vector.z).sqrt();
        let longitude = horizontal_length.atan2(vector.y);
        let distance = vector.mag();
        Self {
            latitude,
            longitude,
            distance,
        }
    }

    pub fn rotate(&mut self, delta: Vec2) {
        use std::f32::consts::PI;
        let speed = 0.15;
        self.latitude -= delta.x.to_radians() * speed;
        self.longitude = (self.longitude - delta.y.to_radians() * speed)
            .max(std::f32::EPSILON)
            .min(PI / 2.0);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance + delta * 0.5).max(1.0).min(10.0);
    }

    fn as_vector(&self) -> Vec3 {
        let y = self.longitude.cos();
        let horizontal_amount = self.longitude.sin();
        let x = horizontal_amount * self.latitude.sin();
        let z = horizontal_amount * self.latitude.cos();
        Vec3::new(x, y, z) * self.distance
    }
}

pub struct Scene {
    pub camera_z_near: f32,
    pub camera_z_far: f32,
    camera_y_fov: f32,
    pub orbit: Orbit,
    pub texture_bind_group: wgpu::BindGroup,
    pub sun_buffer: wgpu::Buffer,
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub num_indices: u32,
    pub sun_facing: Vec3,
    look_at: Vec3,
}

impl Scene {
    pub fn load(
        bytes: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &RenderResources,
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
        let look_at = {
            // Multiplier to bring y to zero
            let multiplier = camera_eye.y / camera_view_direction.y;
            camera_eye - camera_view_direction * multiplier
        };
        let orbit = Orbit::from_vector(camera_eye - look_at);

        let mut image_map = HashMap::new();

        for image in gltf.images() {
            image_map.insert(
                image.name().unwrap(),
                load_image(&image, buffer_blob, device, queue)?,
            );
        }

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene texture bind group"),
            layout: &resources.double_texture_bgl,
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

        let sun_facing = sun_rotor * Vec3::unit_z();

        let sun = Sun {
            // Lighting uses the -Z axis.
            // https://github.com/KhronosGroup/glTF/blob/master/extensions/2.0/Khronos/KHR_lights_punctual/README.md#directional
            facing: Vec3A::new(sun_facing),
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

                let num_vertices = vertices.len() as u16;

                let read_indices = match reader.read_indices().unwrap() {
                    gltf::mesh::util::ReadIndices::U16(indices) => indices,
                    gltf::mesh::util::ReadIndices::U32(_) => {
                        return Err(anyhow::anyhow!("U32 indices not supported"))
                    }
                    _ => unreachable!(),
                };

                indices.extend(read_indices.map(|index| index + num_vertices));

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
            camera_z_far: camera_perspective.zfar().unwrap() * 1.5,
            texture_bind_group,
            sun_buffer,
            vertices,
            indices,
            num_indices,
            sun_facing,
            orbit,
            look_at,
        })
    }

    pub fn create_camera(&self, width: u32, height: u32) -> primitives::Camera {
        let camera_eye = self.orbit.as_vector() + self.look_at;

        let camera_view = Mat4::look_at(camera_eye, self.look_at, Vec3::unit_y());

        let perspective = ultraviolet::projection::perspective_wgpu_dx(
            self.camera_y_fov,
            width as f32 / height as f32,
            self.camera_z_near,
            self.camera_z_far,
        );

        let perspective_view = perspective * camera_view;

        primitives::Camera {
            perspective_view,
            view: camera_view,
            perspective,
            position: camera_eye,
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

    // It turns out that we can just reverse the array to iter through nodes depth first! Useful for applying animations.
    fn iter_depth_first(&self) -> impl Iterator<Item = (usize, Option<usize>)> + '_ {
        self.inner
            .iter()
            .enumerate()
            .rev()
            .map(|(index, &(_, parent))| {
                (
                    index,
                    if parent != usize::max_value() {
                        Some(parent)
                    } else {
                        None
                    },
                )
            })
    }
}

pub struct Ship {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub num_indices: u32,
    pub texture_bind_group: wgpu::BindGroup,
}

impl Ship {
    pub fn load(
        bytes: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &RenderResources,
    ) -> anyhow::Result<Self> {
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

                let num_vertices = vertices.len() as u16;

                let read_indices = match reader.read_indices().unwrap() {
                    gltf::mesh::util::ReadIndices::U16(indices) => indices,
                    gltf::mesh::util::ReadIndices::U32(_) => {
                        return Err(anyhow::anyhow!("U32 indices not supported"))
                    }
                    _ => unreachable!(),
                };

                indices.extend(read_indices.map(|index| index + num_vertices));

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

        let image = gltf.images().next().unwrap();
        let image = load_image(&image, buffer_blob, device, queue)?;

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ship texture bind group"),
            layout: &resources.single_texture_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&image),
            }],
        });

        Ok(Self {
            vertices,
            indices,
            num_indices,
            texture_bind_group,
        })
    }
}

pub struct LandCraft {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub num_indices: u32,
    pub texture_bind_group: wgpu::BindGroup,
}

impl LandCraft {
    pub fn load(
        bytes: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &RenderResources,
    ) -> anyhow::Result<Self> {
        let gltf = gltf::Gltf::from_slice(bytes)?;

        let buffer_blob = gltf.blob.as_ref().unwrap();

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        const SIZE: f32 = 0.02;

        for mesh in gltf.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| {
                    assert_eq!(buffer.index(), 0);
                    Some(buffer_blob)
                });

                let num_vertices = vertices.len() as u16;

                let read_indices = match reader.read_indices().unwrap() {
                    gltf::mesh::util::ReadIndices::U16(indices) => indices,
                    gltf::mesh::util::ReadIndices::U32(_) => {
                        return Err(anyhow::anyhow!("U32 indices not supported"))
                    }
                    _ => unreachable!(),
                };

                indices.extend(read_indices.map(|index| index + num_vertices));

                let positions = reader.read_positions().unwrap();
                let uvs = reader.read_tex_coords(0).unwrap().into_f32();
                let normals = reader.read_normals().unwrap();
                let tangents = reader.read_tangents().unwrap();

                positions.zip(uvs).zip(normals).zip(tangents).for_each(
                    |(((position, uv), normal), tangent)| {
                        let position: Vec3 = position.into();

                        vertices.push(Vertex {
                            position: position * SIZE,
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

        let image = gltf.images().next().unwrap();
        let image = load_image(&image, buffer_blob, device, queue)?;

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ship texture bind group"),
            layout: &resources.single_texture_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&image),
            }],
        });

        Ok(Self {
            vertices,
            indices,
            num_indices,
            texture_bind_group,
        })
    }
}

pub struct Explosion {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub num_indices: u32,
    pub texture_bind_group: wgpu::BindGroup,

    pub animation: animation::Animation,
    pub animation_joints: animation::AnimationJoints,
    pub joint_indices_to_node_indices: Vec<usize>,
    pub inverse_bind_matrices: Vec<Mat4>,
    pub depth_first_nodes: Vec<(usize, Option<usize>)>,
}

impl Explosion {
    pub fn load(
        bytes: &[u8],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &RenderResources,
    ) -> anyhow::Result<Self> {
        let gltf = gltf::Gltf::from_slice(bytes)?;

        let buffer_blob = gltf.blob.as_ref().unwrap();

        let node_tree = NodeTree::new(&gltf);

        assert!(gltf.skins().count() <= 1);
        let skin = gltf.skins().next();
        if let Some(skin) = skin.as_ref() {
            assert!(skin.skeleton().is_none());
        }

        println!("{:?}", skin.is_some());

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for (node, mesh) in gltf
            .nodes()
            .filter_map(|node| node.mesh().map(|mesh| (node, mesh)))
        {
            let transform = if node.skin().is_some() {
                // We can't apply transformations on animated models, but we also don't need to..
                Mat4::identity()
            } else {
                // We allow combining animated and non animated meshes
                node_tree.transform_of(node.index())
            };

            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| {
                    assert_eq!(buffer.index(), 0);
                    Some(buffer_blob)
                });

                let num_vertices = vertices.len() as u16;

                let read_indices = match reader.read_indices().unwrap() {
                    gltf::mesh::util::ReadIndices::U16(indices) => indices,
                    gltf::mesh::util::ReadIndices::U32(_) => {
                        return Err(anyhow::anyhow!("U32 indices not supported"))
                    }
                    _ => unreachable!(),
                };

                indices.extend(read_indices.map(|index| index + num_vertices));

                let positions = reader.read_positions().unwrap();
                let uvs = reader.read_tex_coords(0).unwrap().into_f32();
                let normals = reader.read_normals().unwrap();
                let tangents = reader.read_tangents().unwrap();

                let mut joint_indices = reader.read_joints(0).map(|iter| iter.into_u16());
                let mut joint_weights = reader.read_weights(0).map(|iter| iter.into_f32());

                positions.zip(uvs).zip(normals).zip(tangents).for_each(
                    |(((position, uv), normal), tangent)| {
                        let joint_indices = match joint_indices {
                            Some(ref mut iter) => iter.next().unwrap(),
                            None => [node.index() as u16; 4],
                        };
                        let joint_weights = match joint_weights {
                            Some(ref mut iter) => iter.next().unwrap(),
                            None => [1.0, 0.0, 0.0, 0.0],
                        };

                        let position: Vec3 = position.into();

                        vertices.push(AnimatedVertex {
                            position: transform.transform_vec3(position),
                            uv: uv.into(),
                            normal: normal.into(),
                            tangent: tangent.into(),
                            joint_indices,
                            joint_weights: joint_weights.into(),
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

        let image = gltf.images().next().unwrap();
        let image = load_image(&image, buffer_blob, device, queue)?;

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ship texture bind group"),
            layout: &resources.single_texture_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&image),
            }],
        });

        let mut animations =
            animation::read_animations(gltf.animations(), &buffer_blob, "explosion");

        let animation = animations.remove(0);

        let depth_first_nodes: Vec<_> = node_tree.iter_depth_first().collect();

        let animation_joints = animation::AnimationJoints::new(gltf.nodes(), &depth_first_nodes);

        let joint_indices_to_node_indices = if let Some(skin) = skin.as_ref() {
            skin.joints().map(|node| node.index()).collect()
        } else {
            gltf.nodes().map(|node| node.index()).collect()
        };

        let inverse_bind_matrices = if let Some(skin) = skin.as_ref() {
            skin.reader(|buffer| {
                assert_eq!(buffer.index(), 0);
                Some(buffer_blob)
            })
            .read_inverse_bind_matrices()
            .ok_or_else(|| anyhow::anyhow!("Missing inverse bind matrices"))?
            .map(|mat| mat.into())
            .collect()
        } else {
            gltf.nodes()
                .map(|node| node_tree.transform_of(node.index()).inversed())
                .collect()
        };

        Ok(Self {
            vertices,
            indices,
            num_indices,
            texture_bind_group,
            animation,
            depth_first_nodes,
            animation_joints,
            joint_indices_to_node_indices,
            inverse_bind_matrices,
        })
    }
}
