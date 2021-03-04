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

layout(set = 0, binding = 3) uniform Settings {
    vec3 base_colour;
    float specular_power;
    vec3 ambient_lighting;
    float detail_map_scale;
} settings;

layout(set = 1, binding = 0) uniform texture2D u_normals_texture;
layout(set = 1, binding = 1) uniform texture2D u_details_texture;

// https://blog.selfshadow.com/publications/blending-in-detail/
vec3 blend_rnm(vec4 n1, vec4 n2) {
    vec3 t = n1.xyz * vec3( 2,  2, 2) + vec3(-1, -1,  0);
    vec3 u = n2.xyz * vec3(-2, -2, 2) + vec3( 1,  1, -1);
    vec3 r = t * dot(t, u) - u * t.z;
    return normalize(r);
}

void main() {
    vec3 normal = normalize(in_normal);
    vec3 tangent = normalize(in_tangent.xyz);
    vec3 bitangent = cross(in_normal, in_tangent.xyz) * in_tangent.w;
    mat3 TBN = mat3(tangent, bitangent, normal);

    vec4 map_normal = textureLod(sampler2D(u_normals_texture, u_sampler), in_uv, 0);
    vec2 detail_uv = in_uv * settings.detail_map_scale;
    vec4 detail_normal = textureLod(sampler2D(u_details_texture, u_sampler), detail_uv, 0);

    vec3 local_normal = blend_rnm(map_normal, detail_normal);

    normal = normalize(TBN * local_normal);

    float diffuse = max(0.0, dot(normal, sun.facing));

    vec3 camera_dir = normalize(in_camera_dir);
    vec3 halfway_dir = normalize(sun.facing + camera_dir);

    float specular = pow(max(dot(normal, halfway_dir), 0.0), settings.specular_power);

    vec3 diffuse_colour = settings.base_colour * (diffuse * sun.light_output + settings.ambient_lighting);

    vec3 specular_colour = specular * sun.light_output;

    colour = vec4(diffuse_colour + specular_colour, 1.0);
}
