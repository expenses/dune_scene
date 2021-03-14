#include "../includes/rotor.glsl"

float cublic_spline_interpolate(
    float starting_point,
    float starting_out_tangent,
    float ending_point,
    float ending_in_tangent,
    float time_between_keyframes,
    float t
) {
    float p0 = starting_point;
    float m0 = starting_out_tangent * time_between_keyframes;
    float p1 = ending_point;
    float m1 = ending_in_tangent * time_between_keyframes;

    float t2 = t * t;
    float t3 = t * t * t;

    return p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
        + m0 * (t3 - 2.0 * t2 + t)
        + p1 * (-2.0 * t3 + 3.0 * t2)
        + m1 * (t3 - t2);
}

vec3 cublic_spline_interpolate(
    vec3 starting_point,
    vec3 starting_out_tangent,
    vec3 ending_point,
    vec3 ending_in_tangent,
    float time_between_keyframes,
    float t
) {
    vec3 p0 = starting_point;
    vec3 m0 = starting_out_tangent * time_between_keyframes;
    vec3 p1 = ending_point;
    vec3 m1 = ending_in_tangent * time_between_keyframes;

    float t2 = t * t;
    float t3 = t * t * t;

    return p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
        + m0 * (t3 - 2.0 * t2 + t)
        + p1 * (-2.0 * t3 + 3.0 * t2)
        + m1 * (t3 - t2);
}

Rotor cublic_spline_interpolate(
    Rotor starting_point,
    Rotor starting_out_tangent,
    Rotor ending_point,
    Rotor ending_in_tangent,
    float time_between_keyframes,
    float t
) {
    Rotor p0 = starting_point;
    Rotor m0 = rotor_mul_scalar(starting_out_tangent, time_between_keyframes);
    Rotor p1 = ending_point;
    Rotor m1 = rotor_mul_scalar(ending_in_tangent, time_between_keyframes);

    float t2 = t * t;
    float t3 = t * t * t;

    Rotor a = rotor_mul_scalar(p0, (2.0 * t3 - 3.0 * t2 + 1.0));
    Rotor b = rotor_mul_scalar(m0, (t3 - 2.0 * t2 + t));
    Rotor c = rotor_mul_scalar(p1, (-2.0 * t3 + 3.0 * t2));
    Rotor d = rotor_mul_scalar(m1, (t3 - t2));

    Rotor sum = rotor_add_rotor(rotor_add_rotor(a, b), rotor_add_rotor(c, d));

    return rotor_normalize(sum);
}