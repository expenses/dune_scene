use crate::model_loading::AnimatedModel;
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

pub fn create_animated_models(
    num_animated_models: u32,
    device: &wgpu::Device,
    resources: &RenderResources,
    rng: &mut rand::rngs::ThreadRng,
    animated_model: &AnimatedModel,
) -> (wgpu::BindGroup, wgpu::Buffer) {
    let animated_model_states: Vec<_> = (0..num_animated_models)
        .map(|_| {
            let animation = rng.gen_range(0..animated_model.animations.len());

            primitives::AnimatedModelState {
                time: rng.gen_range(0.0..=animated_model.animations[animation].total_time),
                animation_duration: animated_model.animations[animation].total_time,
                animation_index: animation as u32,
            }
        })
        .collect();

    let animation_bind_group = create_animation_bind_group(
        device,
        resources,
        num_animated_models as usize,
        &animated_model
            .depth_first_nodes
            .iter()
            .map(|(node_index, parent_index)| primitives::NodeAndParent {
                node_index: *node_index as u32,
                parent_index: parent_index.map(|p| p as i32).unwrap_or(-1),
            })
            .collect::<Vec<_>>(),
        &animated_model.joint_indices_to_node_indices,
        &animated_model.inverse_bind_transforms,
        &animated_model.initial_local_transforms,
        &animated_model_states,
    );

    let position_instances: Vec<_> = (0..num_animated_models)
        .map(|_| {
            Vec3::new(
                rng.gen_range(-2.0..=2.0),
                rng.gen_range(0.0..=0.5),
                rng.gen_range(-2.0..=2.0),
            )
        })
        .collect();

    let position_instances_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("position instances"),
        usage: wgpu::BufferUsage::VERTEX,
        contents: bytemuck::cast_slice(&position_instances),
    });

    (animation_bind_group, position_instances_buffer)
}

fn create_animation_bind_group(
    device: &wgpu::Device,
    resources: &RenderResources,
    num_instances: usize,
    depth_first_nodes: &[primitives::NodeAndParent],
    joint_indices_to_node_indices: &[u32],
    inverse_bind_transforms: &[primitives::Similarity],
    local_transforms: &[primitives::Similarity],
    animated_model_states: &[primitives::AnimatedModelState],
) -> wgpu::BindGroup {
    let num_joints = joint_indices_to_node_indices.len();
    let num_nodes = depth_first_nodes.len();

    debug_assert_eq!(inverse_bind_transforms.len(), num_joints);
    debug_assert_eq!(local_transforms.len(), num_nodes);

    let joints = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("animation joints"),
        usage: wgpu::BufferUsage::STORAGE,
        size: (num_joints * num_instances * std::mem::size_of::<primitives::Similarity>()) as u64,
        mapped_at_creation: false,
    });

    let info = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animated model info"),
        usage: wgpu::BufferUsage::UNIFORM,
        contents: bytemuck::bytes_of(&primitives::AnimatedModelInfo {
            num_joints: num_joints as u32,
            num_nodes: num_nodes as u32,
            num_instances: num_instances as u32,
        }),
    });

    let mut full_local_transforms = Vec::new();
    for _ in 0..num_instances {
        full_local_transforms.extend_from_slice(local_transforms);
    }

    debug_assert_eq!(full_local_transforms.len(), num_nodes * num_instances);

    let local_transforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animation local transforms"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&full_local_transforms),
    });

    let animated_model_states = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animated model states"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&animated_model_states),
    });

    let global_transforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("animation global transforms"),
        usage: wgpu::BufferUsage::STORAGE,
        size: (std::mem::size_of::<primitives::Similarity>() * num_instances * num_nodes) as u64,
        mapped_at_creation: false,
    });

    let depth_first_nodes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animation depth first nodes"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(depth_first_nodes),
    });

    let joint_indices_to_node_indices =
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("animation joint indices to node indices"),
            usage: wgpu::BufferUsage::STORAGE,
            contents: bytemuck::cast_slice(joint_indices_to_node_indices),
        });

    let inverse_bind_transforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animation inverse bind transforms"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&inverse_bind_transforms),
    });

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("animation bind group"),
        layout: &resources.animation_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: joints.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: info.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: local_transforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: animated_model_states.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: global_transforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: depth_first_nodes.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: joint_indices_to_node_indices.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: inverse_bind_transforms.as_entire_binding(),
            },
        ],
    })
}

fn create_channel_bind_group<T: bytemuck::Pod + Clone>(
    device: &wgpu::Device,
    name: &str,
    resources: &RenderResources,
    iter: impl Iterator<Item = (Vec<f32>, Vec<T>, u32)>,
    num_channels_per_animation: impl Iterator<Item = u32> + Clone,
) -> (wgpu::BindGroup, u32) {
    let mut combined_inputs = Vec::new();
    let mut combined_outputs = Vec::new();
    let mut channels = Vec::new();

    for (inputs, outputs, node_index) in iter {
        let inputs_offset = combined_inputs.len() as u32;
        let outputs_offset = combined_outputs.len() as u32;
        let num_inputs = inputs.len() as u32;

        channels.push(primitives::Channel {
            inputs_offset,
            outputs_offset,
            num_inputs,
            node_index,
        });

        combined_inputs.extend_from_slice(&inputs);
        combined_outputs.extend_from_slice(&outputs);
    }

    let inputs = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("animation {} channel inputs", name)),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&combined_inputs),
    });

    let outputs = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("animation {} channel outputs", name)),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&combined_outputs),
    });

    let channels_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("animation {} channels", name)),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&channels),
    });

    let max_num_channels = num_channels_per_animation.clone().fold(0, |a, b| a.max(b));

    let max_num_channels_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("animation {} max num channels", name)),
        usage: wgpu::BufferUsage::UNIFORM,
        contents: bytemuck::bytes_of(&max_num_channels),
    });

    let mut animation_info = Vec::new();
    let mut channels_offset = 0;

    for num_channels in num_channels_per_animation {
        animation_info.push(primitives::AnimationInfo {
            num_channels,
            channels_offset,
        });

        channels_offset += num_channels;
    }

    let animation_info = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("animation {} animation info", name)),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&animation_info),
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("animation {} channels bind group", name)),
        layout: &resources.channels_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: inputs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: outputs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: channels_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: max_num_channels_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: animation_info.as_entire_binding(),
            },
        ],
    });

    (bind_group, max_num_channels)
}

pub fn create_scale_channel_bind_group(
    device: &wgpu::Device,
    resources: &RenderResources,
    animated_model: &AnimatedModel,
) -> (wgpu::BindGroup, u32) {
    create_channel_bind_group(
        device,
        "scales",
        resources,
        animated_model
            .animations
            .iter()
            .flat_map(|animation| animation.scale_channels.iter())
            .map(|channel| {
                (
                    channel.inputs.clone(),
                    channel.outputs.clone(),
                    channel.node_index as u32,
                )
            }),
        animated_model
            .animations
            .iter()
            .map(|animation| animation.scale_channels.len() as u32),
    )
}

pub fn create_translation_channel_bind_group(
    device: &wgpu::Device,
    resources: &RenderResources,
    animated_model: &AnimatedModel,
) -> (wgpu::BindGroup, u32) {
    create_channel_bind_group(
        device,
        "translations",
        resources,
        animated_model
            .animations
            .iter()
            .flat_map(|animation| animation.translation_channels.iter())
            .map(move |channel| {
                let outputs = channel
                    .outputs
                    .iter()
                    .map(|&vec| primitives::Vec3A::new(vec))
                    .collect::<Vec<_>>();

                (channel.inputs.clone(), outputs, channel.node_index as u32)
            }),
        animated_model
            .animations
            .iter()
            .map(|animation| animation.translation_channels.len() as u32),
    )
}

pub fn create_rotation_channel_bind_group(
    device: &wgpu::Device,
    resources: &RenderResources,
    animated_model: &AnimatedModel,
) -> (wgpu::BindGroup, u32) {
    create_channel_bind_group(
        device,
        "rotations",
        resources,
        animated_model
            .animations
            .iter()
            .flat_map(|animation| animation.rotation_channels.iter())
            .map(move |channel| {
                (
                    channel.inputs.clone(),
                    channel.outputs.clone(),
                    channel.node_index as u32,
                )
            }),
        animated_model
            .animations
            .iter()
            .map(|animation| animation.rotation_channels.len() as u32),
    )
}
