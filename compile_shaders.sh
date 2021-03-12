#!/bin/sh

# Allow globs that don't return anything
shopt -s nullglob

rm -r shaders/compiled/*.spv

for file in shaders/*.{vert,frag,comp}
do
output=shaders/compiled/$(basename $file).spv
glslc $file -o $output
spirv-opt $output -O -o $output
done

for file in shaders/**/*.{vert,frag,comp}
do
dir=$(basename $(dirname $file))
output="shaders/compiled/${dir}_$(basename $file).spv"
glslc $file -o $output
spirv-opt $output -O -o $output
done
