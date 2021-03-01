use primitives::{Sun, Vertex};
use ultraviolet::{Mat4, Vec3};
use wgpu::util::DeviceExt;

fn main() -> anyhow::Result<()> {
    pollster::block_on(run())
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Sampler {
                    filtering: false,
                    comparison: false,
                },
                count: None,
            },
        ],
    });

    let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("texture bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("linear sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let scene_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("scene pipeline layout"),
        bind_group_layouts: &[&bind_group_layout, &texture_bgl],
        push_constant_ranges: &[],
    });

    let vs_scene = wgpu::include_spirv!("../shaders/compiled/scene.vert.spv");
    let vs_scene = device.create_shader_module(&vs_scene);
    let fs_scene = wgpu::include_spirv!("../shaders/compiled/scene.frag.spv");
    let fs_scene = device.create_shader_module(&fs_scene);

    let scene_bytes = include_bytes!("../models/dune.glb");
    let scene = load_scene(scene_bytes, &device, &queue, &texture_bgl)?;

    // Now we can create a window.

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop)?;

    let window_size = window.inner_size();
    let width = window_size.width;
    let height = window_size.height;

    let aspect_ratio = width as f32 / height as f32;
    let perspective_matrix = scene.create_perspective_matrix(aspect_ratio);
    let perspective_view = perspective_matrix * scene.camera_view;

    let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("camera buffer"),
        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        contents: bytemuck::bytes_of(&perspective_view),
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bind group"),
        layout: &bind_group_layout,
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
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    let surface = unsafe { instance.create_surface(&window) };

    let display_format = adapter.get_swap_chain_preferred_format(&surface);

    let scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            targets: &[display_format.into()],
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
    });

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

    use winit::event::*;
    use winit::event_loop::*;

    event_loop.run(move |event, _, control_flow| match event {
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

                let aspect_ratio = width as f32 / height as f32;
                let perspective_matrix = scene.create_perspective_matrix(aspect_ratio);
                let perspective_view = perspective_matrix * scene.camera_view;

                queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&perspective_view));
            }
            _ => {}
        },
        Event::MainEventsCleared => window.request_redraw(),
        Event::RedrawRequested(_) => match swap_chain.get_current_frame() {
            Ok(frame) => {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render encoder"),
                });

                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("main render pass"),
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: &frame.output.view,
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

                render_pass.set_pipeline(&scene_pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.set_bind_group(1, &scene.texture_bind_group, &[]);
                render_pass.set_vertex_buffer(0, scene.vertices.slice(..));
                render_pass.set_index_buffer(scene.indices.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..scene.num_indices, 0, 0..1);

                drop(render_pass);

                queue.submit(Some(encoder.finish()));
            }
            Err(error) => println!("Swap chain error: {:?}", error),
        },
        _ => {}
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

    let mut images = gltf.images();

    let normal_map_image = images.next().unwrap();
    let normal_map_texture = load_image(&normal_map_image, buffer_blob, device, queue)?;

    let diffuse_image = images.next().unwrap();
    let diffuse_texture = load_image(&diffuse_image, buffer_blob, device, queue)?;

    let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("texture bind group"),
        layout: texture_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&diffuse_texture),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&normal_map_texture),
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
        facing: sun_rotor * Vec3::unit_z(),
        padding: 0,
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
    texture_bind_group: wgpu::BindGroup,
    sun_buffer: wgpu::Buffer,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    num_indices: u32,
}

impl Scene {
    fn create_perspective_matrix(&self, aspect_ratio: f32) -> Mat4 {
        ultraviolet::projection::perspective_infinite_z_wgpu_dx(
            self.camera_y_fov,
            aspect_ratio,
            self.camera_z_near,
        )
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
