
struct Settings {
    vec3 base_colour;
    float detail_map_scale;
    vec3 ambient_lighting;
    float roughness;
    float specular_factor;
    uint mode;
    float ship_movement_bounds;
};

const uint MODE_FULL = 0;
const uint MODE_NORMALS = 1;
const uint MODE_NOISE = 2;
const uint MODE_HUE_NOISE = 3;
const uint MODE_SHADOW_CASCADE = 4;

struct CSM {
    mat4 matrices[3];
    vec2 split_depths;
};

struct Camera {
    mat4 perspective_view;
    mat4 view;
    mat4 perspective;
    vec3 position;
};

struct Ship {
    vec3 position;
    float facing;
    mat3 y_rotation_matrix;
    float rotation_speed;
};

struct Sun {
    vec3 facing;
    vec3 light_output;
};

const uint TONEMAPPER_MODE_ON = 0;
const uint TONEMAPPER_MODE_NO_CROSSTALK = 1;
const uint TONEMAPPER_MODE_OFF = 2;
const uint TONEMAPPER_MODE_WASM_GAMMA_CORRECT = 3;

struct ParticlesBufferInfo {
    vec3 colour;
    uint offset;
    float particle_duration;
    float half_size_linear;
    float last_particle_spawn_time;
};

struct Particle {
    vec3 position;
    float time_alive_percentage;
    vec3 velocity;
    vec3 view_space_position;
};

struct Time {
    float time_since_start;
    float delta_time;
};

struct LandCraft {
    vec3 position;
    float facing;
    mat3 rotation_matrix;
};
