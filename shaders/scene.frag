#version 450

layout(location = 0) in vec3 in_normal;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_tangent;
layout(location = 3) in vec3 in_camera_dir;

layout(location = 0) out vec4 colour;

layout(set = 0, binding = 1) uniform Sun {
    vec3 facing;
    vec3 light_output;
} sun;

layout(set = 0, binding = 2) uniform sampler u_sampler;

layout(set = 1, binding = 0) uniform texture2D u_diffuse_texture;
layout(set = 1, binding = 1) uniform texture2D u_normal_map_texture;

void main() {
    vec3 normal = normalize(in_normal);
    vec3 tangent = normalize(in_tangent.xyz);
    vec3 bitangent = cross(in_normal, in_tangent.xyz) * in_tangent.w;
    mat3 TBN = mat3(tangent, bitangent, normal);
    vec3 local_normal = texture(sampler2D(u_normal_map_texture, u_sampler), in_uv).xyz * 2.0 - 1.0;

    float strength = 0.6;
    local_normal = normalize(mix(vec3(0.0, 0.0, 1.0), local_normal, strength));

    normal = normalize(TBN * local_normal);

    vec3 ambient = vec3(0.051);

    float diffuse = max(0.0, dot(normal, sun.facing));

    vec3 camera_dir = normalize(in_camera_dir);
    vec3 halfway_dir = normalize(sun.facing + camera_dir);
    float specular = pow(max(dot(normal, halfway_dir), 0.0), 64.0);

    vec3 coloured_lighting = diffuse * sun.light_output + ambient + specular;

    vec4 texture_colour = texture(sampler2D(u_diffuse_texture, u_sampler), in_uv);

    colour = vec4(coloured_lighting, 1.0) * texture_colour;
}
