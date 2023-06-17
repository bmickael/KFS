#!/bin/bash
source ./env.sh

rm -rvf $SYSTEM_ROOT
rm -vf $IMG_DISK
sudo rm -r $ROOT_TOOLCHAIN
make -C libc clean
make -C programs clean
make -C $KERNEL_DIRECTORY clean
rm -rvf build_dir
exit 0