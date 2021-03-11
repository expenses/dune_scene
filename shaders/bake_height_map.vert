#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in vec4 in_tangent;

layout(location = 0) out float out_height;

void main() {
    out_height = in_position.y;

    gl_Position = vec4(in_position.xz / 2.0, 0.0, 1.0);
}
