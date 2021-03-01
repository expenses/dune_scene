#!/bin/sh

glslc shaders/scene.vert -o shaders/compiled/scene.vert.spv
glslc shaders/scene.frag -o shaders/compiled/scene.frag.spv

spirv-opt shaders/compiled/scene.vert.spv -O -o shaders/compiled/scene.vert.spv
spirv-opt shaders/compiled/scene.frag.spv -O -o shaders/compiled/scene.frag.spv
