use primitives::{Sun, Vec3A, Vertex};
use std::collections::HashMap;
use ultraviolet::{Mat4, Vec3};
use wgpu::util::DeviceExt;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    pollster::block_on(run())
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const FRAMEBUFFER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

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

    let mut settings = primitives::Settings {
        base_colour: Vec3::new(0.8, 0.535, 0.297),
        detail_map_scale: 1.5,
        ambient_lighting: Vec3::broadcast(0.024),
        roughness: 0.207,
        mode: primitives::Mode::default() as u32,
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
        max_luminance: 20.0,
        grey_in: 0.75,
        grey_out: 0.5,
        enable: true,
    };

    let tonemapper_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Tonemapper uniform buffer"),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        contents: bytemuck::bytes_of(&tonemapper_params.convert()),
    });

    let scene_bytes = include_bytes!("../models/dune.glb");
    let scene = load_scene(scene_bytes, &device, &queue, &resources.texture_bgl)?;

    // Now we can create a window.

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop)?;

    let window_size = window.inner_size();
    let width = window_size.width;
    let height = window_size.height;

    let camera = scene.create_camera(width, height);

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

    let pipelines = Pipelines::new(&device, display_format, &resources);

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

    let mut render_sun_dir = false;

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

                    let camera = scene.create_camera(width, height);
                    queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&camera));
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

                    render_pass.set_pipeline(&pipelines.scene_pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.set_bind_group(1, &scene.texture_bind_group, &[]);
                    render_pass.set_vertex_buffer(0, scene.vertices.slice(..));
                    render_pass
                        .set_index_buffer(scene.indices.slice(..), wgpu::IndexFormat::Uint32);
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

                    if render_sun_dir {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("sun dir render pass"),
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
                        let mut settings_dirty = false;
                        let mut tonemapper_dirty = false;

                        let mut base_colour: [f32; 3] = settings.base_colour.into();

                        if imgui::ColorPicker::new(imgui::im_str!("Colour"), &mut base_colour)
                            .build(&ui)
                        {
                            settings.base_colour = base_colour.into();
                            settings_dirty = true;
                        }

                        let mut ambient_lighting: [f32; 3] = settings.ambient_lighting.into();

                        if imgui::ColorPicker::new(
                            imgui::im_str!("Ambient Lighting"),
                            &mut ambient_lighting,
                        )
                        .build(&ui)
                        {
                            settings.ambient_lighting = ambient_lighting.into();
                            settings_dirty = true;
                        }

                        settings_dirty |= imgui::Drag::new(imgui::im_str!("Detail Scale"))
                            .range(0.0..=10.0)
                            .build(&ui, &mut settings.detail_map_scale);

                        settings_dirty |= imgui::Drag::new(imgui::im_str!("Roughness"))
                            .range(0.0..=1.0)
                            .speed(0.005)
                            .build(&ui, &mut settings.roughness);

                        settings_dirty |= imgui::Drag::new(imgui::im_str!("Specular Factor"))
                            .range(0.0..=2.0)
                            .speed(0.005)
                            .build(&ui, &mut settings.specular_factor);

                        for (mode, index) in primitives::Mode::iter() {
                            settings_dirty |= ui.radio_button(
                                &imgui::im_str!("{:?}", mode),
                                &mut settings.mode,
                                index,
                            );
                        }

                        ui.checkbox(imgui::im_str!("Render Sun Direction"), &mut render_sun_dir);

                        tonemapper_dirty |= ui.checkbox(
                            imgui::im_str!("Enable Tonemapper"),
                            &mut tonemapper_params.enable,
                        );

                        if settings_dirty {
                            queue.write_buffer(&settings_buffer, 0, bytemuck::bytes_of(&settings));
                        }

                        if tonemapper_dirty {
                            queue.write_buffer(
                                &tonemapper_uniform_buffer,
                                0,
                                bytemuck::bytes_of(&tonemapper_params.convert()),
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

fn load_scene(
    bytes: &[u8],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture_bgl: &wgpu::BindGroupLayout,
) -> anyhow::Result<Scene> {
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
                resource: wgpu::BindingResource::TextureView(&image_map.get("normals").unwrap()),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&image_map.get("details").unwrap()),
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

    Ok(Scene {
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

struct Scene {
    camera_y_fov: f32,
    camera_z_near: f32,
    camera_view: Mat4,
    camera_eye: Vec3,
    texture_bind_group: wgpu::BindGroup,
    sun_buffer: wgpu::Buffer,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    num_indices: u32,
}

impl Scene {
    fn create_camera(&self, width: u32, height: u32) -> primitives::Camera {
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

/// All the permement resources that we can load before creating a window.
struct RenderResources {
    main_bgl: wgpu::BindGroupLayout,
    texture_bgl: wgpu::BindGroupLayout,
    tonemap_bgl: wgpu::BindGroupLayout,
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
}

impl Pipelines {
    fn new(
        device: &wgpu::Device,
        display_format: wgpu::TextureFormat,
        resources: &RenderResources,
    ) -> Self {
        Self {
            scene_pipeline: {
                let scene_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("scene pipeline layout"),
                        bind_group_layouts: &[&resources.main_bgl, &resources.texture_bgl],
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
            sun_dir_pipeline: {
                let sun_dir_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("sun dir pipeline layout"),
                        bind_group_layouts: &[&resources.main_bgl],
                        push_constant_ranges: &[],
                    });

                let vs_sun_dir = wgpu::include_spirv!("../shaders/compiled/sun_dir.vert.spv");
                let vs_sun_dir = device.create_shader_module(&vs_sun_dir);
                let fs_flat_colour =
                    wgpu::include_spirv!("../shaders/compiled/flat_colour.frag.spv");
                let fs_flat_colour = device.create_shader_module(&fs_flat_colour);

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("sun dir pipeline"),
                    layout: Some(&sun_dir_pipeline_layout),
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
    enable: bool,
}

impl TonemapperParams {
    fn convert(self) -> primitives::TonemapperSettings {
        let TonemapperParams {
            toe,
            shoulder,
            max_luminance,
            grey_in,
            grey_out,
            enable,
        } = self;

        let a = toe;
        let d = shoulder;

        let denominator = (max_luminance.powf(a * d) - grey_in.powf(a * d)) * grey_out;

        let b = (-grey_in.powf(a) + max_luminance.powf(a) * grey_out) / denominator;

        let c = (max_luminance.powf(a * d) * grey_in.powf(a)
            - max_luminance.powf(a) * grey_in.powf(a * d) * grey_out)
            / denominator;

        let mode = enable as u32;

        primitives::TonemapperSettings { a, b, c, d, mode }
    }
}
