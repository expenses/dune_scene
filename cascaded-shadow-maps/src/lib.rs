use ultraviolet::{Mat4, Vec3, Vec4};

pub struct CascadedShadowMaps {
    textures: [wgpu::TextureView; 3],
    light_projection_buffers: [wgpu::Buffer; 3],
    light_projection_bind_groups: [wgpu::BindGroup; 3],
    projection_bgl: wgpu::BindGroupLayout,
    rendering_bgl: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    rendering_bind_group: wgpu::BindGroup,
}

impl CascadedShadowMaps {
    pub fn new(device: &wgpu::Device, size: u32) -> Self {
        let array_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cascaded shadow map - shadow texture array"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth: 3,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        });

        let texture_view = |label, i| {
            array_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("cascaded shadow map - {} texture", label)),
                base_array_layer: i,
                array_layer_count: Some(std::num::NonZeroU32::new(1).unwrap()),
                ..Default::default()
            })
        };

        let projection_buffer = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!(
                    "cascaded shadow map - {} projection buffer",
                    label
                )),
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
                size: std::mem::size_of::<Mat4>() as u64,
                mapped_at_creation: false,
            })
        };

        let projection_buffers = [
            projection_buffer("near"),
            projection_buffer("middle"),
            projection_buffer("far"),
        ];

        let projection_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cascaded shadow map - projection bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let rendering_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cascaded shadow map - rendering bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT | wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cascaded shadow map - uniform buffer"),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            size: std::mem::size_of::<Uniform>() as u64,
            mapped_at_creation: false,
        });

        let rendering_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cascaded shadow map - rendering bind group"),
            layout: &rendering_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&array_texture.create_view(
                        &wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::D2Array),
                            ..Default::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let projection_bind_group = |label, i: usize| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!(
                    "cascaded shadow map - {} projection bind group",
                    label
                )),
                layout: &projection_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: (&projection_buffers[i]).as_entire_binding(),
                }],
            })
        };

        Self {
            textures: [
                texture_view("near", 0),
                texture_view("middle", 1),
                texture_view("far", 2),
            ],
            light_projection_bind_groups: [
                projection_bind_group("near", 0),
                projection_bind_group("middle", 1),
                projection_bind_group("far", 2),
            ],
            light_projection_buffers: projection_buffers,
            projection_bgl,
            uniform_buffer,
            rendering_bgl,
            rendering_bind_group,
        }
    }

    pub fn textures(&self) -> &[wgpu::TextureView; 3] {
        &self.textures
    }

    pub fn light_projection_bind_groups(&self) -> &[wgpu::BindGroup; 3] {
        &self.light_projection_bind_groups
    }

    pub fn light_projection_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.projection_bgl
    }

    pub fn rendering_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.rendering_bgl
    }

    pub fn rendering_bind_group(&self) -> &wgpu::BindGroup {
        &self.rendering_bind_group
    }

    pub fn update_params(
        &self,
        camera: CameraParams,
        origin_to_light: Vec3,
        cascade_split_lambda: f32,
        queue: &wgpu::Queue,
    ) {
        let uniform = update_cascades(camera, cascade_split_lambda, origin_to_light);

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        for i in 0..3 {
            queue.write_buffer(
                &self.light_projection_buffers[i],
                0,
                bytemuck::bytes_of(&uniform.matrices[i]),
            );
        }
    }
}

pub struct CameraParams {
    pub projection_view: Mat4,
    pub near_clip: f32,
    pub far_clip: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    matrices: [Mat4; 3],
    split_depths: [f32; 3],
}

// https://github.com/SaschaWillems/Vulkan/blob/5db9781d529467c4474bbc957ab5f1ee06126cf4/examples/shadowmappingcascade/shadowmappingcascade.cpp#L634-L638
fn update_cascades(
    camera: CameraParams,
    cascade_split_lambda: f32,
    origin_to_light: Vec3,
) -> Uniform {
    let clip_range = camera.far_clip - camera.near_clip;

    let min_z = camera.near_clip;
    let max_z = camera.far_clip;

    let range = max_z - min_z;
    let ratio = max_z / min_z;

    // Calculate split depths based on view camera frustum
    // Based on method presented in https://developer.nvidia.com/gpugems/GPUGems3/gpugems3_ch10.html
    let cascade_split = |i| {
        let p = (i + 1) as f32 / 3.0;
        let log = min_z * ratio.powf(p);
        let uniform = min_z + range * p;
        let d = cascade_split_lambda * (log - uniform) + uniform;
        (d - camera.near_clip) / clip_range
    };
    let cascade_splits = [cascade_split(0), cascade_split(1), cascade_split(2)];

    let inverse_camera_projection_view = camera.projection_view.inversed();

    let calculate_matrix = |last_split_dist, split_dist| {
        let mut frustum_corners = [
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(-1.0, 1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(-1.0, -1.0, 1.0),
        ];

        for i in 0..8 {
            let inv_corner = inverse_camera_projection_view
                * Vec4::new(
                    frustum_corners[i].x,
                    frustum_corners[i].y,
                    frustum_corners[i].z,
                    1.0,
                );

            frustum_corners[i] = inv_corner.truncated() / inv_corner.w;
        }

        for i in 0..4 {
            let dist = frustum_corners[i + 4] - frustum_corners[i];
            frustum_corners[i + 4] = frustum_corners[i] + (dist * split_dist);
            frustum_corners[i] = frustum_corners[i] + (dist * last_split_dist);
        }

        let mut frustum_center = Vec3::zero();
        for i in 0..8 {
            frustum_center += frustum_corners[i];
        }
        frustum_center /= 8.0;

        let mut radius = 0.0;
        for i in 0..8 {
            let distance = (frustum_corners[i] - frustum_center).mag();
            radius = f32::max(radius, distance);
        }
        radius = (radius * 16.0).ceil() / 16.0;

        let max_extents = Vec3::broadcast(radius);
        let min_extents = -max_extents;

        let light_dir = -origin_to_light;

        let light_view_matrix = Mat4::look_at(
            frustum_center - light_dir * -min_extents.z,
            frustum_center,
            Vec3::unit_y(),
        );
        let light_ortho_matrix = ultraviolet::projection::orthographic_wgpu_dx(
            min_extents.x,
            max_extents.x,
            min_extents.y,
            max_extents.y,
            0.0,
            max_extents.z - min_extents.z,
        );

        let matrix = light_ortho_matrix * light_view_matrix;
        let split_depth = (camera.near_clip + split_dist * clip_range) * -1.0;

        (matrix, split_depth)
    };

    let (matrix_1, split_depth_1) = calculate_matrix(0.0, cascade_splits[0]);
    let (matrix_2, split_depth_2) = calculate_matrix(cascade_splits[0], cascade_splits[1]);
    let (matrix_3, split_depth_3) = calculate_matrix(cascade_splits[1], cascade_splits[2]);

    Uniform {
        matrices: [matrix_1, matrix_2, matrix_3],
        split_depths: [split_depth_1, split_depth_2, split_depth_3],
    }
}
