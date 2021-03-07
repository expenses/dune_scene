#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec4 tangent;

layout(set = 0, binding = 0) uniform SunProjectionView {
    mat4 projection_view;
};

void main() {
    gl_Position = projection_view * vec4(position, 1.0);
}
