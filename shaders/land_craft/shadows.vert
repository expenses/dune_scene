#version 450

#include "../includes/structs.glsl"

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec4 tangent;

layout(set = 0, binding = 0) uniform SunProjectionView {
    mat4 projection_view;
};

layout(set = 1, binding = 0) readonly buffer LandCraftBuffer {
    LandCraft crafts[];
};

void main() {
    LandCraft craft = crafts[gl_InstanceIndex];

    vec3 transformed_pos = craft.position + craft.rotation_matrix * position;

    gl_Position = projection_view * vec4(transformed_pos, 1.0);
}
