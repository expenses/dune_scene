#version 450

#include "includes/structs.glsl"

layout(set = 0, binding = 0) uniform CameraUniform {
    Camera camera;
};

layout(set = 0, binding = 4) uniform TimeBuffer {
    Time time;
};

layout(set = 1, binding = 0) readonly buffer ParticlesBuffer {
    Particle particles[];
};

layout(location = 0) out vec4 out_colour;

const vec3 VERTICES[6] = {
    vec3(-1.0, -1.0, 0.0),
    vec3( 1.0, -1.0, 0.0),
    vec3(-1.0,  1.0, 0.0),

    vec3( 1.0, -1.0, 0.0),
    vec3(-1.0,  1.0, 0.0),
    vec3( 1.0,  1.0, 0.0)
};

void main() {
    Particle particle = particles[gl_VertexIndex / 6];

    // Using `fract` here is just because if you disable the ship movement
    // shader, `time_alive_percentage` will become > 1.0 which is just black.
    // This is more fun.
    float red = fract(1.0 - particle.time_alive_percentage);
    out_colour = vec4(red, 0.0, 0.0, 1.0);

    float half_size = 0.005 * particle.time_alive_percentage;

    vec3 view_space = particle.view_space_position + half_size * VERTICES[gl_VertexIndex % 6];

    gl_Position = camera.perspective * vec4(view_space, 1.0);
}
