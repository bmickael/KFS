#!/bin/bash
source ./env.sh

RAM_AMOUNT=512

set -e
qemu-system-x86_64 -m $RAM_AMOUNT -vga std -enable-kvm -cpu IvyBridge -drive format=raw,file=$IMG_DISK -rtc base=localtime,clock=rt,driftfix=none
exit 0

# exec_sata:
#	qemu-system-x86_64 -m $(RAM_AMOUNT) -vga std -enable-kvm -cpu IvyBridge \
#	-drive file=$(IMG_DISK),if=none,id=toto,format=raw \
#	-device ich9-ahci,id=ahci \
#	-device ide-drive,drive=toto,bus=ahci.0 \