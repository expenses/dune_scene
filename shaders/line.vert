#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec4 colour;

layout(set = 0, binding = 0) uniform Camera {
    mat4 perspective_view;
    vec3 camera_position;
};

layout(location = 0) out vec4 out_colour;

void main() {
    out_colour = colour;
    gl_Position = perspective_view * vec4(position, 1.0);
}
