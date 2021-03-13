use ultraviolet::{Mat4, Vec2, Vec3, Vec4};

/// A 16-byte aligned `Vec3`.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vec3A {
    inner: Vec3,
    padding: u32,
}

impl Vec3A {
    pub fn new(vec: Vec3) -> Self {
        Self {
            inner: vec,
            padding: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Sun {
    pub facing: Vec3A,
    pub output: Vec3,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub tangent: Vec4,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Camera {
    pub perspective_view: Mat4,
    pub view: Mat4,
    pub perspective: Mat4,
    pub position: Vec3,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Settings {
    pub base_colour: Vec3,
    pub detail_map_scale: f32,
    pub ambient_lighting: Vec3,
    pub roughness: f32,
    pub specular_factor: f32,
    pub mode: u32,
    pub ship_movement_bounds: f32,
}

#[derive(Debug, Copy, Clone)]
pub enum Mode {
    Full,
    Normals,
    Noise,
    HueNoise,
    ShadowCascade,
}

impl Mode {
    pub fn iter() -> impl Iterator<Item = (Self, u32)> {
        enumerate(&[
            Self::Full,
            Self::Normals,
            Self::Noise,
            Self::HueNoise,
            Self::ShadowCascade,
        ])
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TonemapperSettings {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub mode: u32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TonemapperMode {
    On,
    NoCrosstalk,
    Off,
    WasmGammaCorrect,
}

impl TonemapperMode {
    pub fn iter() -> impl Iterator<Item = Self> {
        [
            Self::On,
            Self::NoCrosstalk,
            Self::Off,
            Self::WasmGammaCorrect,
        ]
        .iter()
        .cloned()
    }
}

fn enumerate<T: Copy>(slice: &'static [T]) -> impl Iterator<Item = (T, u32)> {
    slice
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, t)| (t, i as u32))
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Ship {
    pub position: Vec3,
    pub y_rotation: f32,
    pub _rotation_matrix: [Vec4; 3],
    pub rotation_speed: f32,
    pub _end_padding: [u32; 3],
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticlesBufferInfo {
    pub colour: Vec3,
    pub _offset: u32,
    pub _particle_duration: f32,
    pub half_size_linear: f32,
    pub _last_particle_spawn_time: f32,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Particle {
    pub position: Vec3,
    pub time_spawned: f32,
    pub velocity: Vec3,
    pub _padding_0: u32,
    pub _view_space_position: Vec3,
    pub _padding_1: u32,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Time {
    pub time_since_start: f32,
    pub delta_time: f32,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LandCraft {
    pub position: Vec3,
    pub facing: f32,
    pub _rotation_matrix: [Vec4; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Similarity {
    pub scale: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NodeAndParent {
    pub node_index: u32,
    pub parent_index: i32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AnimatedVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub tangent: Vec4,
    pub joint_indices: [u16; 4],
    pub joint_weights: Vec4,
}
