#!/bin/bash

cp -r ../libc .
docker build --tag turbofish:0.1 . 

docker run \
  -it \
  --mount type=bind,source=/toolchain_turbofish,target=/toolchain_turbofish  turbofish:0.1
