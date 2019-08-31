#!/bin/bash
export TARGET="i686-turbofish"
export PATH="/toolchain_turbofish/cross/bin:$PATH"
export TARGET_DIR="../../../rust_kernel/src/userland"

mkdir -pv build_dash
cd build_dash
wget -c 'http://gondor.apana.org.au/~herbert/dash/files/dash-0.5.10.tar.gz'
tar -xf 'dash-0.5.10.tar.gz'
cd dash-0.5.10
mkdir build
cd build
CFLAGS="-g -O0 -fno-omit-frame-pointer" ../configure --host=$TARGET
make
cp -v src/dash $TARGET_DIR