// https://en.wikipedia.org/wiki/Rotation_matrix#Basic_rotations

mat3 rotation_matrix_x(float sin, float cos) {
    return mat3(
        1, 0, 0,
        0, cos, -sin,
        0, sin, cos
    );
}

mat3 rotation_matrix_x(vec2 sin_cos) {
    return rotation_matrix_x(sin_cos.x, sin_cos.y);
}

mat3 rotation_matrix_x(float theta) {
    return rotation_matrix_x(sin(theta), cos(theta));
}

mat3 rotation_matrix_y(float sin, float cos) {
    return mat3(
        cos, 0, sin,
        0, 1, 0,
        -sin, 0, cos
    );
}

mat3 rotation_matrix_y(vec2 sin_cos) {
    return rotation_matrix_y(sin_cos.x, sin_cos.y);
}

mat3 rotation_matrix_y(float theta) {
    return rotation_matrix_y(sin(theta), cos(theta));
}


mat3 rotation_matrix_z(float sin, float cos) {
    return mat3(
        cos, -sin, 0,
        sin, cos, 0,
        0, 0, 1
    );
}

mat3 rotation_matrix_z(vec2 sin_cos) {
    return rotation_matrix_z(sin_cos.x, sin_cos.y);
}

mat3 rotation_matrix_z(float theta) {
    return rotation_matrix_z(sin(theta), cos(theta));
}

mat2 rotation_matrix(float theta) {
    return mat2(
        cos(theta), sin(theta),
        -sin(theta), cos(theta)
    );
}

mat4 scale_matrix(float scale) {
    return mat4(
        vec4(scale, 0.0, 0.0, 0.0),
        vec4(0.0, scale, 0.0, 0.0),
        vec4(0.0, 0.0, scale, 0.0),
        vec4(0.0, 0.0, 0.0, 1.0)
    );
}
