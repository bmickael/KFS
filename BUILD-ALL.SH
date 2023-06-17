#!/bin/bash
set -e

./00-create-image-mirror.sh
./01-create-grubable-image-disk.sh
./02-install-toolchain-turbofish.sh
./03-install-libc.sh
./04-install-coreutils.sh
./05-install-dash.sh
./06-install-misc.sh
./10-install-kernel.sh
exit 0