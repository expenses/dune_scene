#version 450

#include "includes/structs.glsl"

layout(set = 0, binding = 0) uniform CameraUniform {
    Camera camera;
};

layout(set = 1, binding = 0) readonly buffer ParticlesBuffer {
    Particle particles[];
};

layout(location = 0) out vec4 out_colour;

void main() {
    out_colour = vec4(0.0, 1.0, 1.0, 1.0);

    vec3 position = particles[gl_VertexIndex].position;

    gl_Position = camera.perspective_view * vec4(position, 1.0);
    gl_PointSize = 2.0;
}
