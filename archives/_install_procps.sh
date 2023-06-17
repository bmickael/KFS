#!/bin/bash
export TARGET="i686-turbofish"
export PATH="/toolchain_turbofish/cross/bin:$PATH"
export TARGET_DIR="../../../../system_disk/bin"
export BUILD_DIR="build_dir/build_procps"
export PATCH1="patch-procps-config-sub"
export PATCH2="patch-procps-configure"

mkdir -pv $BUILD_DIR
cp -v patch/$PATCH1 patch/$PATCH2 $BUILD_DIR
cd $BUILD_DIR
wget -c "https://downloads.sourceforge.net/project/procps-ng/Production/procps-ng-3.3.16.tar.xz"
tar -xf 'procps-ng-3.3.16.tar.xz'
patch -p0 < $PATCH1
patch -p0 < $PATCH2
cd procps-ng-3.3.16
# rm -rf build
mkdir -pv build
cd build
CFLAGS="-g -O0 -fno-omit-frame-pointer" ../configure --without-ncurses --disable-modern-top  --disable-pidof --disable-kill   --disable-nls --disable-rpath --disable-numa --build="`gcc -dumpmachine`" --host=$TARGET
make
# sudo cp -v ps/pscommand $TARGET_DIR/ps
# sudo cp -v free $TARGET_DIR/free
