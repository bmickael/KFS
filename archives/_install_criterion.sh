#!/bin/bash
export BUILD_DIR="build_dir/build_criterion"

mkdir -pv $BUILD_DIR
cd $BUILD_DIR
git clone --recursive https://github.com/Snaipe/Criterion.git
cd Criterion
mkdir -pv build
cd build
cmake ..
cmake --build .
make install
