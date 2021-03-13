// https://www.shadertoy.com/view/MsS3Wc
// Smooth HSV to RGB conversion
vec3 hsv2rgb_smooth(vec3 hsv) {
    vec3 c = hsv;

    vec3 rgb = clamp( abs(mod(c.x*6.0+vec3(0.0,4.0,2.0),6.0)-3.0)-1.0, 0.0, 1.0 );

    rgb = rgb*rgb*(3.0-2.0*rgb); // cubic smoothing

    return c.z * mix( vec3(1.0), rgb, c.y);
}

float random (vec2 st) {
    return fract(sin(dot(st.xy,
                         vec2(12.9898,78.233)))*
        43758.5453123);
}

// See https://github.com/KhronosGroup/glTF-Sample-Viewer/blob/master/source/Renderer/shaders/brdf.glsl
vec3 compute_f90(vec3 f0) {
    // Compute reflectance.
    float reflectance = max(max(f0.r, f0.g), f0.b);

    // Anything less than 2% is physically impossible and is instead considered
    // to be shadowing. Compare to "Real-Time-Rendering" 4th editon on page 325.
    vec3 f90 = vec3(clamp(reflectance * 50.0, 0.0, 1.0));
    return f90;
}

float clamped_dot(vec3 a, vec3 b) {
    return max(dot(a, b), 0.0);
}

vec2 repeat_over_bounds(vec2 position, float bounds) {
    bvec2 is_over_bounds = greaterThan(abs(position), vec2(bounds));
    vec2 offset = vec2(is_over_bounds) * sign(position) * (bounds * 2.0);
    return position - offset;
}

vec3 randomish_unit_vector(vec3 seed) {
    return normalize(vec3(
        random(seed.xy) - 0.5,
        random(seed.yz) - 0.5,
        random(seed.zx) - 0.5
    ));
}
