#!/bin/bash
source ./env.sh

TARGET_DIR=`readlink -f $SYSTEM_ROOT`/bin
BUILD_DIR="build_dir/build_coreutils"
PATCH="patch-coreutils"

set -e
mkdir -pv $BUILD_DIR
cp patch/$PATCH $BUILD_DIR
cd $BUILD_DIR
rm -rf coreutils-5.0
wget -c 'https://ftp.gnu.org/gnu/coreutils/coreutils-5.0.tar.bz2'
tar -xf 'coreutils-5.0.tar.bz2'
patch -p0 < $PATCH
cd coreutils-5.0
# cp ../../patch-coreutils-configure .
mkdir build
cd build
CFLAGS="-O3 -Wl,--gc-sections -fno-omit-frame-pointer" ../configure --build="`gcc -dumpmachine`" --host=$TARGET
make -C lib
make -C src basename
make -C src chgrp
# make -C src chroot
make -C src chown
make -C src cksum
make -C src comm
make -C src cp
make -C src csplit
# make -C src cut
make -C src dir
make -C src dircolors
make -C src dirname
# make -C src du
make -C src env
# make -C src expand
make -C src expr
make -C src factor
make -C src false
make -C src fmt
make -C src fold
# make -C src ginstall
make -C src groups
make -C src head
# make -C src hostid
make -C src id
make -C src join
make -C src link
# make -C src logname
make -C src md5sum
make -C src mkfifo
make -C src mknod
make -C src nice
# make -C src nl
make -C src nohup
# make -C src od
# make -C src paste
make -C src pathchk
# make -C src pinky
# make -C src pr
make -C src printenv
# make -C src printf
make -C src ptx
make -C src readlink
make -C src seq
make -C src sha1sum
# make -C src shred
# make -C src sort
make -C src split
# make -C src stat
# make -C src stty
# make -C src su
make -C src sum
make -C src sync
# make -C src tac
# make -C src tail
# make -C src tee
make -C src test
make -C src touch
make -C src tr
make -C src true
make -C src tsort
# make -C src tty
# make -C src uname
# make -C src unexpand
make -C src uniq
make -C src unlink
# make -C src uptime
# make -C src users
make -C src vdir
make -C src wc
# make -C src who
make -C src whoami
make -C src yes
# MANDATORY PART
echo "**************************************"
echo "* Compiling some mandatory executables *"
echo "**************************************"
make -C src cat
make -C src chmod
make -C src cp
make -C src date
make -C src dd
make -C src df
make -C src echo
make -C src hostname
make -C src kill
make -C src ln
make -C src ls
make -C src mkdir
make -C src mv
# (ps is not in coreutils)
make -C src pwd
make -C src rm
make -C src rmdir
make -C src sleep
echo "**************************************"

cp -v src/basename $TARGET_DIR
cp -v src/chgrp $TARGET_DIR
cp -v src/chown $TARGET_DIR
# cp -v src/chroot $TARGET_DIR
cp -v src/cksum $TARGET_DIR
cp -v src/comm $TARGET_DIR
cp -v src/csplit $TARGET_DIR
# cp -v src/cut $TARGET_DIR
cp -v src/dir $TARGET_DIR
cp -v src/dircolors $TARGET_DIR
cp -v src/dirname $TARGET_DIR
# cp -v src/du $TARGET_DIR
cp -v src/env $TARGET_DIR
# cp -v src/expand $TARGET_DIR
cp -v src/expr $TARGET_DIR
cp -v src/factor $TARGET_DIR
cp -v src/false $TARGET_DIR
cp -v src/fmt $TARGET_DIR
cp -v src/fold $TARGET_DIR
# cp -v src/ginstall $TARGET_DIR
cp -v src/groups $TARGET_DIR
cp -v src/head $TARGET_DIR
# cp -v src/hostid $TARGET_DIR
cp -v src/id $TARGET_DIR
cp -v src/join $TARGET_DIR
cp -v src/link $TARGET_DIR
# cp -v src/logname $TARGET_DIR
cp -v src/md5sum $TARGET_DIR
cp -v src/mkfifo $TARGET_DIR
cp -v src/mknod $TARGET_DIR
cp -v src/nice $TARGET_DIR
# cp -v src/nl $TARGET_DIR
cp -v src/nohup $TARGET_DIR
# cp -v src/od $TARGET_DIR
# scp -v src/paste $TARGET_DIR
cp -v src/pathchk $TARGET_DIR
# cp -v src/pinky $TARGET_DIR
# cp -v src/pr $TARGET_DIR
cp -v src/printenv $TARGET_DIR
# cp -v src/printf $TARGET_DIR
cp -v src/ptx $TARGET_DIR
cp -v src/readlink $TARGET_DIR
cp -v src/seq $TARGET_DIR
cp -v src/sha1sum $TARGET_DIR
# cp -v src/shred $TARGET_DIR
# cp -v src/sort $TARGET_DIR
cp -v src/split $TARGET_DIR
# cp -v src/stat $TARGET_DIR
# cp -v src/stty $TARGET_DIR
# cp -v src/su $TARGET_DIR
cp -v src/sum $TARGET_DIR
cp -v src/sync $TARGET_DIR
# cp -v src/tac $TARGET_DIR
# cp -v src/tail $TARGET_DIR
# cp -v src/tee $TARGET_DIR
cp -v src/test $TARGET_DIR
cp -v src/touch $TARGET_DIR
cp -v src/tr $TARGET_DIR
cp -v src/true $TARGET_DIR
cp -v src/tsort $TARGET_DIR
# cp -v src/tty $TARGET_DIR
# cp -v src/uname $TARGET_DIR
# cp -v src/unexpand $TARGET_DIR
cp -v src/uniq $TARGET_DIR
cp -v src/unlink $TARGET_DIR
# cp -v src/uptime $TARGET_DIR
# cp -v src/users $TARGET_DIR
cp -v src/vdir $TARGET_DIR
cp -v src/wc $TARGET_DIR
# cp -v src/who $TARGET_DIR
cp -v src/whoami $TARGET_DIR
cp -v src/yes $TARGET_DIR
# MANDATORY PART
echo "**************************************"
echo "* Copying some mandatory executables *"
echo "**************************************"
cp -v src/cat $TARGET_DIR
cp -v src/chmod $TARGET_DIR
cp -v src/cp $TARGET_DIR
cp -v src/date $TARGET_DIR
cp -v src/dd $TARGET_DIR
cp -v src/df $TARGET_DIR
cp -v src/echo $TARGET_DIR
cp -v src/hostname $TARGET_DIR
cp -v src/kill $TARGET_DIR
cp -v src/ln $TARGET_DIR
cp -v src/ls $TARGET_DIR
cp -v src/mkdir $TARGET_DIR
cp -v src/mv $TARGET_DIR
# (ps is not in coreutils)
cp -v src/pwd $TARGET_DIR
cp -v src/rm $TARGET_DIR
cp -v src/rmdir $TARGET_DIR
cp -v src/sleep $TARGET_DIR
echo "**************************************"
