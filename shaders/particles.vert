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
layout(location = 1) out vec2 out_coord;

const vec2 COORDS[6] = {
    vec2(-1.0, -1.0),
    vec2( 1.0, -1.0),
    vec2(-1.0,  1.0),

    vec2( 1.0, -1.0),
    vec2(-1.0,  1.0),
    vec2( 1.0,  1.0)
};

void main() {
    Particle particle = particles[gl_VertexIndex / 6];

    // Using `fract` here is just because if you disable the ship movement
    // shader, `time_alive_percentage` will become > 1.0 which is just black.
    // This is more fun.
    float red = fract(1.0 - particle.time_alive_percentage);
    float alpha = fract(1.0 - particle.time_alive_percentage * 0.5);
    out_colour = vec4(red, 0.0, 0.0, alpha);

    float half_size = 0.01 * min(particle.time_alive_percentage, 1.0);

    vec2 coord = COORDS[gl_VertexIndex % 6];

    out_coord = coord;

    vec3 view_space = particle.view_space_position + half_size * vec3(coord, 0.0);

    gl_Position = camera.perspective * vec4(view_space, 1.0);
}
