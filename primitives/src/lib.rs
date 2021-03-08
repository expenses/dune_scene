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
pub struct Transform {
    pub translation: Vec3,
    pub y_rotation: f32,
    pub _rotation_matrix: [Vec4; 3],
    pub rotation_speed: f32,
    pub _end_padding: [u32; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub position: Vec3,
    pub colour: Vec4,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShipMovementSettings {
    pub bounds: f32,
}
