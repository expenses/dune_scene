uint cascade_index(float view_pos_z, vec3 splits) {
	// Compare the z against the split distances. We only care about the first
	// two. We want to find out how many splits the z is less than, as that's
	// our cascade index.
	bvec2 less_than_split = lessThan(vec2(view_pos_z), splits.xy);
	// Convert this bvec2 into a vec of integers that are either 0 or 1.
	uvec2 ints = uvec2(less_than_split);
	// Sum the integers to get a count and return it.
	uint count = ints.x + ints.y;
	return count;
}

vec3 debug_colour_for_cascade(uint cascade_index) {
	vec3 colours[3] = {
		vec3(1.0, 0.25, 0.25),
		vec3(0.25, 1.0, 0.25),
		vec3(0.25, 0.25, 1.0)
	};

	return colours[cascade_index];
}

float percentage_closer_filtering(vec2 light_local, uint cascade_index, float comparison) {
    vec2 step = 1.0 / textureSize(sampler2DArrayShadow(SHADOW_TEXTURE_ARRAY, SHADOW_SAMPLER), 0).xy;

    float shadow_sum = 0.0;
    // This is expensive so we only do a 3x3 kernel.
    int kernel_size = 3;
    int count = kernel_size * kernel_size;
    int range = (kernel_size - 1) / 2;

    for (int x = -range; x <= range; x++) {
        for (int y = -range; y <= range; y++) {
            vec2 offset = vec2(x, y);
            shadow_sum += texture(
                sampler2DArrayShadow(SHADOW_TEXTURE_ARRAY, SHADOW_SAMPLER),
                vec4(light_local + step * vec2(x, y), cascade_index, comparison)
            );
        }
    }

    return shadow_sum / count;
}

// See https://github.com/gfx-rs/wgpu-rs/blob/cadc2df8a106ad122c10c2e07733ade8f1e5653c/examples/shadow/shader.wgsl#L67
float calculate_shadow(float view_pos_z, mat4 matrices[3], vec3 splits, vec3 frag_pos) {
	uint cascade_index = cascade_index(view_pos_z, splits);
	vec4 transformed_coords = matrices[cascade_index] * vec4(frag_pos, 1.0);

    /*
	if (transformed_coords.w <= 0.0) {
        return 1.0;
    }
	*/

    // compensate for the Y-flip difference between the NDC and texture coordinates
    vec2 flip_correction = vec2(0.5, -0.5);
    // compute texture coordinates for shadow lookup
    float proj_correction = 1.0 / transformed_coords.w;
	vec2 light_local = transformed_coords.xy * flip_correction * proj_correction + vec2(0.5, 0.5);
    // do the lookup, using HW PCF and comparison
	float bias = 0.005;
    float comparison = transformed_coords.z * proj_correction - bias;

    return percentage_closer_filtering(light_local, cascade_index, comparison);
}
