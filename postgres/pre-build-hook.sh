#!/bin/bash
set -xe

# Run this command to populate the extensions directory for a local build

GIT_ROOT=$(git rev-parse --show-toplevel)
cd ${GIT_ROOT}/postgres

cd ${GIT_ROOT}/extensions/pgmq
/bin/bash build-extension.sh

cd ${GIT_ROOT}/postgres
mkdir extensions || true

cp ${GIT_ROOT}/extensions/pgmq/*.deb ./extensions/
