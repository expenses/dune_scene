#version 450

layout(location = 0) in vec4 in_colour;
layout(location = 1) in vec2 in_coord;

layout(location = 0) out vec4 out_colour;

void main() {
    float distance_to_center_sq = dot(in_coord, in_coord);
    float alpha = max(1.0 - distance_to_center_sq, 0.0);

    out_colour = vec4(in_colour.rgb, in_colour.a * alpha);
}
