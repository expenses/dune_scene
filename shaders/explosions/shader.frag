#version 450

layout(location = 0) in vec2 in_uv;

layout(location = 0) out vec4 out_colour;

layout(set = 0, binding = 2) uniform sampler u_sampler;

layout(set = 1, binding = 0) uniform texture2D u_texture;

void main() {
    out_colour = texture(sampler2D(u_texture, u_sampler), in_uv);
}
