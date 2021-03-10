use crate::{DEPTH_FORMAT, FRAMEBUFFER_FORMAT};
use cascaded_shadow_maps::CascadedShadowMaps;
use primitives::Vertex;

/// All the permament resources that we can load before creating a window.
pub struct RenderResources {
    pub main_bgl: wgpu::BindGroupLayout,
    pub single_texture_bgl: wgpu::BindGroupLayout,
    pub double_texture_bgl: wgpu::BindGroupLayout,
    pub tonemap_bgl: wgpu::BindGroupLayout,
    pub ship_bgl: wgpu::BindGroupLayout,
    pub particles_bgl: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
}

impl RenderResources {
    pub fn new(device: &wgpu::Device) -> Self {
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
                    uniform(3, wgpu::ShaderStage::FRAGMENT | wgpu::ShaderStage::COMPUTE),
                ],
            }),
            single_texture_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("single texxture bind group layout"),
                entries: &[texture(0, wgpu::ShaderStage::FRAGMENT)],
            }),
            double_texture_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("double texture bind group layout"),
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
            particles_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("particles bind group layout"),
                entries: &[
                    storage(
                        0,
                        wgpu::ShaderStage::COMPUTE | wgpu::ShaderStage::VERTEX,
                        false,
                    ),
                    storage(1, wgpu::ShaderStage::COMPUTE, false),
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

pub struct Pipelines {
    pub scene_pipeline: wgpu::RenderPipeline,
    pub sun_dir_pipeline: wgpu::RenderPipeline,
    pub tonemap_pipeline: wgpu::RenderPipeline,
    pub ship_pipeline: wgpu::RenderPipeline,
    pub particles_pipeline: wgpu::RenderPipeline,
    pub scene_shadows_pipeline: wgpu::RenderPipeline,
    pub ship_shadows_pipeline: wgpu::RenderPipeline,
    pub ship_movement_pipeline: wgpu::ComputePipeline,
}

impl Pipelines {
    pub fn new(
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
                            &resources.double_texture_bgl,
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
                            &resources.single_texture_bgl,
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
            particles_pipeline: {
                let vs_particles = wgpu::include_spirv!("../shaders/compiled/particles.vert.spv");
                let vs_particles = device.create_shader_module(&vs_particles);

                let particles_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("particles pipeline layout"),
                        bind_group_layouts: &[&resources.main_bgl, &resources.particles_bgl],
                        push_constant_ranges: &[],
                    });

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("particles pipeline"),
                    layout: Some(&particles_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_particles,
                        entry_point: "main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_flat_colour,
                        entry_point: "main",
                        targets: &[FRAMEBUFFER_FORMAT.into()],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::PointList,
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
                        bind_group_layouts: &[
                            &resources.main_bgl,
                            &resources.ship_bgl,
                            &resources.particles_bgl,
                        ],
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
