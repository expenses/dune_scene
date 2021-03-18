#version 450

#include "../includes/structs.glsl"
#include "../includes/matrices.glsl"
#include "../includes/rotor.glsl"
#include "../includes/similarity.glsl"

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in vec4 in_tangent;
layout(location = 4) in uvec4 joint_indices;
layout(location = 5) in vec4 joint_weights;

layout(location = 6) in vec3 instance_position;

layout(location = 0) out vec2 out_uv;

layout(set = 0, binding = 0) uniform CameraUniform {
    Camera camera;
};

layout(set = 2, binding = 0) readonly buffer JointTransforms {
    Similarity joint_transforms[];
};

layout(set = 2, binding = 1) uniform AnimatedModelInfoUniform {
    AnimatedModelInfo animated_model_info;
};

void main() {
    out_uv = in_uv;

    uint joint_offset = gl_InstanceIndex * animated_model_info.num_joints;

    // Calculate skinning sim from weights and joint indices of the current vertex
    Similarity x = similarity_mul_scalar(joint_transforms[joint_indices.x + joint_offset], joint_weights.x);
    Similarity y = similarity_mul_scalar(joint_transforms[joint_indices.y + joint_offset], joint_weights.y);
    Similarity z = similarity_mul_scalar(joint_transforms[joint_indices.z + joint_offset], joint_weights.z);
    Similarity w = similarity_mul_scalar(joint_transforms[joint_indices.w + joint_offset], joint_weights.w);

    Similarity skin = similarity_add_similarity(
        similarity_add_similarity(x, y), similarity_add_similarity(z, w)
    );

    vec3 skinned_pos = similarity_transform_vec(skin, in_position);

    vec3 position = skinned_pos + instance_position;

    gl_Position = camera.perspective_view * vec4(position, 1.0);
}
