#version 450

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 out_colour;

layout(set = 0, binding = 0) uniform texture2D u_texture;
layout(set = 0, binding = 1) uniform sampler u_sampler;
layout(set = 0, binding = 2) uniform TonemapperUniform {
    float A;
    float B;
    float C;
    float D;
    uint mode;
};

float tonemap(float x) {
    float z = pow(x, A);

    return z / (pow(z, D) * B + C);
}

vec3 lerp(vec3 a, vec3 b, float factor) {
    return (1.0 - factor) * a + factor * b;
}

void main() {
    vec4 texel = texture(sampler2D(u_texture, u_sampler), uv);
    vec3 rgb = texel.rgb;

    float peak = max(max(rgb.r, rgb.g), rgb.b);
    vec3 ratio = rgb / peak;
    peak = tonemap(peak);

    vec3 no_crosstalk_ratio = ratio;

    /*
    // Apply channel crosstalk

    float saturation = 1.5;
    float crossSaturation = 2.0;
    float crosstalk = 1.0 / 10.0;

    ratio = pow(ratio, vec3(saturation / crossSaturation));
    ratio = lerp(ratio, vec3(1.0), pow(peak, 1.0 / crosstalk));
    ratio = pow(ratio, vec3(crossSaturation));
    */

    vec3 colour = peak * ratio;

    switch (mode) {
        case 0:
            break;
        case 1:
            colour = peak * no_crosstalk_ratio;
            break;
        case 2:
            colour = rgb;
            break;
    }

    out_colour = vec4(colour, 1.0);
}

