mod model_loading;
mod resources_and_pipelines;

use cascaded_shadow_maps::CascadedShadowMaps;
use model_loading::Scene;
use rand::Rng;
use resources_and_pipelines::{Pipelines, RenderResources};
use ultraviolet::{Mat4, Vec2, Vec3};
use wgpu::util::DeviceExt;

fn main() -> anyhow::Result<()> {
    #[cfg(not(feature = "wasm"))]
    {
        env_logger::init();
        pollster::block_on(run())
    }
    #[cfg(feature = "wasm")]
    {
        console_error_panic_hook::set_once();
        console_log::init_with_level(log::Level::Trace)?;
        wasm_bindgen_futures::spawn_local(async {
            if let Err(error) = run().await {
                println!("Error: {}", error);
            }
        });
        Ok(())
    }
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const FRAMEBUFFER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const INDEX_FORMAT: wgpu::IndexFormat = wgpu::IndexFormat::Uint16;

async fn run() -> anyhow::Result<()> {
    let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);

    let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!(
                "'request_adapter' failed. If you get this on linux, try installing the vulkan drivers for your gpu. \
                You can check that they're working properly by running `vulkaninfo` or `vkcube`."
            ))?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("device"),
                features: wgpu::Features::empty(),
                limits: wgpu::Limits {
                    max_storage_buffers_per_shader_stage: 7,
                    ..Default::default()
                },
            },
            None,
        )
        .await?;

    let resources = RenderResources::new(&device);

    let cascaded_shadow_maps = CascadedShadowMaps::new(&device, 1024);

    let mut settings = primitives::Settings {
        base_colour: Vec3::new(0.8, 0.535, 0.297),
        detail_map_scale: 1.5,
        ambient_lighting: Vec3::broadcast(0.024),
        roughness: 0.207,
        mode: primitives::Mode::Full as u32,
        specular_factor: 1.0,
        ship_movement_bounds: 2.5,
    };

    let settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("settings buffer"),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        contents: bytemuck::bytes_of(&settings),
    });

    let mut tonemapper_params = TonemapperParams {
        toe: 1.0,
        shoulder: 0.987,
        max_luminance: 1.0,
        grey_in: 0.5,
        grey_out: 0.5,
        mode: if cfg!(feature = "wasm") {
            primitives::TonemapperMode::WasmGammaCorrect
        } else {
            primitives::TonemapperMode::On
        },
    };

    let tonemapper_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Tonemapper uniform buffer"),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        contents: bytemuck::bytes_of(&tonemapper_params.convert()),
    });

    let mut time_since_start = 0.0;
    let time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("time buffer"),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        contents: bytemuck::bytes_of(&primitives::Time {
            time_since_start: 0.0,
            ..Default::default()
        }),
    });

    let mut rng = rand::thread_rng();

    let num_ships = 100;
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

    let num_land_craft = 200;
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

    let scene_bytes = include_bytes!("../models/dune.glb");
    let mut scene = Scene::load(scene_bytes, &device, &queue, &resources)?;
    println!(
        "Camera z near: {}, Camera z far: {}",
        scene.camera_z_near, scene.camera_z_far
    );

    let ship_bytes = include_bytes!("../models/ship.glb");
    let ship = model_loading::Ship::load(ship_bytes, &device, &queue, &resources)?;

    let land_craft_bytes = include_bytes!("../models/landcraft.glb");
    let land_craft = model_loading::LandCraft::load(land_craft_bytes, &device, &queue, &resources)?;

    let explosion_bytes = include_bytes!("../models/mouse.glb");
    let explosion = model_loading::Explosion::load(explosion_bytes, &device, &queue, &resources)?;

    // Now we can create a window.

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop)?;

    #[cfg(feature = "wasm")]
    {
        window.set_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

        use winit::platform::web::WindowExtWebSys;
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");
    }

    let window_size = window.inner_size();
    let width = window_size.width;
    let height = window_size.height;

    let mut camera = scene.create_camera(width, height);

    let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("camera buffer"),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        contents: bytemuck::bytes_of(&camera),
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bind group"),
        layout: &resources.main_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: scene.sun_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&resources.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: settings_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: time_buffer.as_entire_binding(),
            },
        ],
    });

    let surface = unsafe { instance.create_surface(&window) };

    let display_format = adapter.get_swap_chain_preferred_format(&surface);

    let pipelines = Pipelines::new(&device, display_format, &resources, &cascaded_shadow_maps);

    let mut imgui = imgui::Context::create();
    let mut imgui_platform = imgui_winit_support::WinitPlatform::init(&mut imgui);
    imgui_platform.attach_window(
        imgui.io_mut(),
        &window,
        imgui_winit_support::HiDpiMode::Default,
    );
    imgui.set_ini_filename(None);

    let mut imgui_renderer = imgui_wgpu::Renderer::new(
        &mut imgui,
        &device,
        &queue,
        imgui_wgpu::RendererConfig {
            texture_format: display_format,
            ..Default::default()
        },
    );

    let mut swap_chain_descriptor = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
        format: display_format,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
    };

    let mut swap_chain = device.create_swap_chain(&surface, &swap_chain_descriptor);

    let mut depth_texture = create_texture(
        &device,
        "depth texture",
        width,
        height,
        DEPTH_FORMAT,
        wgpu::TextureUsage::RENDER_ATTACHMENT,
    );

    let (mut framebuffer_texture, mut tonemapper_bind_group) =
        framebuffer_and_tonemapper_bind_group(
            &device,
            width,
            height,
            &resources,
            &tonemapper_uniform_buffer,
        );

    let mut cascade_split_lambda = 0.1;
    let mut split_cascades = cascaded_shadow_maps::calculate_split_cascades(
        scene.camera_z_near,
        scene.camera_z_far,
        cascade_split_lambda,
    );

    cascaded_shadow_maps.update_params(
        cascaded_shadow_maps::CameraParams {
            projection_view: camera.perspective_view,
            far_clip: scene.camera_z_far,
            near_clip: scene.camera_z_near,
        },
        split_cascades,
        scene.sun_facing,
        &queue,
    );

    let height_map_texture = {
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
        render_pass.set_vertex_buffer(0, scene.vertices.slice(..));
        render_pass.set_index_buffer(scene.indices.slice(..), INDEX_FORMAT);
        render_pass.draw_indexed(0..scene.num_indices, 0, 0..1);

        drop(render_pass);

        queue.submit(Some(encoder.finish()));

        height_map_texture
    };

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
                resource: wgpu::BindingResource::TextureView(&height_map_texture),
            },
        ],
    });

    let (sample_scales_bind_group, num_scale_channels) = create_channel_bind_group(
        &device,
        "scales",
        &resources,
        explosion.animation.scale_channels.iter().map(|channel| {
            (
                channel.inputs.clone(),
                channel.outputs.clone(),
                channel.node_index as u32,
            )
        }),
    );

    let (sample_translations_bind_group, num_translation_channels) = create_channel_bind_group(
        &device,
        "translations",
        &resources,
        explosion
            .animation
            .translation_channels
            .iter()
            .map(move |channel| {
                let outputs = channel
                    .outputs
                    .iter()
                    .map(|&vec| primitives::Vec3A::new(vec))
                    .collect::<Vec<_>>();

                (channel.inputs.clone(), outputs, channel.node_index as u32)
            }),
    );

    let (sample_rotations_bind_group, num_rotation_channels) = create_channel_bind_group(
        &device,
        "rotations",
        &resources,
        explosion
            .animation
            .rotation_channels
            .iter()
            .map(move |channel| {
                let outputs = channel
                    .outputs
                    .iter()
                    .map(|&rotor| primitives::Rotor {
                        s: rotor.s,
                        bv: Vec3::new(rotor.bv.xy, rotor.bv.xz, rotor.bv.yz),
                    })
                    .collect::<Vec<_>>();

                (channel.inputs.clone(), outputs, channel.node_index as u32)
            }),
    );

    let num_explosions = 50;

    let explosion_animation_states: Vec<_> = (0..num_explosions)
        .map(|_| primitives::AnimationState {
            time: rng.gen_range(0.0..=explosion.animation.total_time()),
            animation_duration: explosion.animation.total_time(),
        })
        .collect();

    let animation_bind_group = create_animation_bind_group(
        &device,
        &resources,
        num_explosions as usize,
        &explosion
            .depth_first_nodes
            .iter()
            .map(|(node_index, parent_index)| primitives::NodeAndParent {
                node_index: *node_index as u32,
                parent_index: parent_index.map(|p| p as i32).unwrap_or(-1),
            })
            .collect::<Vec<_>>(),
        &explosion
            .joint_indices_to_node_indices
            .iter()
            .map(|index| *index as u32)
            .collect::<Vec<_>>(),
        &explosion.inverse_bind_matrices,
        &explosion.animation_joints.global_transforms,
        &explosion.animation_joints.local_transforms,
        &explosion_animation_states,
    );

    let position_instances: Vec<_> = (0..num_explosions)
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

    let mut render_sun_dir = false;
    let mut move_vehicles = true;
    let mut render_ships = true;
    let mut render_ship_shadows = true;

    use winit::dpi::*;
    use winit::event::*;
    use winit::event_loop::*;

    let mut previous_cursor_position = Vec2::zero();
    let mut mouse_down = false;

    event_loop.run(move |event, _, control_flow| {
        imgui_platform.handle_event(imgui.io_mut(), &window, &event);

        match event {
            Event::WindowEvent { ref event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => {
                    let width = size.width as u32;
                    let height = size.height as u32;

                    swap_chain_descriptor.width = width;
                    swap_chain_descriptor.height = height;

                    swap_chain = device.create_swap_chain(&surface, &swap_chain_descriptor);

                    depth_texture = create_texture(
                        &device,
                        "depth texture",
                        width,
                        height,
                        DEPTH_FORMAT,
                        wgpu::TextureUsage::RENDER_ATTACHMENT,
                    );

                    let (new_framebuffer_texture, new_tonemapper_bind_group) =
                        framebuffer_and_tonemapper_bind_group(
                            &device,
                            width,
                            height,
                            &resources,
                            &tonemapper_uniform_buffer,
                        );

                    framebuffer_texture = new_framebuffer_texture;
                    tonemapper_bind_group = new_tonemapper_bind_group;

                    update_camera_and_shadows(
                        &mut camera,
                        &camera_buffer,
                        &swap_chain_descriptor,
                        &cascaded_shadow_maps,
                        &queue,
                        &scene,
                        split_cascades,
                    );
                }
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Left,
                    ..
                } => {
                    mouse_down = *state == ElementState::Pressed;
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let delta = match delta {
                        MouseScrollDelta::LineDelta(_, y) => -*y,
                        MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => {
                            *y as f32 / -200.0
                        }
                    };

                    scene.orbit.zoom(delta);
                    update_camera_and_shadows(
                        &mut camera,
                        &camera_buffer,
                        &swap_chain_descriptor,
                        &cascaded_shadow_maps,
                        &queue,
                        &scene,
                        split_cascades,
                    );
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let position = position.to_logical::<f32>(window.scale_factor());
                    let position = Vec2::new(position.x, position.y);

                    if mouse_down {
                        let delta = position - previous_cursor_position;
                        scene.orbit.rotate(delta);

                        update_camera_and_shadows(
                            &mut camera,
                            &camera_buffer,
                            &swap_chain_descriptor,
                            &cascaded_shadow_maps,
                            &queue,
                            &scene,
                            split_cascades,
                        );
                    }

                    previous_cursor_position = position;
                }
                _ => {}
            },
            Event::MainEventsCleared => window.request_redraw(),
            Event::RedrawRequested(_) => match swap_chain.get_current_frame() {
                Ok(frame) => {
                    let delta_time = if move_vehicles { 1.0 / 60.0 } else { 0.0 };
                    time_since_start += delta_time;
                    queue.write_buffer(
                        &time_buffer,
                        0,
                        bytemuck::bytes_of(&primitives::Time {
                            time_since_start,
                            delta_time,
                        }),
                    );

                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("render encoder"),
                        });

                    let mut compute_pass =
                        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("compute pass"),
                        });

                    if num_translation_channels > 0 {
                        compute_pass.set_pipeline(&pipelines.sample_translations_pipeline);
                        compute_pass.set_bind_group(0, &animation_bind_group, &[]);
                        compute_pass.set_bind_group(1, &sample_translations_bind_group, &[]);
                        compute_pass.dispatch(
                            dispatch_count(num_explosions * num_translation_channels, 64),
                            1,
                            1,
                        );
                    }

                    if num_scale_channels > 0 {
                        compute_pass.set_pipeline(&pipelines.sample_scales_pipeline);
                        compute_pass.set_bind_group(0, &animation_bind_group, &[]);
                        compute_pass.set_bind_group(1, &sample_scales_bind_group, &[]);
                        compute_pass.dispatch(
                            dispatch_count(num_explosions * num_scale_channels, 64),
                            1,
                            1,
                        );
                    }

                    if num_rotation_channels > 0 {
                        compute_pass.set_pipeline(&pipelines.sample_rotations_pipeline);
                        compute_pass.set_bind_group(0, &animation_bind_group, &[]);
                        compute_pass.set_bind_group(1, &sample_rotations_bind_group, &[]);
                        compute_pass.dispatch(
                            dispatch_count(num_explosions * num_rotation_channels, 64),
                            1,
                            1,
                        );
                    }

                    compute_pass.set_pipeline(&pipelines.compute_joint_transforms_pipeline);
                    compute_pass.set_bind_group(0, &animation_bind_group, &[]);
                    compute_pass.set_bind_group(1, &bind_group, &[]);
                    compute_pass.dispatch(dispatch_count(num_explosions, 64), 1, 1);

                    compute_pass.set_pipeline(&pipelines.particles_movement_pipeline);
                    compute_pass.set_bind_group(0, &bind_group, &[]);

                    compute_pass.set_bind_group(1, &exhaust_particles_bind_group, &[]);
                    compute_pass.dispatch(dispatch_count(num_exhaust_particles, 64), 1, 1);

                    compute_pass.set_bind_group(1, &smoke_particles_bind_group, &[]);
                    compute_pass.dispatch(dispatch_count(num_smoke_particles, 64), 1, 1);

                    compute_pass.set_bind_group(1, &sand_particles_bind_group, &[]);
                    compute_pass.dispatch(dispatch_count(num_sand_particles, 64), 1, 1);

                    if move_vehicles {
                        compute_pass.set_pipeline(&pipelines.land_craft_movement_pipeline);
                        compute_pass.set_bind_group(0, &bind_group, &[]);
                        compute_pass.set_bind_group(1, &land_craft_bind_group, &[]);
                        compute_pass.set_bind_group(2, &smoke_particles_bind_group, &[]);
                        compute_pass.set_bind_group(3, &sand_particles_bind_group, &[]);
                        compute_pass.dispatch(dispatch_count(num_land_craft, 64), 1, 1);

                        compute_pass.set_pipeline(&pipelines.ship_movement_pipeline);
                        compute_pass.set_bind_group(0, &bind_group, &[]);
                        compute_pass.set_bind_group(1, &ship_bind_group, &[]);
                        compute_pass.set_bind_group(2, &exhaust_particles_bind_group, &[]);
                        compute_pass.dispatch(dispatch_count(num_ships, 64), 1, 1);
                    }

                    drop(compute_pass);

                    let labels = ["near shadow pass", "middle shadow pass", "far shadow pass"];
                    let shadow_textures = cascaded_shadow_maps.textures();
                    let light_projection_bind_groups =
                        cascaded_shadow_maps.light_projection_bind_groups();

                    for i in 0..3 {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some(labels[i]),
                                color_attachments: &[],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachmentDescriptor {
                                        attachment: &shadow_textures[i],
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(1.0),
                                            store: true,
                                        }),
                                        stencil_ops: None,
                                    },
                                ),
                            });

                        if render_ship_shadows {
                            render_pass.set_pipeline(&pipelines.ship_shadows_pipeline);
                            render_pass.set_bind_group(0, &light_projection_bind_groups[i], &[]);
                            render_pass.set_bind_group(1, &ship_bind_group, &[]);
                            render_pass.set_vertex_buffer(0, ship.vertices.slice(..));
                            render_pass.set_index_buffer(ship.indices.slice(..), INDEX_FORMAT);
                            render_pass.draw_indexed(0..ship.num_indices, 0, 0..num_ships);
                        }

                        render_pass.set_pipeline(&pipelines.land_craft_shadows_pipeline);
                        render_pass.set_bind_group(0, &light_projection_bind_groups[i], &[]);
                        render_pass.set_bind_group(1, &land_craft_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, land_craft.vertices.slice(..));
                        render_pass.set_index_buffer(land_craft.indices.slice(..), INDEX_FORMAT);
                        render_pass.draw_indexed(0..land_craft.num_indices, 0, 0..num_land_craft);

                        render_pass.set_pipeline(&pipelines.scene_shadows_pipeline);
                        render_pass.set_bind_group(0, &light_projection_bind_groups[i], &[]);
                        render_pass.set_vertex_buffer(0, scene.vertices.slice(..));
                        render_pass.set_index_buffer(scene.indices.slice(..), INDEX_FORMAT);
                        render_pass.draw_indexed(0..scene.num_indices, 0, 0..1);
                    }

                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("main render pass"),
                        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: &framebuffer_texture,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: true,
                            },
                        }],
                        depth_stencil_attachment: Some(
                            wgpu::RenderPassDepthStencilAttachmentDescriptor {
                                attachment: &depth_texture,
                                depth_ops: Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(1.0),
                                    store: true,
                                }),
                                stencil_ops: None,
                            },
                        ),
                    });

                    render_pass.set_pipeline(&pipelines.explosions_pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.set_bind_group(1, &explosion.texture_bind_group, &[]);
                    render_pass.set_bind_group(2, &animation_bind_group, &[]);
                    render_pass.set_vertex_buffer(0, explosion.vertices.slice(..));
                    render_pass.set_vertex_buffer(1, position_instances_buffer.slice(..));
                    render_pass.set_index_buffer(explosion.indices.slice(..), INDEX_FORMAT);
                    render_pass.draw_indexed(0..explosion.num_indices, 0, 0..num_explosions);

                    if render_ships {
                        render_pass.set_pipeline(&pipelines.ship_pipeline);
                        render_pass.set_bind_group(0, &bind_group, &[]);
                        render_pass.set_bind_group(1, &ship_bind_group, &[]);
                        render_pass.set_bind_group(2, &ship.texture_bind_group, &[]);
                        render_pass.set_bind_group(
                            3,
                            cascaded_shadow_maps.rendering_bind_group(),
                            &[],
                        );
                        render_pass.set_vertex_buffer(0, ship.vertices.slice(..));
                        render_pass.set_index_buffer(ship.indices.slice(..), INDEX_FORMAT);
                        render_pass.draw_indexed(0..ship.num_indices, 0, 0..num_ships);
                    }

                    render_pass.set_pipeline(&pipelines.land_craft_pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.set_bind_group(1, &land_craft_bind_group, &[]);
                    render_pass.set_bind_group(2, &land_craft.texture_bind_group, &[]);
                    render_pass.set_bind_group(3, cascaded_shadow_maps.rendering_bind_group(), &[]);
                    render_pass.set_vertex_buffer(0, land_craft.vertices.slice(..));
                    render_pass.set_index_buffer(land_craft.indices.slice(..), INDEX_FORMAT);
                    render_pass.draw_indexed(0..land_craft.num_indices, 0, 0..num_land_craft);

                    render_pass.set_pipeline(&pipelines.scene_pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.set_bind_group(1, &scene.texture_bind_group, &[]);
                    render_pass.set_bind_group(2, cascaded_shadow_maps.rendering_bind_group(), &[]);
                    render_pass.set_vertex_buffer(0, scene.vertices.slice(..));
                    render_pass.set_index_buffer(scene.indices.slice(..), INDEX_FORMAT);
                    render_pass.draw_indexed(0..scene.num_indices, 0, 0..1);

                    render_pass.set_pipeline(&pipelines.particles_pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);

                    render_pass.set_bind_group(1, &sand_particles_bind_group, &[]);
                    render_pass.draw(0..num_sand_particles * 6, 0..1);

                    render_pass.set_bind_group(1, &smoke_particles_bind_group, &[]);
                    render_pass.draw(0..num_smoke_particles * 6, 0..1);

                    render_pass.set_bind_group(1, &exhaust_particles_bind_group, &[]);
                    render_pass.draw(0..num_exhaust_particles * 6, 0..1);

                    drop(render_pass);

                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("tonemap render pass"),
                        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: &frame.output.view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: true,
                            },
                        }],
                        depth_stencil_attachment: None,
                    });

                    render_pass.set_pipeline(&pipelines.tonemap_pipeline);
                    render_pass.set_bind_group(0, &tonemapper_bind_group, &[]);
                    render_pass.draw(0..3, 0..1);

                    drop(render_pass);

                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("lines render pass"),
                        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: &frame.output.view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: true,
                            },
                        }],
                        depth_stencil_attachment: Some(
                            wgpu::RenderPassDepthStencilAttachmentDescriptor {
                                attachment: &depth_texture,
                                depth_ops: Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: true,
                                }),
                                stencil_ops: None,
                            },
                        ),
                    });

                    if render_sun_dir {
                        render_pass.set_pipeline(&pipelines.sun_dir_pipeline);
                        render_pass.set_bind_group(0, &bind_group, &[]);
                        render_pass.draw(0..2, 0..1);
                    }

                    drop(render_pass);

                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("ui render pass"),
                        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: &frame.output.view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: true,
                            },
                        }],
                        depth_stencil_attachment: None,
                    });

                    imgui_platform
                        .prepare_frame(imgui.io_mut(), &window)
                        .expect("Failed to prepare frame");
                    let ui = imgui.frame();

                    {
                        let dirty = draw_ui(
                            &ui,
                            &mut settings,
                            &mut tonemapper_params,
                            &mut render_sun_dir,
                            &mut move_vehicles,
                            &mut render_ships,
                            &mut render_ship_shadows,
                            &mut cascade_split_lambda,
                        );

                        if dirty.settings {
                            queue.write_buffer(&settings_buffer, 0, bytemuck::bytes_of(&settings));
                        }

                        if dirty.tonemapper {
                            queue.write_buffer(
                                &tonemapper_uniform_buffer,
                                0,
                                bytemuck::bytes_of(&tonemapper_params.convert()),
                            );
                        }

                        if dirty.csm {
                            split_cascades = cascaded_shadow_maps::calculate_split_cascades(
                                scene.camera_z_near,
                                scene.camera_z_far,
                                cascade_split_lambda,
                            );
                            cascaded_shadow_maps.update_params(
                                cascaded_shadow_maps::CameraParams {
                                    projection_view: camera.perspective_view,
                                    far_clip: scene.camera_z_far,
                                    near_clip: scene.camera_z_near,
                                },
                                split_cascades,
                                scene.sun_facing,
                                &queue,
                            );
                        };

                        imgui_renderer
                            .render(ui.render(), &queue, &device, &mut render_pass)
                            .expect("Rendering failed");
                    }

                    drop(render_pass);

                    queue.submit(Some(encoder.finish()));
                }
                Err(error) => println!("Swap chain error: {:?}", error),
            },
            _ => {}
        }
    });
}

fn create_texture(
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

fn framebuffer_and_tonemapper_bind_group(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    resources: &RenderResources,
    tonemapper_uniform_buffer: &wgpu::Buffer,
) -> (wgpu::TextureView, wgpu::BindGroup) {
    let framebuffer_texture = create_texture(
        &device,
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

#[derive(Copy, Clone)]
struct TonemapperParams {
    toe: f32,
    shoulder: f32,
    max_luminance: f32,
    grey_in: f32,
    grey_out: f32,
    mode: primitives::TonemapperMode,
}

impl TonemapperParams {
    // Based on https://www.desmos.com/calculator/0eo9pzo1at.
    fn convert(self) -> primitives::TonemapperSettings {
        let TonemapperParams {
            toe,
            shoulder,
            max_luminance,
            grey_in,
            grey_out,
            mode,
        } = self;

        let a = toe;
        let d = shoulder;

        let denominator = (max_luminance.powf(a * d) - grey_in.powf(a * d)) * grey_out;

        let b = (-grey_in.powf(a) + max_luminance.powf(a) * grey_out) / denominator;

        let c = (max_luminance.powf(a * d) * grey_in.powf(a)
            - max_luminance.powf(a) * grey_in.powf(a * d) * grey_out)
            / denominator;

        let mode = mode as u32;

        primitives::TonemapperSettings { a, b, c, d, mode }
    }
}

fn draw_ui(
    ui: &imgui::Ui,
    settings: &mut primitives::Settings,
    tonemapper_params: &mut TonemapperParams,
    render_sun_dir: &mut bool,
    move_vehicles: &mut bool,
    render_ships: &mut bool,
    render_ship_shadows: &mut bool,
    cascade_split_lambda: &mut f32,
) -> DirtyObjects {
    let mut dirty = DirtyObjects::default();

    let mut base_colour: [f32; 3] = settings.base_colour.into();

    if imgui::ColorPicker::new(imgui::im_str!("Colour"), &mut base_colour).build(&ui) {
        settings.base_colour = base_colour.into();
        dirty.settings = true;
    }

    let mut ambient_lighting: [f32; 3] = settings.ambient_lighting.into();

    if imgui::ColorPicker::new(imgui::im_str!("Ambient Lighting"), &mut ambient_lighting).build(&ui)
    {
        settings.ambient_lighting = ambient_lighting.into();
        dirty.settings = true;
    }

    dirty.settings |= imgui::Drag::new(imgui::im_str!("Detail Scale"))
        .range(0.0..=10.0)
        .speed(0.05)
        .build(&ui, &mut settings.detail_map_scale);

    dirty.settings |= imgui::Drag::new(imgui::im_str!("Roughness"))
        .range(0.0..=1.0)
        .speed(0.005)
        .build(&ui, &mut settings.roughness);

    dirty.settings |= imgui::Drag::new(imgui::im_str!("Specular Factor"))
        .range(0.0..=2.0)
        .speed(0.005)
        .build(&ui, &mut settings.specular_factor);

    for (mode, index) in primitives::Mode::iter() {
        dirty.settings |= ui.radio_button(&imgui::im_str!("{:?}", mode), &mut settings.mode, index);
    }

    ui.checkbox(imgui::im_str!("Render Sun Direction"), render_sun_dir);
    ui.checkbox(imgui::im_str!("Move Vehicles"), move_vehicles);
    ui.checkbox(imgui::im_str!("Render Ships"), render_ships);
    ui.checkbox(imgui::im_str!("Render Ship Shadows"), render_ship_shadows);

    dirty.csm |= imgui::Drag::new(&imgui::im_str!("Cascade Split Lambda"))
        .range(0.0..=1.0)
        .speed(0.01)
        .build(&ui, cascade_split_lambda);

    dirty.settings |= imgui::Drag::new(imgui::im_str!("Ship Movement Bounds"))
        .range(0.0..=2.5)
        .speed(0.01)
        .build(&ui, &mut settings.ship_movement_bounds);

    for mode in primitives::TonemapperMode::iter() {
        dirty.tonemapper |= ui.radio_button(
            &imgui::im_str!("Tonemapper {:?}", mode),
            &mut tonemapper_params.mode,
            mode,
        );
    }

    dirty.tonemapper |= imgui::Drag::new(imgui::im_str!("Tonemapper - Toe"))
        .range(1.0..=3.0)
        .speed(0.005)
        .build(&ui, &mut tonemapper_params.toe);

    dirty.tonemapper |= imgui::Drag::new(imgui::im_str!("Tonemapper - Shoulder"))
        .range(0.5..=2.0)
        .speed(0.005)
        .build(&ui, &mut tonemapper_params.shoulder);

    dirty.tonemapper |= imgui::Drag::new(imgui::im_str!("Tonemapper - Max Luminance"))
        .range(0.0..=30.0)
        .speed(0.1)
        .build(&ui, &mut tonemapper_params.max_luminance);

    dirty.tonemapper |= imgui::Drag::new(imgui::im_str!("Tonemapper - Grey In"))
        .range(0.0..=tonemapper_params.max_luminance / 2.0)
        .speed(0.05)
        .build(&ui, &mut tonemapper_params.grey_in);

    dirty.tonemapper |= imgui::Drag::new(imgui::im_str!("Tonemapper - Grey Out"))
        .range(0.0..=0.5)
        .speed(0.005)
        .build(&ui, &mut tonemapper_params.grey_out);

    dirty
}

#[derive(Default)]
struct DirtyObjects {
    settings: bool,
    tonemapper: bool,
    csm: bool,
}

const fn dispatch_count(num: u32, group_size: u32) -> u32 {
    let mut count = num / group_size;
    let rem = num % group_size;
    if rem != 0 {
        count += 1;
    }

    count
}

fn update_camera_and_shadows(
    camera: &mut primitives::Camera,
    camera_buffer: &wgpu::Buffer,
    swap_chain_descriptor: &wgpu::SwapChainDescriptor,
    cascaded_shadow_maps: &CascadedShadowMaps,
    queue: &wgpu::Queue,
    scene: &Scene,
    split_cascades: [f32; 4],
) {
    *camera = scene.create_camera(swap_chain_descriptor.width, swap_chain_descriptor.height);
    queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(camera));

    cascaded_shadow_maps.update_params(
        cascaded_shadow_maps::CameraParams {
            projection_view: camera.perspective_view,
            far_clip: scene.camera_z_far,
            near_clip: scene.camera_z_near,
        },
        split_cascades,
        scene.sun_facing,
        &queue,
    );
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

fn create_animation_bind_group(
    device: &wgpu::Device,
    resources: &RenderResources,
    num_instances: usize,
    depth_first_nodes: &[primitives::NodeAndParent],
    joint_indices_to_node_indices: &[u32],
    inverse_bind_matrices: &[Mat4],
    global_transforms: &[ultraviolet::Similarity3],
    local_transforms: &[ultraviolet::Similarity3],
    animation_states: &[primitives::AnimationState],
) -> wgpu::BindGroup {
    let num_joints = joint_indices_to_node_indices.len();
    let num_nodes = depth_first_nodes.len();

    println!("{}/{}", num_joints, num_nodes);

    debug_assert_eq!(inverse_bind_matrices.len(), num_joints);
    debug_assert_eq!(global_transforms.len(), num_nodes);

    let joints = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("animation joints"),
        usage: wgpu::BufferUsage::STORAGE,
        size: (num_joints * num_instances * std::mem::size_of::<Mat4>()) as u64,
        mapped_at_creation: false,
    });

    let info = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animation info"),
        usage: wgpu::BufferUsage::UNIFORM,
        contents: bytemuck::bytes_of(&primitives::AnimationInfo {
            num_joints: num_joints as u32,
            num_nodes: num_nodes as u32,
            num_instances: num_instances as u32,
        }),
    });

    let mut full_local_transforms = Vec::new();
    for _ in 0..num_instances {
        full_local_transforms.extend(local_transforms.iter().map(|sim| primitives::Similarity {
            translation: sim.translation,
            scale: sim.scale,
            rotation: primitives::Rotor {
                s: sim.rotation.s,
                bv: Vec3::new(sim.rotation.bv.xy, sim.rotation.bv.xz, sim.rotation.bv.yz),
            },
        }));
    }

    let local_transforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animation local transforms"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&full_local_transforms),
    });

    let animation_states = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animation states"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&animation_states),
    });

    let mut full_global_transforms = Vec::new();
    for _ in 0..num_instances {
        full_global_transforms.extend(
            global_transforms
                .iter()
                .map(|sim| sim.into_homogeneous_matrix()),
        );
    }

    debug_assert_eq!(full_global_transforms.len(), num_instances * num_nodes);

    let global_transforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animation global transforms"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&full_global_transforms),
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

    let inverse_bind_matrices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("animation inverse bind matrices"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(inverse_bind_matrices),
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
                resource: animation_states.as_entire_binding(),
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
                resource: inverse_bind_matrices.as_entire_binding(),
            },
        ],
    })
}

fn create_channel_bind_group<'a, T: bytemuck::Pod + Clone>(
    device: &wgpu::Device,
    name: &str,
    resources: &RenderResources,
    iter: impl Iterator<Item = (Vec<f32>, Vec<T>, u32)>,
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
        ],
    });

    (bind_group, channels.len() as u32)
}
