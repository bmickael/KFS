#!/bin/bash
source ./env.sh

rm -rvf $SYSTEM_ROOT
set -e
mkdir -pv $SYSTEM_ROOT
mkdir -pv $SYSTEM_ROOT/bin
mkdir -pv $SYSTEM_ROOT/bin/wolf3D
mkdir -pv $SYSTEM_ROOT/dev
mkdir -pv $SYSTEM_ROOT/etc
mkdir -pv $SYSTEM_ROOT/var
mkdir -pv $SYSTEM_ROOT/grub
mkdir -pv $SYSTEM_ROOT/home
mkdir -pv $SYSTEM_ROOT/home/$STANDARD_USER
mkdir -pv $SYSTEM_ROOT/turbofish
mkdir -pv $SYSTEM_ROOT/turbofish/mod
mkdir -pv $SYSTEM_ROOT/root
cp -v files/shinit $SYSTEM_ROOT/root/.shinit
cp -v files/shinit $SYSTEM_ROOT/home/$STANDARD_USER/.shinit
cp -v files/pulp_fiction.txt $SYSTEM_ROOT/home/$STANDARD_USER
cp -v common/medias/univers.bmp $SYSTEM_ROOT/home
cp -v common/medias/wanggle.bmp $SYSTEM_ROOT/home
cp -v common/medias/asterix.bmp $SYSTEM_ROOT/home
exit 0
