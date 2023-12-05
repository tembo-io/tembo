#!/bin/bash

# directory of this script
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

set -xe

# Create new cluster
kind delete cluster || true
kind create cluster

# Label the default namespace as safe to run tests
kubectl label namespace default safe-to-run-coredb-tests=true

# patch storageclass to allow volume expansion
kubectl patch storageclass standard -p '{"allowVolumeExpansion": true}'

# Wait for kind cluster to be running
kubectl wait pods --for=condition=Ready --timeout=300s --all --all-namespaces

sleep 10

# Install CoreDB CRDs
cd $SCRIPT_DIR
cd ..
make setup.traefik
coredb-cli install --branch main

# Wait for the coredb operator to come online
kubectl wait --timeout=60s --for=condition=ready pod -l app=coredb-controller -n coredb-operator
