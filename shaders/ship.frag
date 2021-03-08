#version 450

#include "brdf.glsl"
#include "utils.glsl"
#include "structs.glsl"

layout(location = 0) in vec3 in_normal;
layout(location = 1) in vec3 in_colour;
layout(location = 2) in vec3 in_camera_dir;
layout(location = 3) in vec3 in_pos;
layout(location = 4) in vec3 in_view_pos;

layout(set = 0, binding = 1) uniform SunUniform {
    Sun sun;
};

layout(set = 0, binding = 2) uniform sampler u_sampler;

layout(set = 0, binding = 3) uniform SettingsUniform {
    Settings settings;
};

layout(set = 2, binding = 0) uniform texture2DArray shadow_texture_array;

layout(set = 2, binding = 1) uniform CascadedShadowMapUniform {
    CSM csm;
};

#define SHADOW_MAP sampler2DArray(shadow_texture_array, u_sampler)
#include "shadows.glsl"

layout(location = 0) out vec4 out_colour;

void main() {
    vec3 normal = normalize(in_normal);

    vec3 camera_dir = normalize(in_camera_dir);
    vec3 halfway_dir = normalize(sun.facing + camera_dir);

    vec3 f0 = vec3(0.04);
    vec3 f90 = compute_f90(f0);

    float NdotL = clamped_dot(normal, sun.facing);
    float VdotH = clamped_dot(camera_dir, halfway_dir);
    float NdotV = clamped_dot(normal, camera_dir);
    float NdotH = clamped_dot(normal, halfway_dir);

    vec3 lighting_factor = sun.light_output * NdotL;

    vec3 diffuse = lighting_factor *
        BRDF_lambertian(f0, f90, in_colour, VdotH);

    float shadow = calculate_shadow(in_view_pos.z, csm.matrices, csm.split_depths, in_pos);

    float diffuse_shadow_amount = 0.1;
    float diffuse_shadowing = shadow * (1.0 - diffuse_shadow_amount) + diffuse_shadow_amount;

    vec3 colour = settings.ambient_lighting * in_colour + (diffuse_shadowing * diffuse);

    if (settings.mode == MODE_SHADOW_CASCADE) {
        uint cascade_index = cascade_index(in_view_pos.z, csm.split_depths);
        colour = debug_colour_by_cascade(colour, cascade_index);
    }

    out_colour = vec4(colour, 1.0);
}
