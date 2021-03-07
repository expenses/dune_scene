#version 450

#include "brdf.glsl"
#include "utils.glsl"
#include "structs.glsl"

layout(location = 0) in vec3 in_normal;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_tangent;
layout(location = 3) in vec3 in_camera_dir;
layout(location = 4) in vec3 in_pos;
layout(location = 5) in vec3 in_view_pos;

layout(location = 0) out vec4 out_colour;

layout(set = 0, binding = 1) uniform SunUniform {
    Sun sun;
};

layout(set = 0, binding = 2) uniform sampler u_sampler;

layout(set = 0, binding = 3) uniform SettingsUniform {
    Settings settings;
};

layout(set = 1, binding = 0) uniform texture2D u_normals_texture;
layout(set = 1, binding = 1) uniform texture2D u_details_texture;

layout(set = 2, binding = 0) uniform texture2DArray shadow_texture_array;

layout(set = 2, binding = 1) uniform CascadedShadowMapUniform {
    CSM csm;
};

#define SHADOW_MAP sampler2DArray(shadow_texture_array, u_sampler)
#include "shadows.glsl"

// todo: use a better blending function than this.
// https://blog.selfshadow.com/publications/blending-in-detail/
vec3 blend_normals(vec3 a, vec3 b) {
    float xs = a.x + b.x;
    float ys = a.y + b.y;
    return normalize(vec3(xs, ys, a.z)) * 2.0 - 1.0;
}

vec3 normal_to_view_space(vec3 normal) {
    return normal * 0.5 + 0.5;
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

    vec3 diffuse =  lighting_factor *
        BRDF_lambertian(f0, f90, settings.base_colour, VdotH);
    vec3 specular = lighting_factor *
        BRDF_specularGGX(f0, f90, alpha_roughness, VdotH, NdotL, NdotV, NdotH);

    float noise = random(in_uv);
    vec3 hue_noise = hsv2rgb_smooth(vec3(noise, 1.0, 1.0));

    specular *= settings.specular_factor * hue_noise;

    float shadow = calculate_shadow(in_view_pos.z, csm.matrices, csm.split_depths, in_pos);

    float diffuse_shadow_amount = 0.1;
    float diffuse_shadowing = shadow * (1.0 - diffuse_shadow_amount) + diffuse_shadow_amount;

    vec3 colour = settings.ambient_lighting + (diffuse * diffuse_shadowing) + (specular * shadow);

    switch (settings.mode) {
        case MODE_FULL:
            break;
        case MODE_NORMALS:
            // To compare with the normals in blender, we need to shift the
            // normals from Y space to Z space.
            colour = normal_to_view_space(normal.xzy * vec3(1, -1, 1));
            break;
        case MODE_NOISE:
            colour = vec3(noise);
            break;
        case MODE_HUE_NOISE:
            colour = hue_noise;
            break;
        case MODE_SHADOW_CASCADE:
            uint cascade_index = cascade_index(in_view_pos.z, csm.split_depths);
            colour = debug_colour_by_cascade(colour, cascade_index);
            break;
    }

    out_colour = vec4(colour, 1.0);
}
