#!/bin/sh

rm -r shaders/compiled/*.spv

for file in shaders/*.{vert,frag,comp}
do
output=shaders/compiled/$(basename $file).spv
glslc $file -o $output
spirv-opt $output -O -o $output
done
