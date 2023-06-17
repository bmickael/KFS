#!/bin/bash
source ./env.sh

export SYSTEM_ROOT=$TURBOFISH_ROOT/image-mirror

set -e
make -C programs clean
make -C programs
exit 0