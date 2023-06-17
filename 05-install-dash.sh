#!/bin/bash
source ./env.sh

TARGET_DIR=`readlink -f $SYSTEM_ROOT`/bin
BUILD_DIR="build_dir/build_dash"

set -e
mkdir -pv $BUILD_DIR
cd $BUILD_DIR
rm -rf dash-0.5.10
wget -c 'http://gondor.apana.org.au/~herbert/dash/files/dash-0.5.10.tar.gz'
tar -xf 'dash-0.5.10.tar.gz'
cd dash-0.5.10
mkdir build
cd build
CFLAGS="-O3 -fno-omit-frame-pointer -Wl,--gc-sections" ../configure --build=`gcc -dumpmachine` --host=$TARGET
make

cp -v src/dash $TARGET_DIR
ln -s -v --force /bin/dash $TARGET_DIR/sh
