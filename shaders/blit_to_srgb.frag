#version 450

layout(location = 0) in vec2 uv;

layout(location = 0) out vec4 colour;

layout(set = 0, binding = 2) uniform sampler u_sampler;

layout(set = 1, binding = 0) uniform texture2D ui_texture;

void main() {
    colour = texture(sampler2D(ui_texture, u_sampler), uv);
    colour.rgb = pow(colour.rgb, vec3(1.0/2.2));
}

