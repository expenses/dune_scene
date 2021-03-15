mod model_loading;
mod resources_and_pipelines;

use cascaded_shadow_maps::CascadedShadowMaps;
use model_loading::Scene;
use rand::Rng;
use resources_and_pipelines::{Pipelines, RenderResources};
use ultraviolet::{Vec2, Vec3};
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
                limits: wgpu::Limits::default(),
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

    let num_ships = 200;
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

    let num_land_craft = 400;
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
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&resources.clamp_sampler),
            },
        ],
    });

    let surface = unsafe { instance.create_surface(&window) };

    let display_format = adapter.get_swap_chain_preferred_format(&surface);

    let pipelines = Pipelines::new(&device, display_format, &resources, &cascaded_shadow_maps);

    const BACKGROUND: egui::Color32 = egui::Color32::from_rgba_premultiplied(64, 0, 0, 224);
    const INACTIVE: egui::Color32 = egui::Color32::from_rgba_premultiplied(48, 0, 0, 224);
    const ACTIVE: egui::Color32 = egui::Color32::from_rgba_premultiplied(64, 0, 0, 224);
    const HIGHLIGHT: egui::Color32 = egui::Color32::from_rgba_premultiplied(255, 224, 224, 224);

    let active_stroke = egui::Stroke::new(1.0, ACTIVE);
    let highlight_stroke = egui::Stroke::new(1.0, HIGHLIGHT);
    let white_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);

    let mut style = egui::style::Style::default();
    style.visuals.widgets.noninteractive.bg_fill = BACKGROUND;
    style.visuals.widgets.noninteractive.fg_stroke = white_stroke;
    style.visuals.widgets.noninteractive.bg_stroke = active_stroke;
    style.visuals.widgets.inactive.bg_fill = INACTIVE;
    style.visuals.widgets.hovered.bg_fill = ACTIVE;
    style.visuals.widgets.hovered.bg_stroke = highlight_stroke;
    style.visuals.widgets.active.bg_fill = ACTIVE;

    let mut egui_platform =
        egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
            physical_width: width as u32,
            physical_height: height as u32,
            scale_factor: window.scale_factor(),
            font_definitions: Default::default(),
            style,
        });

    let mut egui_renderpass =
        egui_wgpu_backend::RenderPass::new(&device, wgpu::TextureFormat::Bgra8UnormSrgb);

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

    // As zooming in/out crashes firefox with webgpu, we don't need to recreate these :^)

    #[cfg(feature = "wasm")]
    let ui_framebuffer = create_texture(
        &device,
        "ui framebuffer texture",
        width,
        height,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
    );

    #[cfg(feature = "wasm")]
    let ui_framebuffer_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ui framebuffer bind group"),
        layout: &resources.single_texture_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&ui_framebuffer),
        }],
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
        egui_platform.handle_event(&event);

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

                    if mouse_down
                        && !egui_platform.context().is_pointer_over_area()
                        && !egui_platform.context().is_using_pointer()
                    {
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

                    egui_platform.update_time(1.0 / 60.0);

                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("render encoder"),
                        });

                    let mut compute_pass =
                        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("compute pass"),
                        });

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

                    if render_sun_dir {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

                        render_pass.set_pipeline(&pipelines.sun_dir_pipeline);
                        render_pass.set_bind_group(0, &bind_group, &[]);
                        render_pass.draw(0..2, 0..1);
                    }

                    egui_platform.begin_frame();
                    egui::containers::Window::new("Controls").show(
                        &egui_platform.context(),
                        |ui| {
                            let dirty = draw_ui(
                                ui,
                                &mut settings,
                                &mut tonemapper_params,
                                &mut render_sun_dir,
                                &mut move_vehicles,
                                &mut render_ships,
                                &mut render_ship_shadows,
                                &mut cascade_split_lambda,
                            );

                            if dirty.settings {
                                queue.write_buffer(
                                    &settings_buffer,
                                    0,
                                    bytemuck::bytes_of(&settings),
                                );
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
                        },
                    );
                    let (_output, paint_commands) = egui_platform.end_frame();
                    let paint_jobs = egui_platform.context().tessellate(paint_commands);

                    let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
                        physical_width: swap_chain_descriptor.width,
                        physical_height: swap_chain_descriptor.height,
                        scale_factor: window.scale_factor() as f32,
                    };
                    egui_renderpass.update_texture(
                        &device,
                        &queue,
                        &egui_platform.context().texture(),
                    );
                    egui_renderpass.update_user_textures(&device, &queue);
                    egui_renderpass.update_buffers(
                        &device,
                        &queue,
                        &paint_jobs,
                        &screen_descriptor,
                    );

                    // Record all render passes.
                    egui_renderpass.execute(
                        &mut encoder,
                        #[cfg(feature = "wasm")]
                        &ui_framebuffer,
                        #[cfg(not(feature = "wasm"))]
                        &frame.output.view,
                        &paint_jobs,
                        &screen_descriptor,
                        #[cfg(feature = "wasm")]
                        Some(wgpu::Color::TRANSPARENT),
                        #[cfg(not(feature = "wasm"))]
                        None,
                    );

                    #[cfg(feature = "wasm")]
                    {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("blit ui render pass"),
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

                        render_pass.set_pipeline(&pipelines.blit_to_srgb_pipeline);
                        render_pass.set_bind_group(0, &bind_group, &[]);
                        render_pass.set_bind_group(1, &ui_framebuffer_bind_group, &[]);
                        render_pass.draw(0..3, 0..1);
                    }

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
    ui: &mut egui::Ui,
    settings: &mut primitives::Settings,
    tonemapper_params: &mut TonemapperParams,
    render_sun_dir: &mut bool,
    move_vehicles: &mut bool,
    render_ships: &mut bool,
    render_ship_shadows: &mut bool,
    cascade_split_lambda: &mut f32,
) -> DirtyObjects {
    let mut dirty = DirtyObjects::default();

    // todo: re-enable with a newer egui version.
    /*
    let mut base_colour = egui::color::Hsva::from_rgb(settings.base_colour.into());
    use egui::widgets::color_picker::Alpha;

    let response = egui::widgets::color_picker::color_edit_button_hsva(ui, &mut base_colour, Alpha::Opaque);

    if response.changed() || response.changed() {
        settings.base_colour = base_colour.to_rgb().into();
        dirty.settings = true;
    }
    */

    dirty.settings |= ui
        .add(
            egui::widgets::Slider::f32(&mut settings.detail_map_scale, 0.0..=10.0)
                .text("Detail Map Scale"),
        )
        .changed();

    dirty.settings |= ui
        .add(
            egui::widgets::Slider::f32(&mut settings.roughness, 0.0..=1.0)
                .text("Ground Specular Roughness"),
        )
        .changed();

    dirty.settings |= ui
        .add(
            egui::widgets::Slider::f32(&mut settings.specular_factor, 0.0..=2.0)
                .text("Ground Specular Multiplier"),
        )
        .changed();

    for (mode, index) in primitives::Mode::iter() {
        dirty.settings |= ui
            .radio_value(&mut settings.mode, index, format!("{:?}", mode))
            .changed();
    }

    ui.checkbox(render_sun_dir, "Render Sun Direction");
    ui.checkbox(move_vehicles, "Move Vehicles");
    ui.checkbox(render_ships, "Render Ships");
    ui.checkbox(render_ship_shadows, "Render Ship Shadows");

    dirty.csm |= ui
        .add(
            egui::widgets::Slider::f32(cascade_split_lambda, 0.0..=1.0)
                .text("Cascade Split Lambda"),
        )
        .changed();

    dirty.settings |= ui
        .add(
            egui::widgets::Slider::f32(&mut settings.ship_movement_bounds, 0.0..=2.5)
                .text("Ship Movement Bounds"),
        )
        .changed();

    for mode in primitives::TonemapperMode::iter() {
        dirty.tonemapper |= ui
            .radio_value(
                &mut tonemapper_params.mode,
                mode,
                format!("Tonemapper {:?}", mode),
            )
            .changed();
    }

    dirty.tonemapper |= ui
        .add(
            egui::widgets::Slider::f32(&mut tonemapper_params.toe, 1.0..=3.0)
                .text("Tonemapper - Toe"),
        )
        .changed();

    dirty.tonemapper |= ui
        .add(
            egui::widgets::Slider::f32(&mut tonemapper_params.shoulder, 0.5..=2.0)
                .text("Tonemapper - Shoulder"),
        )
        .changed();

    dirty.tonemapper |= ui
        .add(
            egui::widgets::Slider::f32(&mut tonemapper_params.max_luminance, 0.0..=30.0)
                .text("Tonemapper - Max Luminance"),
        )
        .changed();

    dirty.tonemapper |= ui
        .add(
            egui::widgets::Slider::f32(
                &mut tonemapper_params.grey_in,
                0.0..=tonemapper_params.max_luminance,
            )
            .text("Tonemapper - Grey In"),
        )
        .changed();

    dirty.tonemapper |= ui
        .add(
            egui::widgets::Slider::f32(&mut tonemapper_params.grey_out, 0.0..=0.5)
                .text("Tonemapper - Grey Out"),
        )
        .changed();

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
