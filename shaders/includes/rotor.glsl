// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L606-L637
mat3 rotor_to_matrix(Rotor self) {
    float s2 = self.s * self.s;
    float bxy2 = self.bv_xy * self.bv_xy;
    float bxz2 = self.bv_xz * self.bv_xz;
    float byz2 = self.bv_yz * self.bv_yz;
    float s_bxy = self.s * self.bv_xy;
    float s_bxz = self.s * self.bv_xz;
    float s_byz = self.s * self.bv_yz;
    float bxz_byz = self.bv_xz * self.bv_yz;
    float bxy_byz = self.bv_xy * self.bv_yz;
    float bxy_bxz = self.bv_xy * self.bv_xz;

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

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L291-L297
Rotor rotor_mul_scalar(Rotor rotor, float scalar) {
    rotor.s *= scalar;
    rotor.bv_xy *= scalar;
    rotor.bv_xz *= scalar;
    rotor.bv_yz *= scalar;
    return rotor;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L248-L254
Rotor rotor_add_rotor(Rotor a, Rotor b) {
    a.s += b.s;
    a.bv_xy += b.bv_xy;
    a.bv_xz += b.bv_xz;
    a.bv_yz += b.bv_yz;
    return a;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L122-L137
Rotor rotor_normalize(Rotor rotor) {
    float mag_sq = (rotor.bv_xy * rotor.bv_xy) + (rotor.bv_xz * rotor.bv_xz) +
        (rotor.bv_yz * rotor.bv_yz) + (rotor.s * rotor.s);
    float mag = sqrt(mag_sq);
    rotor.s /= mag;
    rotor.bv_xy /= mag;
    rotor.bv_xz /= mag;
    rotor.bv_yz /= mag;
    return rotor;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L675-L691
Rotor rotor_mul_rotor(Rotor self, Rotor q) {
    Rotor res;
    res.s = self.s * q.s - self.bv_xy * q.bv_xy - self.bv_xz * q.bv_xz - self.bv_yz * q.bv_yz;
    res.bv_xy = self.bv_xy * q.s + self.s * q.bv_xy + self.bv_yz * q.bv_xz - self.bv_xz * q.bv_yz;
    res.bv_xz = self.bv_xz * q.s + self.s * q.bv_xz - self.bv_yz * q.bv_xy + self.bv_xy * q.bv_yz;
    res.bv_yz = self.bv_yz * q.s + self.s * q.bv_yz + self.bv_xz * q.bv_xy - self.bv_xy * q.bv_xz;
    return res;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L550-L563
vec3 rotor_mul_vec(Rotor self, vec3 vec) {
    float fx = self.s * vec.x + self.bv_xy * vec.y + self.bv_xz * vec.z;
    float fy = self.s * vec.y - self.bv_xy * vec.x + self.bv_yz * vec.z;
    float fz = self.s * vec.z - self.bv_xz * vec.x - self.bv_yz * vec.y;
    float fw = self.bv_xy * vec.z - self.bv_xz * vec.y + self.bv_yz * vec.x;

    vec.x = self.s * fx + self.bv_xy * fy + self.bv_xz * fz + self.bv_yz * fw;
    vec.y = self.s * fy - self.bv_xy * fx - self.bv_xz * fw + self.bv_yz * fz;
    vec.z = self.s * fz + self.bv_xy * fw - self.bv_xz * fx - self.bv_yz * fy;
    return vec;
}
