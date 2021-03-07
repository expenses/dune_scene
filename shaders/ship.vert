#version 450

#include "structs.glsl"

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec4 tangent;

layout(location = 0) out vec3 out_normal;
layout(location = 1) out vec3 out_colour;
layout(location = 2) out vec3 out_camera_dir;
layout(location = 3) out vec3 out_pos;
layout(location = 4) out vec3 out_view_pos;

layout(set = 0, binding = 0) uniform CameraUniform {
    Camera camera;
};

layout(set = 1, binding = 0) readonly buffer ShipTransforms {
    Ship ship_transforms[];
};

void main() {
    Ship ship_transform = ship_transforms[gl_InstanceIndex];

    mat3 rotation_matrix = ship_transform.y_rotation_matrix;

    vec3 transformed_pos = rotation_matrix * position + ship_transform.position;

    out_normal = rotation_matrix * normal;
    out_colour = out_normal;
    out_camera_dir = camera.position - transformed_pos;
    out_pos = transformed_pos;
    out_view_pos = (camera.view * vec4(transformed_pos, 1.0)).xyz;

    gl_Position = camera.perspective_view * vec4(transformed_pos, 1.0);
}
