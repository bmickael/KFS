#!/bin/bash
source ./env.sh

IMAGE_SIZE=$((512 * 1024))
FIRST_PART_SIZE=$(($IMAGE_SIZE - 1024 * 10))
DEVICE_MAP_FILE="build_dir/loopdevice.map"

DD="/usr/bin/dd"
FDISK="/usr/sbin/fdisk"
MKFS="/usr/sbin/mkfs.ext2"
MOUNT="/usr/bin/mount"
UMOUNT="/usr/bin/umount"
GRUB_INSTALL="/usr/sbin/grub-install"
READLINK="/usr/bin/readlink"

set -e
$DD if=/dev/zero of=$IMG_DISK bs=1024 count=$IMAGE_SIZE

echo -e "o\nn\np\n1\n2048\n${FIRST_PART_SIZE}\na\nw\n" | $FDISK $IMG_DISK
echo -e "n\np\n2\n\n\nw\n" | $FDISK $IMG_DISK
sudo $LOSETUP -fP $IMG_DISK
sudo $MKFS ${LOOP_DEVICE}p1
sudo $MKFS ${LOOP_DEVICE}p2
mkdir -pv $MOUNT_POINT
sudo $MOUNT ${LOOP_DEVICE}p1 $MOUNT_POINT
echo "(hd0) " $LOOP_DEVICE > $DEVICE_MAP_FILE

# test - This module provides the "test" command which is used to evaluate an expression.
# echo - This module provides the "echo" command.
# vga - This module provides VGA support.
# normal - This module provides "Normal Mode" which is the opposite of "Rescue Mode".
# elf - This module loads ELF files.
# multiboot - multiboot - This module provides various functions needed to support multi-booting systems.
# part_msdos - This module provides support for MS-DOS (MBR) partitions and partitioning tables.
# ext2 - This module provides support for EXT2 filesystems.
# sleep - This module allow to sleep a while.

sudo $GRUB_INSTALL --target=i386-pc --no-floppy --grub-mkdevicemap=$DEVICE_MAP_FILE --install-modules="sleep test echo vga normal elf multiboot part_msdos ext2" --locales="" --fonts="" --themes=no --modules="part_msdos" --boot-directory=`$READLINK -f $MOUNT_POINT` $LOOP_DEVICE -v
sudo cp -vf grub/grub.cfg $MOUNT_POINT/grub

sudo $UMOUNT $MOUNT_POINT
sudo $LOSETUP -d $LOOP_DEVICE
exit 0