#version 450

#include "../includes/structs.glsl"

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec4 tangent;

layout(location = 0) out vec3 out_normal;
layout(location = 1) out vec2 out_uv;
layout(location = 2) out vec3 out_camera_dir;
layout(location = 3) out vec3 out_pos;
layout(location = 4) out vec3 out_view_pos;

layout(set = 0, binding = 0) uniform CameraUniform {
    Camera camera;
};

layout(set = 1, binding = 0) readonly buffer LandCraftBuffer {
    LandCraft crafts[];
};

void main() {
    LandCraft craft = crafts[gl_InstanceIndex];

    mat3 rotation = craft.rotation_matrix;

    vec3 transformed_pos = craft.position + rotation * position;

    out_normal = rotation * normal;
    out_uv = uv;
    out_camera_dir = camera.position - transformed_pos;
    out_pos = transformed_pos;
    out_view_pos = (camera.view * vec4(transformed_pos, 1.0)).xyz;

    gl_Position = camera.perspective_view * vec4(transformed_pos, 1.0);
}
