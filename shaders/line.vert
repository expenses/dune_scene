#version 450

#include "structs.glsl"

layout(location = 0) in vec3 position;
layout(location = 1) in vec4 colour;

layout(set = 0, binding = 0) uniform CameraUniform {
    Camera camera;
};

layout(location = 0) out vec4 out_colour;

void main() {
    out_colour = colour;
    gl_Position = camera.perspective_view * vec4(position, 1.0);
}
