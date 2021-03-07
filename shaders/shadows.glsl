const mat4 shadow_bias_mat = mat4(
	0.5, 0.0, 0.0, 0.0,
	0.0, 0.5, 0.0, 0.0,
	0.0, 0.0, 1.0, 0.0,
	0.5, 0.5, 0.0, 1.0
);

uint cascade_index(float view_pos_z, vec3 splits) {
    uint index = 0;

    for (uint i = 0; i < 2; i++) {
        if (view_pos_z < splits[i]) {
            index = i + 1;
        }
    }

    return index;
}

vec3 debug_colour_by_cascade(vec3 colour, uint cascade_index) {
	switch(cascade_index) {
		case 0:
			colour *= vec3(1.0f, 0.25f, 0.25f);
			break;
		case 1:
			colour *= vec3(0.25f, 1.0f, 0.25f);
			break;
		case 2:
			colour *= vec3(0.25f, 0.25f, 1.0f);
			break;
	}

	return colour;
}

float textureProj(vec2 uv, float z, float w, uint cascade_index) {
	float shadow = 1.0;
	float bias = 0.005;

	if ( z > -1.0 && z < 1.0 ) {
		float dist = texture(SHADOW_MAP, vec3(uv, cascade_index)).r;
		if (w > 0 && dist < z - bias) {
			shadow = 0.0;
		}
	}
	return shadow;
}

float filter_pcf(vec2 uv, float z, float w, uint cascadeIndex) {
	ivec2 texDim = textureSize(SHADOW_MAP, 0).xy;
	float scale = 0.75;
	float dx = scale * 1.0 / float(texDim.x);
	float dy = scale * 1.0 / float(texDim.y);

	float shadowFactor = 0.0;
	int range = 1;
	int count = (range * 2 + 1) * (range * 2 + 1);

	for (int x = -range; x <= range; x++) {
		for (int y = -range; y <= range; y++) {
			shadowFactor += textureProj(uv + vec2(dx*x, dy*y), z, w, cascadeIndex);
		}
	}
	return shadowFactor / count;
}

float calculate_shadow(float view_pos_z, mat4 matrices[3], vec3 splits, vec3 frag_pos) {
	uint cascade_index = cascade_index(view_pos_z, splits);
	vec4 light_space_pos = matrices[cascade_index] * vec4(frag_pos, 1.0);

	vec4 coords = light_space_pos / light_space_pos.w;

    vec2 uv = vec2(
        (coords.x + 1.0) / 2.0,
		// VERY important for webgpu reasons!!
        (1.0 - coords.y) / 2.0
    );

    return filter_pcf(uv, coords.z, coords.w, cascade_index);
}
