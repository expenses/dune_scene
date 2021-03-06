#version 450

#include "brdf.glsl"
#include "utils.glsl"

layout(location = 0) in vec3 in_normal;
layout(location = 1) in vec3 in_colour;
layout(location = 2) in vec3 in_camera_dir;

layout(set = 0, binding = 1) uniform Sun {
    vec3 facing;
    vec3 light_output;
} sun;

layout(set = 0, binding = 3) uniform Settings {
    vec3 base_colour;
    float detail_map_scale;
    vec3 ambient_lighting;
    float roughness;
    float specular_factor;
    uint mode;
} settings;

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

    vec3 colour = settings.ambient_lighting * in_colour + diffuse;

    out_colour = vec4(colour, 1.0);
}
