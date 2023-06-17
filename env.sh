#/bin/bash
export SYSTEM_ROOT=image-mirror
export STANDARD_USER=boilerman

export IMG_DISK=image_disk.img
export MOUNT_POINT="build_dir/mount_point"

export LOSETUP="/usr/sbin/losetup"
export LOOP_DEVICE=`$LOSETUP -f`

export ROOT_TOOLCHAIN="/toolchain_turbofish"
export TURBOFISH_ROOT=`pwd`
export TOOLCHAIN_TURBOFISH=$ROOT_TOOLCHAIN

export TARGET="i686-turbofish"
export PATH="$ROOT_TOOLCHAIN/cross/bin:$PATH"

export KERNEL_DIRECTORY="rust_kernel"