use crate::resources_and_pipelines::{Pipelines, RenderResources};
use crate::{FRAMEBUFFER_FORMAT, INDEX_FORMAT};
use rand::Rng;
use ultraviolet::Vec3;
use wgpu::util::DeviceExt;

pub fn create_height_map(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipelines: &Pipelines,
    vertices: &wgpu::Buffer,
    indices: &wgpu::Buffer,
    num_indices: u32,
) -> wgpu::TextureView {
    let height_map_texture = create_texture(
        &device,
        "height map texture",
        1024,
        1024,
        wgpu::TextureFormat::R32Float,
        wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("height map encoder"),
    });

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("height map render pass"),
        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
            attachment: &height_map_texture,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: true,
            },
        }],
        depth_stencil_attachment: None,
    });

    render_pass.set_pipeline(&pipelines.bake_height_map_pipeline);
    render_pass.set_vertex_buffer(0, vertices.slice(..));
    render_pass.set_index_buffer(indices.slice(..), INDEX_FORMAT);
    render_pass.draw_indexed(0..num_indices, 0, 0..1);

    drop(render_pass);

    queue.submit(Some(encoder.finish()));

    height_map_texture
}

pub fn create_texture(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsage,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

pub fn framebuffer_and_tonemapper_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    resources: &RenderResources,
    tonemapper_uniform_buffer: &wgpu::Buffer,
) -> (wgpu::TextureView, wgpu::BindGroup) {
    let framebuffer_texture = create_texture(
        device,
        "framebuffer texture",
        width,
        height,
        FRAMEBUFFER_FORMAT,
        wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
    );

    let tonemapper_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tonemapper bind group"),
        layout: &resources.tonemap_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&framebuffer_texture),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&resources.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: tonemapper_uniform_buffer.as_entire_binding(),
            },
        ],
    });

    (framebuffer_texture, tonemapper_bind_group)
}

fn create_particle_bind_group(
    device: &wgpu::Device,
    name: &str,
    num: u64,
    colour: Vec3,
    half_size_linear: f32,
    resources: &RenderResources,
) -> wgpu::BindGroup {
    let particles_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(&format!("{} buffer", name)),
        usage: wgpu::BufferUsage::STORAGE,
        size: std::mem::size_of::<primitives::Particle>() as u64 * num,
        mapped_at_creation: false,
    });

    let particles_buffer_info = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{} info buffer", name)),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::bytes_of(&primitives::ParticlesBufferInfo {
            colour,
            half_size_linear,
            ..Default::default()
        }),
    });

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{} bind group", name)),
        layout: &resources.particles_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: particles_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: particles_buffer_info.as_entire_binding(),
            },
        ],
    })
}

pub fn create_ships(
    num_ships: u32,
    device: &wgpu::Device,
    rng: &mut rand::rngs::ThreadRng,
    resources: &RenderResources,
) -> (wgpu::BindGroup, u32, wgpu::BindGroup) {
    let ship_positions: Vec<_> = (0..num_ships)
        .map(|_| primitives::Ship {
            position: Vec3::new(
                rng.gen_range(-2.0..=2.0),
                rng.gen_range(0.49..=0.51),
                rng.gen_range(-2.0..=2.0),
            ),
            y_rotation: rng.gen_range(0.0..360.0_f32.to_radians()),
            rotation_speed: rng.gen_range(-0.02..=0.02),
            ..Default::default()
        })
        .collect();

    let ship_positions_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ship positions buffer"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&ship_positions),
    });

    let ship_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ship bind group"),
        layout: &resources.ship_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: ship_positions_buffer.as_entire_binding(),
        }],
    });

    let particles_per_ship = 15 * 2;
    let num_exhaust_particles = num_ships * particles_per_ship;
    let exhaust_particles_bind_group = create_particle_bind_group(
        &device,
        "exhaust particles",
        num_exhaust_particles as u64,
        Vec3::new(0.5, 0.75, 1.0),
        0.02,
        &resources,
    );

    (
        ship_bind_group,
        particles_per_ship,
        exhaust_particles_bind_group,
    )
}

pub fn create_land_craft(
    num_land_craft: u32,
    device: &wgpu::Device,
    rng: &mut rand::rngs::ThreadRng,
    resources: &RenderResources,
    height_map_texture: &wgpu::TextureView,
    settings: &primitives::Settings,
) -> (wgpu::BindGroup, u32, wgpu::BindGroup, u32, wgpu::BindGroup) {
    let land_craft: Vec<_> = (0..num_land_craft)
        .map(|_| primitives::LandCraft {
            position: Vec3::new(rng.gen_range(-2.0..=2.0), 0.0, rng.gen_range(-2.0..=2.0)),
            facing: rng.gen_range(0.0..360.0_f32.to_radians()),
            ..Default::default()
        })
        .collect();

    let land_craft_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("land craft buffer"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&land_craft),
    });

    let land_craft_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("land craft bind group"),
        layout: &resources.land_craft_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: land_craft_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(height_map_texture),
            },
        ],
    });

    let smoke_particles_per_landcraft = 45;
    let num_smoke_particles = num_land_craft * smoke_particles_per_landcraft;
    let smoke_particles_bind_group = create_particle_bind_group(
        &device,
        "smoke particles",
        num_smoke_particles as u64,
        Vec3::broadcast(0.15),
        0.03,
        &resources,
    );

    let sand_particles_per_landcraft = 10;
    let num_sand_particles = num_land_craft * sand_particles_per_landcraft;
    let sand_particles_bind_group = create_particle_bind_group(
        &device,
        "sand particles",
        num_sand_particles as u64,
        settings.base_colour * 0.4,
        0.1,
        &resources,
    );

    (
        land_craft_bind_group,
        num_smoke_particles,
        smoke_particles_bind_group,
        num_sand_particles,
        sand_particles_bind_group,
    )
}
