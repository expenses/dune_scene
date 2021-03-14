// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L606-L637
mat3 rotor_to_matrix(Rotor self) {
    float s2 = self.s * self.s;
    float bxy2 = self.bv.xy * self.bv.xy;
    float bxz2 = self.bv.xz * self.bv.xz;
    float byz2 = self.bv.yz * self.bv.yz;
    float s_bxy = self.s * self.bv.xy;
    float s_bxz = self.s * self.bv.xz;
    float s_byz = self.s * self.bv.yz;
    float bxz_byz = self.bv.xz * self.bv.yz;
    float bxy_byz = self.bv.xy * self.bv.yz;
    float bxy_bxz = self.bv.xy * self.bv.xz;

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
    rotor.bv.xy *= scalar;
    rotor.bv.xz *= scalar;
    rotor.bv.yz *= scalar;
    return rotor;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L248-L254
Rotor rotor_add_rotor(Rotor a, Rotor b) {
    a.s += b.s;
    a.bv.xy += b.bv.xy;
    a.bv.xz += b.bv.xz;
    a.bv.yz += b.bv.yz;
    return a;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L122-L137
Rotor rotor_normalize(Rotor rotor) {
    float mag_sq = rotor.bv.xy * rotor.bv.xy + rotor.bv.xz * rotor.bv.xz + rotor.bv.yz * rotor.bv.yz + rotor.s * rotor.s;
    float inv_mag = inversesqrt(mag_sq);
    rotor.s *= inv_mag;
    rotor.bv.xy *= inv_mag;
    rotor.bv.xz *= inv_mag;
    rotor.bv.yz *= inv_mag;
    return rotor;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L675-L691
Rotor rotor_mul_rotor(Rotor self, Rotor q) {
    Rotor res;
    res.s = self.s * q.s - self.bv.xy * q.bv.xy - self.bv.xz * q.bv.xz - self.bv.yz * q.bv.yz;
    res.bv.xy = self.bv.xy * q.s + self.s * q.bv.xy + self.bv.yz * q.bv.xz - self.bv.xz * q.bv.yz;
    res.bv.xz = self.bv.xz * q.s + self.s * q.bv.xz - self.bv.yz * q.bv.xy + self.bv.xy * q.bv.yz;
    res.bv.yz = self.bv.yz * q.s + self.s * q.bv.yz + self.bv.xz * q.bv.xy - self.bv.xy * q.bv.xz;
    return res;
}

// https://github.com/termhn/ultraviolet/blob/9653d78b68aa19659b904d33d33239bbd2907504/src/rotor.rs#L550-L563
vec3 rotor_mul_vec(Rotor self, vec3 vec) {
    float fx = self.s * vec.x + self.bv.xy * vec.y + self.bv.xz * vec.z;
    float fy = self.s * vec.y - self.bv.xy * vec.x + self.bv.yz * vec.z;
    float fz = self.s * vec.z - self.bv.xz * vec.x - self.bv.yz * vec.y;
    float fw = self.bv.xy * vec.z - self.bv.xz * vec.y + self.bv.yz * vec.x;

    vec.x = self.s * fx + self.bv.xy * fy + self.bv.xz * fz + self.bv.yz * fw;
    vec.y = self.s * fy - self.bv.xy * fx - self.bv.xz * fw + self.bv.yz * fz;
    vec.z = self.s * fz + self.bv.xy * fw - self.bv.xz * fx - self.bv.yz * fy;
    return vec;
}
