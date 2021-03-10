#version 450

#include "includes/structs.glsl"

layout(set = 0, binding = 0) uniform CameraUniform {
    Camera camera;
};

layout(set = 0, binding = 1) uniform SunUniform {
    Sun sun;
};

layout(location = 0) out vec4 colour;

void main() {
    colour = vec4(1.0, 1.0, 0.0, 1.0);

    int scale = 10;
    // We want the two line ends to have a multiplier of [-scale, scale].
    int multiplier = gl_VertexIndex * (2 * scale) - (1 * scale);
    vec3 position = sun.facing * multiplier;

    gl_Position = camera.perspective_view * vec4(position, 1.0);
}
