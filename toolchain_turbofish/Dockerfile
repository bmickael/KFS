FROM debian:buster
# Set the working directory.
WORKDIR /build_toolchain

# Copy the file from your host to your current location.

RUN apt update
RUN yes | apt install binutils make gcc g++ nasm automake-1.15 libmpc-dev libmpfr-dev libgmp-dev wget patch tar xz-utils

COPY install_toolchain_turbofish.sh libc patch-binutils patch-coreutils patch-gcc .

CMD cd /build_toolchain && ./install_toolchain_turbofish.sh