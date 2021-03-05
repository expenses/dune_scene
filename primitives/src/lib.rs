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
pub struct ParticleEmitter {
    pub position: Vec3,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Particle {
    pub position: Vec3,
    pub time_remaining: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Camera {
    pub perspective_view: Mat4,
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
}

impl Default for Mode {
    fn default() -> Self {
        Self::Full
    }
}

impl Mode {
    pub fn iter() -> impl Iterator<Item = (Self, u32)> {
        [Self::Full, Self::Normals]
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, mode)| (mode, i as u32))
    }
}
