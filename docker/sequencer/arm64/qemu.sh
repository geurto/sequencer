#!/bin/bash

# Make the docker builds run in QEMU environment
echo "****************************************************************************************"
echo "Setting up QEMU environment.."
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes
echo "****************************************************************************************"
