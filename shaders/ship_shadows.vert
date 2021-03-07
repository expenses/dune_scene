#version 450

#include "structs.glsl"

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec4 tangent;

layout(set = 0, binding = 0) uniform SunProjectionView {
    mat4 projection_view;
};

layout(set = 1, binding = 0) readonly buffer ShipTransforms {
    Ship ship_transforms[];
};

void main() {
    Ship ship_transform = ship_transforms[gl_InstanceIndex];

    mat3 rotation_matrix = ship_transform.y_rotation_matrix;

    vec3 transformed_pos = rotation_matrix * position + ship_transform.position;

    gl_Position = projection_view * vec4(transformed_pos, 1.0);
}
