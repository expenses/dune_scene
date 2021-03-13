#version 450

#include "../includes/structs.glsl"

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
    mat4 joint_transforms[];
};

void main() {
    out_uv = in_uv;

    uint num_joints = 1;
    uint joint_offset = gl_InstanceIndex * num_joints;

    // Calculate skinned matrix from weights and joint indices of the current vertex
	mat4 skin =
		joint_weights.x * joint_transforms[joint_indices.x + joint_offset] +
		joint_weights.y * joint_transforms[joint_indices.y + joint_offset] +
		joint_weights.z * joint_transforms[joint_indices.z + joint_offset] +
		joint_weights.w * joint_transforms[joint_indices.w + joint_offset];

    vec3 skinned_pos = (skin * vec4(in_position, 1.0)).xyz;

    vec3 position = skinned_pos + instance_position;

    gl_Position = camera.perspective_view * vec4(position, 1.0);
}
