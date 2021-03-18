
mat4 similarity_to_mat4(Similarity sim) {
    mat4 scale_matrix = scale_matrix(sim.scale);
    mat4 translation_matrix = translation_matrix(sim.translation);
    mat4 rotation_matrix = mat4(rotor_to_matrix(sim.rotation));

    return translation_matrix * rotation_matrix * scale_matrix;
}

vec3 similarity_transform_vec(Similarity sim, vec3 vec) {
    vec = rotor_mul_vec(sim.rotation, vec);
    vec *= sim.scale;
    vec += sim.translation;
    return vec;
}

Similarity similarity_mul_similarity(Similarity sim, Similarity base) {
    sim.translation = similarity_transform_vec(sim, base.translation);
    sim.rotation = rotor_mul_rotor(sim.rotation, base.rotation);
    sim.scale *= base.scale;
    return sim;
}

Similarity similarity_mul_scalar(Similarity sim, float scalar) {
    sim.translation *= scalar;
    sim.rotation = rotor_mul_scalar(sim.rotation, scalar);
    sim.scale *= scalar;
    return sim;
}

Similarity similarity_add_similarity(Similarity sim, Similarity base) {
    sim.translation += base.translation;
    sim.rotation = rotor_add_rotor(sim.rotation, base.rotation);
    sim.scale += base.scale;
    return sim;
}
