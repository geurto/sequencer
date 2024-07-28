#!/bin/bash

ARCH=arm64

export DOCKER_BUILDKIT=1

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

docker pull --platform=linux/arm64/v8 arm64v8/rust

(cd "${SCRIPT_DIR}" && ./qemu.sh)

(cd "$SCRIPT_DIR"/../../.. && \
docker build \
-f ./docker/sequencer/Dockerfile \
--platform linux/arm64/v8 \
--build-arg BASE_IMAGE=arm64v8/rust:latest \
-t geurto/generative-sequencer:sequencer-${ARCH} \
.)
