#version 450

#include "../includes/brdf.glsl"
#include "../includes/utils.glsl"
#include "../includes/structs.glsl"

layout(location = 0) in vec3 in_normal;
layout(location = 1) in vec2 in_uv;
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

layout(set = 0, binding = 4) uniform TimeBuffer {
    Time time;
};

layout(set = 2, binding = 0) uniform texture2D u_texture;

layout(set = 3, binding = 0) uniform texture2DArray shadow_texture_array;

layout(set = 3, binding = 1) uniform sampler shadow_sampler;

layout(set = 3, binding = 2) uniform CascadedShadowMapUniform {
    CSM csm;
};

#define SHADOW_SAMPLER shadow_sampler
#define SHADOW_TEXTURE_ARRAY shadow_texture_array
#include "../includes/shadows.glsl"

layout(location = 0) out vec4 out_colour;

// All UVs below this are part of the treads and should be rotated.
const float UV_ROTATION_THRESHOLD = 1.0 - 0.186;

void main() {
    vec3 normal = normalize(in_normal);

    vec3 camera_dir = normalize(in_camera_dir);
    vec3 halfway_dir = normalize(sun.facing + camera_dir);

    vec2 uv = in_uv;
    uv.x = fract(uv.x - float(uv.y > UV_ROTATION_THRESHOLD) * time.time_since_start);

    vec3 texture_colour = texture(sampler2D(u_texture, u_sampler), uv).rgb;

    vec3 f0 = vec3(0.04);
    vec3 f90 = compute_f90(f0);

    float NdotL = clamped_dot(normal, sun.facing);
    float VdotH = clamped_dot(camera_dir, halfway_dir);
    float NdotV = clamped_dot(normal, camera_dir);
    float NdotH = clamped_dot(normal, halfway_dir);

    vec3 lighting_factor = sun.light_output * NdotL;

    vec3 diffuse = lighting_factor *
        BRDF_lambertian(f0, f90, texture_colour, VdotH);

    float shadow = calculate_shadow(in_view_pos.z, csm.matrices, csm.split_depths, in_pos);

    float diffuse_shadow_amount = 0.1;
    float diffuse_shadowing = shadow * (1.0 - diffuse_shadow_amount) + diffuse_shadow_amount;

    vec3 colour = settings.ambient_lighting * texture_colour + (diffuse_shadowing * diffuse);

    if (settings.mode == MODE_SHADOW_CASCADE) {
        uint cascade_index = cascade_index(in_view_pos.z, csm.split_depths);
        colour *= debug_colour_for_cascade(cascade_index);
    }

    out_colour = vec4(colour, 1.0);
}
