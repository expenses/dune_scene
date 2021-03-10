mod model_loading;
mod resources_and_pipelines;

use cascaded_shadow_maps::CascadedShadowMaps;
use model_loading::Scene;
use rand::Rng;
use resources_and_pipelines::{Pipelines, RenderResources};
use ultraviolet::Vec3;
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

    let mut rng = rand::thread_rng();

    let num_ships = 100;
    let ship_positions: Vec<_> = (0..num_ships)
        .map(|_| primitives::Transform {
            translation: Vec3::new(
                rng.gen_range(-2.0..=2.0),
                rng.gen_range(0.49..=0.51),
                rng.gen_range(-2.0..=2.0),
            ),
            y_rotation: rng.gen_range(0.0..360.0_f32).to_radians(),
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

    let particles_per_ship = 50 * 2;
    let num_particles = num_ships * particles_per_ship;
    let particles = vec![primitives::Particle::default(); num_particles as usize];

    let particles_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("particles buffer"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::cast_slice(&particles),
    });

    let particles_buffer_info = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("particles info buffer"),
        usage: wgpu::BufferUsage::STORAGE,
        contents: bytemuck::bytes_of(&primitives::ParticlesBufferInfo::default()),
    });

    let particles_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("particles bind group"),
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
    });

    let scene_bytes = include_bytes!("../models/dune.glb");
    let scene = Scene::load(scene_bytes, &device, &queue, &resources)?;
    println!(
        "Camera z near: {}, Camera z far: {}",
        scene.camera_z_near, scene.camera_z_far
    );

    let ship_bytes = include_bytes!("../models/ship.glb");
    let ship = model_loading::Ship::load(ship_bytes, &device, &queue, &resources)?;

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

    let mut time_since_start = 0.0;
    let time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("time buffer"),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        contents: bytemuck::bytes_of(&primitives::Time {
            time_since_start: 0.0,
        }),
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

    let mut split_cascades = [0.0, 0.4, 0.6, 1.0];

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

    let mut render_sun_dir = false;
    let mut move_ships = true;
    let mut render_ships = true;
    let mut render_ship_shadows = true;

    use winit::event::*;
    use winit::event_loop::*;

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

                    camera = scene.create_camera(width, height);
                    queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&camera));

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
                _ => {}
            },
            Event::MainEventsCleared => window.request_redraw(),
            Event::RedrawRequested(_) => match swap_chain.get_current_frame() {
                Ok(frame) => {
                    time_since_start += 1.0 / 60.0;
                    queue.write_buffer(
                        &time_buffer,
                        0,
                        bytemuck::bytes_of(&primitives::Time { time_since_start }),
                    );

                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("render encoder"),
                        });

                    if move_ships {
                        let mut compute_pass =
                            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                label: Some("ship movement pass"),
                            });

                        compute_pass.set_pipeline(&pipelines.ship_movement_pipeline);
                        compute_pass.set_bind_group(0, &bind_group, &[]);
                        compute_pass.set_bind_group(1, &ship_bind_group, &[]);
                        compute_pass.set_bind_group(2, &particles_bind_group, &[]);
                        compute_pass.dispatch(dispatch_count(num_ships, 64), 1, 1);
                    }

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

                    render_pass.set_pipeline(&pipelines.particles_pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.set_bind_group(1, &particles_bind_group, &[]);
                    render_pass.draw(0..num_particles, 0..1);

                    render_pass.set_pipeline(&pipelines.scene_pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.set_bind_group(1, &scene.texture_bind_group, &[]);
                    render_pass.set_bind_group(2, cascaded_shadow_maps.rendering_bind_group(), &[]);
                    render_pass.set_vertex_buffer(0, scene.vertices.slice(..));
                    render_pass.set_index_buffer(scene.indices.slice(..), INDEX_FORMAT);
                    render_pass.draw_indexed(0..scene.num_indices, 0, 0..1);

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
                            &mut move_ships,
                            &mut render_ships,
                            &mut render_ship_shadows,
                            &mut split_cascades,
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
    move_ships: &mut bool,
    render_ships: &mut bool,
    render_ship_shadows: &mut bool,
    split_cascades: &mut [f32; 4],
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

    ui.checkbox(imgui::im_str!("Move Ships"), move_ships);
    ui.checkbox(imgui::im_str!("Render Ships"), render_ships);
    ui.checkbox(imgui::im_str!("Render Ship Shadows"), render_ship_shadows);

    for i in 0..4 {
        let lower_bound = if i == 0 { 0.0 } else { split_cascades[i - 1] };
        let upper_bound = if i == 3 { 1.0 } else { split_cascades[i + 1] };

        dirty.csm |= imgui::Drag::new(&imgui::im_str!("Split Cascade {}", i))
            .range(lower_bound..=upper_bound)
            .speed(0.01)
            .build(&ui, &mut split_cascades[i]);
    }

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
