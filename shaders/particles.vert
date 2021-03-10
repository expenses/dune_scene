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

layout(set = 1, binding = 1) readonly buffer ParticlesInfo {
    ParticlesBufferInfo particles_info;
};

layout(location = 0) out vec4 out_colour;

void main() {
    Particle particle = particles[gl_VertexIndex];

    float time_alive = time.time_since_start - particle.time_spawned;
    vec3 position = particle.initial_position + time_alive * particle.velocity;

    float time_alive_percentage = time_alive / particles_info.particle_duration;
    // Using `fract` here is just because if you disable the ship movement
    // shader, `time_alive_percentage` will become > 1.0 which is just black.
    // This is more fun.
    float red = fract(1.0 - time_alive_percentage);
    out_colour = vec4(red, 0.0, 0.0, 1.0);

    gl_Position = camera.perspective_view * vec4(position, 1.0);
    gl_PointSize = 2.0;
}
