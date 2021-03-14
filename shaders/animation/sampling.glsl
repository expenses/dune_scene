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
    vec3 time_between_keyframes,
    vec3 t
) {
    vec3 p0 = starting_point;
    vec3 m0 = starting_out_tangent * time_between_keyframes;
    vec3 p1 = ending_point;
    vec3 m1 = ending_in_tangent * time_between_keyframes;

    vec3 t2 = t * t;
    vec3 t3 = t * t * t;

    return p0 * (2.0 * t3 - 3.0 * t2 + 1.0)
        + m0 * (t3 - 2.0 * t2 + t)
        + p1 * (-2.0 * t3 + 3.0 * t2)
        + m1 * (t3 - t2);
}

void sample_cubic_spline_float(float t, Channel channel, out SAMPLE_TYPE output_sample, out bool invalid) {
    uint base = channel.offset;
    uint size = channel.size;

    if (t < CHANNEL_INPUTS[base] || t > CHANNEL_INPUTS[base + size - 1]) {
        invalid = true;
        return;
    }

    uint i = base;

    while (i < base + size && CHANNEL_INPUTS[i + 1] < t) {
        i++;
    }

    SAMPLE_TYPE previous_time = CHANNEL_INPUTS[i];
    SAMPLE_TYPE next_time = CHANNEL_INPUTS[i + 1];
    SAMPLE_TYPE delta = next_time - previous_time;
    SAMPLE_TYPE from_start = t - previous_time;
    SAMPLE_TYPE factor = from_start / delta;

    SAMPLE_TYPE starting_point = CHANNEL_OUTPUTS[i * 3 + 1];
    SAMPLE_TYPE starting_out_tangent = CHANNEL_OUTPUTS[i * 3 + 2];

    SAMPLE_TYPE ending_in_tangent = CHANNEL_OUTPUTS[i * 3 + 3];
    SAMPLE_TYPE ending_point = CHANNEL_OUTPUTS[i * 3 + 4];

    output_sample = cublic_spline_interpolate(
        starting_point,
        starting_out_tangent,
        ending_point,
        ending_in_tangent,
        delta,
        factor
    );

    return;
}
