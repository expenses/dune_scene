#version 450

#include "brdf.glsl"

layout(location = 0) in vec3 in_normal;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_tangent;
layout(location = 3) in vec3 in_camera_dir;

layout(location = 0) out vec4 out_colour;

layout(set = 0, binding = 1) uniform Sun {
    vec3 facing;
    vec3 light_output;
} sun;

layout(set = 0, binding = 2) uniform sampler u_sampler;

layout(set = 0, binding = 3) uniform Settings {
    vec3 base_colour;
    float detail_map_scale;
    vec3 ambient_lighting;
    float roughness;
    uint mode;
} settings;

layout(set = 1, binding = 0) uniform texture2D u_normals_texture;
layout(set = 1, binding = 1) uniform texture2D u_details_texture;

// todo: use a better blending function than this.
// https://blog.selfshadow.com/publications/blending-in-detail/
vec3 blend_normals(vec3 a, vec3 b) {
    float xs = a.x + b.x;
    float ys = a.y + b.y;
    return normalize(vec3(xs, ys, a.z)) * 2.0 - 1.0;
}

float clamped_dot(vec3 a, vec3 b) {
    return max(dot(a, b), 0.0);
}

vec3 normal_to_view_space(vec3 normal) {
    return normal * 0.5 + 0.5;
}

// See https://github.com/KhronosGroup/glTF-Sample-Viewer/blob/master/source/Renderer/shaders/brdf.glsl
vec3 compute_f90(vec3 f0) {
    // Compute reflectance.
    float reflectance = max(max(f0.r, f0.g), f0.b);

    // Anything less than 2% is physically impossible and is instead considered
    // to be shadowing. Compare to "Real-Time-Rendering" 4th editon on page 325.
    vec3 f90 = vec3(clamp(reflectance * 50.0, 0.0, 1.0));
    return f90;
}

void main() {
    vec3 normal = normalize(in_normal);
    vec3 tangent = normalize(in_tangent.xyz);
    vec3 bitangent = cross(in_normal, in_tangent.xyz) * in_tangent.w;
    mat3 TBN = mat3(tangent, bitangent, normal);

    vec4 map_normal = textureLod(sampler2D(u_normals_texture, u_sampler), in_uv, 0);
    vec2 detail_uv = in_uv * settings.detail_map_scale;
    vec4 detail_normal = textureLod(sampler2D(u_details_texture, u_sampler), detail_uv, 0);

    vec3 local_normal = blend_normals(map_normal.xyz, detail_normal.xyz);

    normal = normalize(TBN * local_normal);

    vec3 camera_dir = normalize(in_camera_dir);
    vec3 halfway_dir = normalize(sun.facing + camera_dir);

    vec3 f0 = vec3(0.04);
    vec3 f90 = compute_f90(f0);

    float alpha_roughness = settings.roughness * settings.roughness;

    float NdotL = clamped_dot(normal, sun.facing);
    float VdotH = clamped_dot(camera_dir, halfway_dir);
    float NdotV = clamped_dot(normal, camera_dir);
    float NdotH = clamped_dot(normal, halfway_dir);

    vec3 lighting_factor = sun.light_output * NdotL;

    vec3 diffuse =  lighting_factor * BRDF_lambertian(f0, f90, settings.base_colour, VdotH);
    vec3 specular = lighting_factor * BRDF_specularGGX(f0, f90, alpha_roughness, VdotH, NdotL, NdotV, NdotH);

    vec3 colour = diffuse + specular;

    // todo: HDR
    //colour = colour / (colour + vec3(1.0));
    //colour = pow(colour, vec3(1.0/2.2));

    switch (settings.mode) {
        case 0:
            break;
        case 1:
            colour = diffuse;
            break;
        case 2:
            colour = specular;
            break;
        case 3:
            // To compare with the normals in blender, we need to shift the
            // normals from Y space to Z space.
            colour = normal_to_view_space(normal.xzy * vec3(1, -1, 1));
            break;
        case 4:
            colour = normal_to_view_space(local_normal);
    }

    out_colour = vec4(colour, 1.0);
}
