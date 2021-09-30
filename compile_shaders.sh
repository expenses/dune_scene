#!/bin/sh

# Allow globs that don't return anything
shopt -s nullglob
# Allow globs with ignores
shopt -s extglob

rm -r shaders/compiled/*.spv

for file in shaders/*.{vert,frag,comp}
do
output=shaders/compiled/$(basename $file).spv
glslc $file -o $output
done

for file in shaders/!(animation)/*.{vert,frag,comp} \
    shaders/animation/compute_joint_transforms.comp \
    shaders/animation/set_global_transforms.comp
do
dir=$(basename $(dirname $file))
output="shaders/compiled/${dir}_$(basename $file).spv"
glslc $file -o $output
done

glslc -DSAMPLE_TYPE=float -DFIELD=scale shaders/animation/sample_generic.comp \
    -o shaders/compiled/animation_sample_scales.comp.spv

glslc -DSAMPLE_TYPE=vec3 -DFIELD=translation shaders/animation/sample_generic.comp \
    -o shaders/compiled/animation_sample_translations.comp.spv

glslc -DSAMPLE_TYPE=Rotor -DFIELD=rotation shaders/animation/sample_generic.comp \
    -o shaders/compiled/animation_sample_rotations.comp.spv

for file in shaders/compiled/*.spv
do
spirv-opt $file -O -o $file
done
