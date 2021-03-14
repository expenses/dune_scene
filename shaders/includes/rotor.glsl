// vec -> bivec:
// x = xy
// y = xz
// z = yz

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L606-L637
mat3 rotor_to_matrix(Rotor r) {
    float s2 = r.s * r.s;
    vec3 bv2 = r.bv * r.bv;
    vec3 s_bv = r.s * r.bv;

    float bxz_byz = r.bv.y * r.bv.z;
    float bxy_byz = r.bv.x * r.bv.z;
    float bxy_bxz = r.bv.x * r.bv.y;

    float two = 2.0;

    return mat3(
        vec3(
            s2 - bv2.x - bv2.y + bv2.z,
            -two * (bxz_byz + s_bv.x),
            two * (bxy_byz - s_bv.y)),
        vec3(
            two * (s_bv.x - bxz_byz),
            s2 - bv2.x + bv2.y - bv2.z,
            -two * (s_bv.z + bxy_bxz)
        ),
        vec3(
            two * (s_bv.y + bxy_byz),
            two * (s_bv.z - bxy_bxz),
            s2 + bv2.x - bv2.y - bv2.z
        )
    );
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L291-L297
Rotor rotor_mul_scalar(Rotor rotor, float scalar) {
    rotor.s *= scalar;
    rotor.bv *= scalar;
    return rotor;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L248-L254
Rotor rotor_add_rotor(Rotor a, Rotor b) {
    a.s += b.s;
    a.bv += b.bv;
    return a;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L122-L137
Rotor rotor_normalize(Rotor rotor) {
    float mag_sq = dot(rotor.bv, rotor.bv) + rotor.s * rotor.s;
    float inv_mag = inversesqrt(mag_sq);
    rotor.s *= inv_mag;
    rotor.bv *= inv_mag;
    return rotor;
}

Rotor rotor_mul_rotor(Rotor r, Rotor q) {
    Rotor res;
    res.s = r.s * q.s - r.bv.x * q.bv.x - r.bv.y * q.bv.y - r.bv.z * q.bv.z;
    res.bv = vec3(
        r.bv.x * q.s + r.s * q.bv.x + r.bv.z * q.bv.y - r.bv.y * q.bv.z,
        r.bv.y * q.s + r.s * q.bv.y - r.bv.z * q.bv.x + r.bv.x * q.bv.y,
        r.bv.z * q.s + r.s * q.bv.z + r.bv.y * q.bv.x - r.bv.x * q.bv.z
    );
    return res;
}

vec3 rotor_mul_vec(Rotor r, vec3 vec) {
    float fx = r.s * vec.x + r.bv.x * vec.y + r.bv.y * vec.z;
    float fy = r.s * vec.y - r.bv.x * vec.x + r.bv.z * vec.z;
    float fz = r.s * vec.z - r.bv.y * vec.x - r.bv.z * vec.y;
    float fw = r.bv.x * vec.z - r.bv.y * vec.y + r.bv.z * vec.x;

    return vec3(
        r.s * fx + r.bv.x * fy + r.bv.y * fz + r.bv.z * fw,
        r.s * fy - r.bv.x * fx - r.bv.y * fw + r.bv.z * fz,
        r.s * fz + r.bv.x * fw - r.bv.y * fx - r.bv.z * fy
    );
}
