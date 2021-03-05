#!/bin/sh

glslc shaders/scene.vert -o shaders/compiled/scene.vert.spv
glslc shaders/scene.frag -o shaders/compiled/scene.frag.spv
glslc shaders/sun_dir.vert -o shaders/compiled/sun_dir.vert.spv
glslc shaders/flat_colour.frag -o shaders/compiled/flat_colour.frag.spv

spirv-opt shaders/compiled/scene.vert.spv -O -o shaders/compiled/scene.vert.spv
spirv-opt shaders/compiled/scene.frag.spv -O -o shaders/compiled/scene.frag.spv
spirv-opt shaders/compiled/sun_dir.vert.spv -O -o shaders/compiled/sun_dir.vert.spv
spirv-opt shaders/compiled/flat_colour.frag.spv -O -o shaders/compiled/flat_colour.frag.spv
