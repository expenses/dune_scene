#version 450

layout(location = 0) in vec3 in_colour;
layout(location = 1) in vec2 in_uv;

layout(location = 0) out vec4 out_colour;

void main() {
    float distance = distance(in_uv * 2.0, vec2(1.0));
    float alpha = max(1.0 - pow(distance, 3), 0.0);

    out_colour = vec4(in_colour, alpha);
}
