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

// https://blog.selfshadow.com/publications/blending-in-detail/
vec3 blend_rnm(float4 n1, float4 n2) {
    vec3 t = n1.xyz * vec3( 2,  2, 2) + float3(-1, -1,  0);
    vec3 u = n2.xyz * vec3(-2, -2, 2) + float3( 1,  1, -1);
    vec3 r = t * dot(t, u) - u * t.z;
    return normalize(r);
}

void main() {
    vec3 normal = normalize(in_normal);
    vec3 tangent = normalize(in_tangent.xyz);
    vec3 bitangent = cross(in_normal, in_tangent.xyz) * in_tangent.w;
    mat3 TBN = mat3(tangent, bitangent, normal);
    vec3 local_normal = texture(sampler2D(u_normal_map_texture, u_sampler), in_uv).xyz * 2.0 - 1.0;

    //colour = vec4(local_normal, 1.0);

    normal = normalize(TBN * local_normal);

    vec3 ambient = vec3(0.051);

    float diffuse = max(0.0, dot(normal, sun.facing));

    vec3 camera_dir = normalize(in_camera_dir);
    vec3 halfway_dir = normalize(sun.facing + camera_dir);
    float specular = pow(max(dot(normal, halfway_dir), 0.0), 128.0);

    vec3 texture_colour = texture(sampler2D(u_diffuse_texture, u_sampler), in_uv).rgb;

    vec3 diffuse_colour = texture_colour * (diffuse * sun.light_output + ambient);

    vec3 specular_colour = specular * sun.light_output;

    colour = vec4(diffuse_colour + specular_colour, 1.0);
}
