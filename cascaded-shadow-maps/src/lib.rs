use ultraviolet::{Mat4, Vec3, Vec4};

pub struct CascadedShadowMaps {
    textures: [wgpu::TextureView; 3],
    light_projection_buffers: [wgpu::Buffer; 3],
    light_projection_bind_groups: [wgpu::BindGroup; 3],
    projection_bgl: wgpu::BindGroupLayout,
}

impl CascadedShadowMaps {
    pub fn new(device: &wgpu::Device, size: u32) -> Self {
        let texture = |label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("cascaded shadow map - {} texture", label)),
                    size: wgpu::Extent3d {
                        width: size,
                        height: size,
                        depth: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth32Float,
                    usage: wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
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
            textures: [texture("near"), texture("middle"), texture("far")],
            light_projection_bind_groups: [
                projection_bind_group("near", 0),
                projection_bind_group("middle", 1),
                projection_bind_group("far", 2),
            ],
            light_projection_buffers: projection_buffers,
            projection_bgl,
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

    pub fn update_params(
        &self,
        camera: CameraParams,
        origin_to_light: Vec3,
        cascade_split_lambda: f32,
        queue: &wgpu::Queue,
    ) {
        let matrices = update_cascades(camera, cascade_split_lambda, origin_to_light);

        for i in 0..3 {
            queue.write_buffer(
                &self.light_projection_buffers[i],
                0,
                bytemuck::bytes_of(&matrices[i]),
            );
        }
    }
}

pub struct CameraParams {
    pub projection_view: Mat4,
    pub near_clip: f32,
    pub far_clip: f32,
}

// https://github.com/SaschaWillems/Vulkan/blob/5db9781d529467c4474bbc957ab5f1ee06126cf4/examples/shadowmappingcascade/shadowmappingcascade.cpp#L634-L638
fn update_cascades(
    camera: CameraParams,
    cascade_split_lambda: f32,
    origin_to_light: Vec3,
) -> [Mat4; 3] {
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

    println!("{:?}", cascade_splits);

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

        light_ortho_matrix * light_view_matrix
    };

    [
        calculate_matrix(0.0, cascade_splits[0]),
        calculate_matrix(cascade_splits[0], cascade_splits[1]),
        calculate_matrix(cascade_splits[1], cascade_splits[2]),
    ]
}
