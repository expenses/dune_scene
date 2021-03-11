#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in vec4 in_tangent;

layout(location = 0) out float out_height;

void main() {
    out_height = in_position.y;

    vec2 position = in_position.xz * vec2(0.5, -0.5);

    gl_Position = vec4(position, 0.0, 1.0);
}
