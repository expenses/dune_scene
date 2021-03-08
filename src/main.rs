mod model_loading;

use cascaded_shadow_maps::CascadedShadowMaps;
use model_loading::Scene;
use primitives::{LineVertex, Vertex};
use rand::Rng;
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

    let mut ship_movement_settings = primitives::ShipMovementSettings { bounds: 2.5 };

    let ship_movement_settings_buffer =
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ship movement settings buffer"),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            contents: bytemuck::bytes_of(&ship_movement_settings),
        });

    let ship_movement_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ship movement bind group"),
        layout: &resources.ship_movement_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: ship_movement_settings_buffer.as_entire_binding(),
        }],
    });

    let scene_bytes = include_bytes!("../models/dune.glb");
    let scene = Scene::load(scene_bytes, &device, &queue, &resources.texture_bgl)?;
    println!(
        "Camera z near: {}, Camera z far: {}",
        scene.camera_z_near, scene.camera_z_far
    );

    let ship = model_loading::Ship::load(include_bytes!("../models/ship.glb"), &device, &queue)?;

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

    let mut cascade_split_lambda = 0.0;

    cascaded_shadow_maps.update_params(
        cascaded_shadow_maps::CameraParams {
            projection_view: camera.perspective_view,
            far_clip: scene.camera_z_far,
            near_clip: scene.camera_z_near,
        },
        scene.sun_facing,
        cascade_split_lambda,
        &queue,
    );

    let mut render_sun_dir = false;
    let mut move_ships = true;
    let mut render_ships = true;

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
                        scene.sun_facing,
                        cascade_split_lambda,
                        &queue,
                    );
                }
                _ => {}
            },
            Event::MainEventsCleared => window.request_redraw(),
            Event::RedrawRequested(_) => match swap_chain.get_current_frame() {
                Ok(frame) => {
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
                        compute_pass.set_bind_group(0, &ship_bind_group, &[]);
                        compute_pass.set_bind_group(1, &ship_movement_bind_group, &[]);
                        compute_pass.dispatch(dispatch_count(100, 64), 1, 1);
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

                        if render_ships {
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
                        render_pass.set_bind_group(
                            2,
                            cascaded_shadow_maps.rendering_bind_group(),
                            &[],
                        );
                        render_pass.set_vertex_buffer(0, ship.vertices.slice(..));
                        render_pass.set_index_buffer(ship.indices.slice(..), INDEX_FORMAT);
                        render_pass.draw_indexed(0..ship.num_indices, 0, 0..num_ships);
                    }

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
                            &mut cascade_split_lambda,
                            &mut ship_movement_settings,
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
                                scene.sun_facing,
                                cascade_split_lambda,
                                &queue,
                            );
                        };

                        if dirty.ship_movement_settings {
                            queue.write_buffer(
                                &ship_movement_settings_buffer,
                                0,
                                bytemuck::bytes_of(&ship_movement_settings),
                            );
                        }

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

/// All the permement resources that we can load before creating a window.
struct RenderResources {
    main_bgl: wgpu::BindGroupLayout,
    texture_bgl: wgpu::BindGroupLayout,
    tonemap_bgl: wgpu::BindGroupLayout,
    ship_bgl: wgpu::BindGroupLayout,
    ship_movement_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl RenderResources {
    fn new(device: &wgpu::Device) -> Self {
        let uniform = |binding, shader_stage| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: shader_stage,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let texture = |binding, shader_stage| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: shader_stage,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let sampler = |binding, shader_stage| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: shader_stage,
            ty: wgpu::BindingType::Sampler {
                filtering: false,
                comparison: false,
            },
            count: None,
        };

        let storage = |binding, shader_stage, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: shader_stage,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        Self {
            main_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bind group layout"),
                entries: &[
                    uniform(0, wgpu::ShaderStage::VERTEX),
                    uniform(1, wgpu::ShaderStage::FRAGMENT | wgpu::ShaderStage::VERTEX),
                    sampler(2, wgpu::ShaderStage::FRAGMENT),
                    uniform(3, wgpu::ShaderStage::FRAGMENT),
                ],
            }),
            texture_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture bind group layout"),
                entries: &[
                    texture(0, wgpu::ShaderStage::FRAGMENT),
                    texture(1, wgpu::ShaderStage::FRAGMENT),
                ],
            }),
            tonemap_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tonemapper bind group layout"),
                entries: &[
                    texture(0, wgpu::ShaderStage::FRAGMENT),
                    sampler(1, wgpu::ShaderStage::FRAGMENT),
                    uniform(2, wgpu::ShaderStage::FRAGMENT),
                ],
            }),
            ship_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ship bind group layout"),
                entries: &[storage(
                    0,
                    wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::COMPUTE,
                    false,
                )],
            }),
            ship_movement_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ship movement bind group layout"),
                entries: &[uniform(0, wgpu::ShaderStage::COMPUTE)],
            }),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("linear sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                ..Default::default()
            }),
        }
    }
}

struct Pipelines {
    scene_pipeline: wgpu::RenderPipeline,
    sun_dir_pipeline: wgpu::RenderPipeline,
    tonemap_pipeline: wgpu::RenderPipeline,
    ship_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    scene_shadows_pipeline: wgpu::RenderPipeline,
    ship_shadows_pipeline: wgpu::RenderPipeline,
    ship_movement_pipeline: wgpu::ComputePipeline,
}

impl Pipelines {
    fn new(
        device: &wgpu::Device,
        display_format: wgpu::TextureFormat,
        resources: &RenderResources,
        shadow_maps: &CascadedShadowMaps,
    ) -> Self {
        let fs_flat_colour = wgpu::include_spirv!("../shaders/compiled/flat_colour.frag.spv");
        let fs_flat_colour = device.create_shader_module(&fs_flat_colour);

        let main_bind_group_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("main bind group pipeline layout"),
                bind_group_layouts: &[&resources.main_bgl],
                push_constant_ranges: &[],
            });

        Self {
            scene_pipeline: {
                let scene_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("scene pipeline layout"),
                        bind_group_layouts: &[
                            &resources.main_bgl,
                            &resources.texture_bgl,
                            shadow_maps.rendering_bind_group_layout(),
                        ],
                        push_constant_ranges: &[],
                    });

                let vs_scene = wgpu::include_spirv!("../shaders/compiled/scene.vert.spv");
                let vs_scene = device.create_shader_module(&vs_scene);
                let fs_scene = wgpu::include_spirv!("../shaders/compiled/scene.frag.spv");
                let fs_scene = device.create_shader_module(&fs_scene);

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("scene pipeline"),
                    layout: Some(&scene_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_scene,
                        entry_point: "main",
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Vertex>() as u64,
                            step_mode: wgpu::InputStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float3, 1 => Float3, 2 => Float2, 3 => Float4],
                        }],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_scene,
                        entry_point: "main",
                        targets: &[FRAMEBUFFER_FORMAT.into()],
                    }),
                    primitive: wgpu::PrimitiveState {
                        cull_mode: wgpu::CullMode::Back,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                        clamp_depth: false,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            ship_pipeline: {
                let ship_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("ship pipeline layout"),
                        bind_group_layouts: &[
                            &resources.main_bgl,
                            &resources.ship_bgl,
                            shadow_maps.rendering_bind_group_layout(),
                        ],
                        push_constant_ranges: &[],
                    });

                let vs_ship = wgpu::include_spirv!("../shaders/compiled/ship.vert.spv");
                let vs_ship = device.create_shader_module(&vs_ship);
                let fs_ship = wgpu::include_spirv!("../shaders/compiled/ship.frag.spv");
                let fs_ship = device.create_shader_module(&fs_ship);

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("ship pipeline"),
                    layout: Some(&ship_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_ship,
                        entry_point: "main",
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Vertex>() as u64,
                            step_mode: wgpu::InputStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float3, 1 => Float3, 2 => Float2, 3 => Float4],
                        }],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_ship,
                        entry_point: "main",
                        targets: &[FRAMEBUFFER_FORMAT.into()],
                    }),
                    primitive: wgpu::PrimitiveState {
                        cull_mode: wgpu::CullMode::Back,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                        clamp_depth: false,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            sun_dir_pipeline: {
                let vs_sun_dir = wgpu::include_spirv!("../shaders/compiled/sun_dir.vert.spv");
                let vs_sun_dir = device.create_shader_module(&vs_sun_dir);

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("sun dir pipeline"),
                    layout: Some(&main_bind_group_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_sun_dir,
                        entry_point: "main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_flat_colour,
                        entry_point: "main",
                        targets: &[display_format.into()],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::LineList,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                        clamp_depth: false,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            line_pipeline: {
                let vs_line = wgpu::include_spirv!("../shaders/compiled/line.vert.spv");
                let vs_line = device.create_shader_module(&vs_line);

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("line pipeline"),
                    layout: Some(&main_bind_group_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_line,
                        entry_point: "main",
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<LineVertex>() as u64,
                            step_mode: wgpu::InputStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float3, 1 => Float4],
                        }],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_flat_colour,
                        entry_point: "main",
                        targets: &[display_format.into()],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::LineList,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                        clamp_depth: false,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            tonemap_pipeline: {
                let tonemap_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("tonemapper pipeline layout"),
                        bind_group_layouts: &[&resources.tonemap_bgl],
                        push_constant_ranges: &[],
                    });

                let vs_fullscreen_tri =
                    wgpu::include_spirv!("../shaders/compiled/fullscreen_tri.vert.spv");
                let vs_fullscreen_tri = device.create_shader_module(&vs_fullscreen_tri);
                let fs_tonemap = wgpu::include_spirv!("../shaders/compiled/tonemap.frag.spv");
                let fs_tonemap = device.create_shader_module(&fs_tonemap);

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("tonemap pipeline"),
                    layout: Some(&tonemap_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_fullscreen_tri,
                        entry_point: "main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_tonemap,
                        entry_point: "main",
                        targets: &[display_format.into()],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            scene_shadows_pipeline: {
                let scene_shadows_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("scene shadows pipeline layout"),
                        bind_group_layouts: &[shadow_maps.light_projection_bind_group_layout()],
                        push_constant_ranges: &[],
                    });

                let vs_scene_shadows =
                    wgpu::include_spirv!("../shaders/compiled/scene_shadows.vert.spv");
                let vs_scene_shadows = device.create_shader_module(&vs_scene_shadows);

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("scene shadows pipeline"),
                    layout: Some(&scene_shadows_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_scene_shadows,
                        entry_point: "main",
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Vertex>() as u64,
                            step_mode: wgpu::InputStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float3, 1 => Float3, 2 => Float2, 3 => Float4],
                        }],
                    },
                    fragment: None,
                    primitive: wgpu::PrimitiveState {
                        cull_mode: wgpu::CullMode::Back,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                        clamp_depth: false,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            ship_shadows_pipeline: {
                let ship_shadows_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("ship shadows pipeline layout"),
                        bind_group_layouts: &[
                            shadow_maps.light_projection_bind_group_layout(),
                            &resources.ship_bgl,
                        ],
                        push_constant_ranges: &[],
                    });

                let vs_ship_shadows =
                    wgpu::include_spirv!("../shaders/compiled/ship_shadows.vert.spv");
                let vs_ship_shadows = device.create_shader_module(&vs_ship_shadows);

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("ship shadows pipeline"),
                    layout: Some(&ship_shadows_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_ship_shadows,
                        entry_point: "main",
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Vertex>() as u64,
                            step_mode: wgpu::InputStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float3, 1 => Float3, 2 => Float2, 3 => Float4],
                        }],
                    },
                    fragment: None,
                    primitive: wgpu::PrimitiveState {
                        cull_mode: wgpu::CullMode::Back,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                        clamp_depth: false,
                    }),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            ship_movement_pipeline: {
                let ship_movement_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("ship movement pipeline layout"),
                        bind_group_layouts: &[&resources.ship_bgl, &resources.ship_movement_bgl],
                        push_constant_ranges: &[],
                    });

                let cs_ship_movement =
                    wgpu::include_spirv!("../shaders/compiled/ship_movement.comp.spv");
                let cs_ship_movement = device.create_shader_module(&cs_ship_movement);

                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("ship movement pipeline"),
                    layout: Some(&ship_movement_pipeline_layout),
                    module: &cs_ship_movement,
                    entry_point: "main",
                })
            },
        }
    }
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
    cascade_split_lambda: &mut f32,
    ship_movement_settings: &mut primitives::ShipMovementSettings,
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

    dirty.csm |= imgui::Drag::new(imgui::im_str!("Cascade Split Lambda"))
        .range(0.0..=1.0)
        .speed(0.005)
        .build(&ui, cascade_split_lambda);

    dirty.ship_movement_settings |= imgui::Drag::new(imgui::im_str!("Ship Movement Bounds"))
        .range(0.0..=2.5)
        .speed(0.01)
        .build(&ui, &mut ship_movement_settings.bounds);

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
    ship_movement_settings: bool,
}

const fn dispatch_count(num: u32, group_size: u32) -> u32 {
    let mut count = num / group_size;
    let rem = num % group_size;
    if rem != 0 {
        count += 1;
    }

    count
}
