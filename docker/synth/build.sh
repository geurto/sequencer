#!/bin/bash

export DOCKER_BUILDKIT=1

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

(cd "$SCRIPT_DIR"/../.. && \
docker build \
-f ./docker/synth/Dockerfile \
-t geurto/generative-sequencer:synth \
./docker/synth/)
