KERNEL            := rust
TARGET            := "i686-turbofish"
PATH              := $(TOOLCHAIN_TURBOFISH)/cross/bin/:$(PATH)
TOOLCHAIN_SYSROOT := $(TOOLCHAIN_TURBOFISH)/sysroot
SHELL             := env PATH=$(PATH) /bin/bash

LIBC_AR           := $(TOOLCHAIN_SYSROOT)/usr/lib/libc.a
LIBC_HEADERS      := $(TOOLCHAIN_SYSROOT)/usr/include $(TOOLCHAIN_SYSROOT)/usr/include/sys
STANDARD_USER     := boilerman
