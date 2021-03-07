#!/bin/sh

glslc shaders/scene.vert -o shaders/compiled/scene.vert.spv
glslc shaders/scene.frag -o shaders/compiled/scene.frag.spv

glslc shaders/ship.vert -o shaders/compiled/ship.vert.spv
glslc shaders/ship.frag -o shaders/compiled/ship.frag.spv

glslc shaders/sun_dir.vert -o shaders/compiled/sun_dir.vert.spv
glslc shaders/line.vert -o shaders/compiled/line.vert.spv

glslc shaders/flat_colour.frag -o shaders/compiled/flat_colour.frag.spv

glslc shaders/fullscreen_tri.vert -o shaders/compiled/fullscreen_tri.vert.spv
glslc shaders/tonemap.frag -o shaders/compiled/tonemap.frag.spv

glslc shaders/ship_movement.comp -o shaders/compiled/ship_movement.comp.spv

spirv-opt shaders/compiled/scene.vert.spv -O -o shaders/compiled/scene.vert.spv
spirv-opt shaders/compiled/scene.frag.spv -O -o shaders/compiled/scene.frag.spv

spirv-opt shaders/compiled/ship.vert.spv -O -o shaders/compiled/ship.vert.spv
spirv-opt shaders/compiled/ship.frag.spv -O -o shaders/compiled/ship.frag.spv

spirv-opt shaders/compiled/sun_dir.vert.spv -O -o shaders/compiled/sun_dir.vert.spv
spirv-opt shaders/compiled/line.vert.spv -O -o shaders/compiled/line.vert.spv

spirv-opt shaders/compiled/flat_colour.frag.spv -O -o shaders/compiled/flat_colour.frag.spv

spirv-opt shaders/compiled/fullscreen_tri.vert.spv -O -o shaders/compiled/fullscreen_tri.vert.spv
spirv-opt shaders/compiled/tonemap.frag.spv -O -o shaders/compiled/tonemap.frag.spv

spirv-opt shaders/compiled/ship_movement.comp.spv -O -o shaders/compiled/ship_movement.comp.spv
