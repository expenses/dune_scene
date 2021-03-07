
struct Settings {
    vec3 base_colour;
    float detail_map_scale;
    vec3 ambient_lighting;
    float roughness;
    float specular_factor;
    uint mode;
};

const uint MODE_FULL = 0;
const uint MODE_NORMALS = 1;
const uint MODE_NOISE = 2;
const uint MODE_HUE_NOISE = 3;
const uint MODE_SHADOW_CASCADE = 4;

struct CSM {
    mat4 matrices[3];
    vec3 split_depths;
};

struct Camera {
    mat4 perspective_view;
    mat4 view;
    vec3 position;
};

struct Ship {
    vec3 position;
    float facing;
    mat3 y_rotation_matrix;
};

struct Sun {
    vec3 facing;
    vec3 light_output;
};
