#!/bin/bash

# directory of this script
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)

set -xe

# Create new cluster
kind delete cluster || true
kind create cluster --image=kindest/node:v1.28.9

# Label the default namespace as safe to run tests
kubectl label namespace default safe-to-run-coredb-tests=true

# patch storageclass to allow volume expansion
kubectl patch storageclass standard -p '{"allowVolumeExpansion": true}'

# Install CoreDB CRDs
cd $SCRIPT_DIR
cd ..
cargo run --bin crdgen | kubectl apply -f -
