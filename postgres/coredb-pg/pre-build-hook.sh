#!/bin/bash
set -xe

# Run this command to populate the extensions directory for a local build

GIT_ROOT=$(git rev-parse --show-toplevel)
THIS_DIR="${GIT_ROOT}/postgres"
cd $THIS_DIR

cargo install --version 0.0.1-alpha.3 pg-trunk

mkdir extensions || true

trunk build --path ${GIT_ROOT}/extensions/pgmq/ --output-path ${THIS_DIR}/extensions
