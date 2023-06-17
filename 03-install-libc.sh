#!/bin/bash
source ./env.sh

set -e
make -C libc clean
make -C libc
exit 0