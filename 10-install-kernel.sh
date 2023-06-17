#!/bin/bash
source ./env.sh

export SYSTEM_ROOT=$TURBOFISH_ROOT/image-mirror

set -e
make -C $KERNEL_DIRECTORY
cp -vf $KERNEL_DIRECTORY/build/kernel.elf $SYSTEM_ROOT/turbofish
./sync.sh
exit 0
