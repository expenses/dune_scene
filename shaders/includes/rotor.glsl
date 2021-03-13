// vec -> bivec:
// x = xy
// y = xz
// z = yz

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L606-L637
mat3 rotor_to_matrix(Rotor r) {
    float s2 = r.s * r.s;
    float bxy2 = r.bv.x * r.bv.x;
    float bxz2 = r.bv.y * r.bv.y;
    float byz2 = r.bv.z * r.bv.z;
    float s_bxy = r.s * r.bv.x;
    float s_bxz = r.s * r.bv.y;
    float s_byz = r.s * r.bv.z;
    float bxz_byz = r.bv.y * r.bv.z;
    float bxy_byz = r.bv.x * r.bv.z;
    float bxy_bxz = r.bv.x * r.bv.y;

    float two = 2.0;

    return mat3(
        vec3(
            s2 - bxy2 - bxz2 + byz2,
            -two * (bxz_byz + s_bxy),
            two * (bxy_byz - s_bxz)),
        vec3(
            two * (s_bxy - bxz_byz),
            s2 - bxy2 + bxz2 - byz2,
            -two * (s_byz + bxy_bxz)
        ),
        vec3(
            two * (s_bxz + bxy_byz),
            two * (s_byz - bxy_bxz),
            s2 + bxy2 - bxz2 - byz2
        )
    );
}
