#!/bin/bash

# Steven just ran this manually from local after signing into Quay
set -xe
docker buildx imagetools create -t quay.io/coredb/rust:1.77.0 rust:1.77.0
docker buildx imagetools create -t quay.io/coredb/rust:1.77.0-slim rust:1.77.0-slim
