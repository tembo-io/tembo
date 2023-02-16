#!/bin/bash

# directory of this script
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

set -xe

# Create new cluster
kind delete cluster || true
kind create cluster

# Label the default namespace as safe to run tests
kubectl label namespace default safe-to-run-coredb-tests=true

# Install CoreDB CRDs
cd $SCRIPT_DIR
cd ..
cargo run --bin crdgen | kubectl apply -f -

# Install prometheus operator
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update
helm upgrade monitoring \
  --install \
  --values=$SCRIPT_DIR/kube-prometheus-stack-values.yaml \
  prometheus-community/kube-prometheus-stack
