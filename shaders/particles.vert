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

void main() {
    Particle particle = particles[gl_VertexIndex];

    float time_alive = time.time_since_start - particle.time_spawned;
    float duration = 20.0 / 60.0;
    float time_alive_percentage = time_alive / duration;

    out_colour = vec4(1.0 - time_alive_percentage, 0.0, 0.0, 1.0);

    gl_Position = camera.perspective_view * vec4(particle.position, 1.0);
    gl_PointSize = 2.0;
}
