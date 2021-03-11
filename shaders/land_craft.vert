#version 450

#include "includes/structs.glsl"

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec4 tangent;

layout(location = 0) out vec4 out_colour;

layout(set = 0, binding = 0) uniform CameraUniform {
    Camera camera;
};

layout(set = 1, binding = 0) readonly buffer LandCraftBuffer {
    LandCraft craft[];
};

void main() {
    LandCraft craft = craft[gl_InstanceIndex];

    vec3 transformed_pos = position + craft.position;

    out_colour = vec4(1.0, 0.0, 1.0, 1.0);

    gl_Position = camera.perspective_view * vec4(transformed_pos, 1.0);
}
