#!/bin/bash
source ./env.sh

DD="/usr/bin/dd"
FDISK="/usr/sbin/fdisk"
MKFS="/usr/sbin/mkfs.ext2"
MOUNT="/usr/bin/mount"
UMOUNT="/usr/bin/umount"
GRUB_INSTALL="/usr/sbin/grub-install"
READLINK="/usr/bin/readlink"
RSYNC="/usr/bin/rsync"
CHOWN="/usr/bin/chown"
CHMOD="/usr/bin/chmod"

set -e
sudo $LOSETUP -fP $IMG_DISK
sudo $MOUNT ${LOOP_DEVICE}p1 $MOUNT_POINT

# synchronize all modifieds files
echo ""
echo "### Syncing files ###"
sudo $RSYNC -rltDv $SYSTEM_ROOT/ $MOUNT_POINT
echo ""

sudo $CHOWN -R 0:0 $MOUNT_POINT
sudo $CHMOD -R 0700 $MOUNT_POINT/root
sudo $CHOWN -R 1000:1000 $MOUNT_POINT/home/$STANDARD_USER

sudo $UMOUNT $MOUNT_POINT
sudo $LOSETUP -d $LOOP_DEVICE