#!/bin/bash
source ./env.sh

# local description
BUILD_DIR="build_dir/build_toolchain"
PATCH_BINUTILS="patch-binutils"
PATCH_GCC="patch-gcc"
ROOT_TOOLCHAIN="/toolchain_turbofish"
TARGET="i686-turbofish"
TOOLCHAIN_SYSROOT="$ROOT_TOOLCHAIN/sysroot"
CROSS="$ROOT_TOOLCHAIN/cross"
LIBC_DIR="libc"
HOST_TRIPLET="`gcc -dumpmachine`"

set -e
sudo mkdir -pv $ROOT_TOOLCHAIN
sudo chown $USER:$USER $ROOT_TOOLCHAIN
mkdir -pv $TOOLCHAIN_SYSROOT $CROSS
mkdir -pv $TOOLCHAIN_SYSROOT/usr
mkdir -pv $TOOLCHAIN_SYSROOT/usr/{lib,include}
cp -rv $LIBC_DIR/include/* $TOOLCHAIN_SYSROOT/usr/include

mkdir -pv $BUILD_DIR
cp -v patch/$PATCH_BINUTILS patch/$PATCH_GCC $BUILD_DIR
cd $BUILD_DIR

# CROSS COMPILE BINUTILS
wget -c 'https://ftp.gnu.org/gnu/binutils/binutils-2.32.tar.xz'
tar -xf 'binutils-2.32.tar.xz'
patch -p0 < $PATCH_BINUTILS
cd 'binutils-2.32'
# In LD subdirectory (Maybe install automake 1.15.1)
cd ld
automake-1.15
if [ $? -ne 0 ]; then
  echo "automake-1.15 command failure. Please fix this problem"
  exit 1
fi
cd -
# Create a build directory in binutils
mkdir -p build
cd build
../configure --build=$HOST_TRIPLET --host=$HOST_TRIPLET --target=$TARGET --prefix=$CROSS --with-sysroot=$TOOLCHAIN_SYSROOT
make -j8
make install
cd ../..

# CROSS COMPILE GCC
echo 'WARNING: you must make install on libc to install the headers before compiling gcc'
sudo apt install g++ libmpc-dev libmpfr-dev libgmp-dev
wget -c 'https://ftp.gnu.org/gnu/gcc/gcc-9.1.0/gcc-9.1.0.tar.xz'
tar -xf 'gcc-9.1.0.tar.xz'
patch -p0 < $PATCH_GCC
cd 'gcc-9.1.0'
mkdir -p build
cd build
../configure --build=$HOST_TRIPLET --host=$HOST_TRIPLET --target=$TARGET --prefix=$CROSS --with-sysroot=$TOOLCHAIN_SYSROOT --enable-languages=c,c++ --enable-initfini-array
make -j8 all-gcc all-target-libgcc
make install-gcc install-target-libgcc

rm -vf /toolchain_turbofish/cross/lib/gcc/i686-turbofish/9.1.0/crti.o
rm -vf /toolchain_turbofish/cross/lib/gcc/i686-turbofish/9.1.0/crtn.o
# rm -vf /toolchain_turbofish/cross/lib/gcc/i686-turbofish/9.1.0/crtbegin.o
# rm -vf /toolchain_turbofish/cross/lib/gcc/i686-turbofish/9.1.0/crtend.o
