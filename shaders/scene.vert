#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec4 tangent;

layout(location = 0) out vec3 out_normal;
layout(location = 1) out vec2 out_uv;
layout(location = 2) out vec4 out_tangent;
layout(location = 3) out vec3 out_camera_dir;

layout(set = 0, binding = 0) uniform Camera {
    mat4 perspective_view;
    vec3 camera_position;
};

void main() {
    out_normal = normal;
    out_uv = uv;
    out_tangent = tangent;
    out_camera_dir = camera_position - position;

    gl_Position = perspective_view * vec4(position, 1.0);
}
