#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) in vec4 tangent;

layout(location = 0) out vec3 out_normal;
//layout(location = 1) out vec2 out_uv;
//layout(location = 2) out vec4 out_tangent;
layout(location = 1) out vec3 out_colour;
layout(location = 2) out vec3 out_camera_dir;


layout(set = 0, binding = 0) uniform Camera {
    mat4 perspective_view;
    vec3 camera_position;
};

struct Transform {
    vec3 translation;
    float y_rotation;
    mat3 y_rotation_matrix;
};

layout(set = 1, binding = 0) readonly buffer Transforms {
    Transform transforms[];
};

void main() {
    Transform transform = transforms[gl_InstanceIndex];

    mat3 rotation_matrix = transform.y_rotation_matrix;

    vec3 transformed_pos = rotation_matrix * position + transform.translation;

    out_normal = rotation_matrix * normal;
    out_colour = out_normal;
    //out_uv = uv;
    //out_tangent = vec4(normal_transform * tangent.xyz, tangent.w);
    out_camera_dir = camera_position - transformed_pos;

    gl_Position = perspective_view * vec4(transformed_pos, 1.0);
}
