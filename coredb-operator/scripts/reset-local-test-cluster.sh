#!/bin/bash
set -xe

# Create new cluster
kind delete cluster || true
kind create cluster

# Label the default namespace as safe to run tests
kubectl label namespace default safe-to-run-coredb-tests=true

# Install CoreDB CRDs
cargo run --bin crdgen | kubectl apply -f -

# Install prometheus operator
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update
helm upgrade monitoring \
  --install \
  --set kubeStateMetrics.enabled=false \
  --set nodeExporter.enabled=false \
  --set grafana.enabled=false \
  --set alertmanager.enabled=false \
  prometheus-community/kube-prometheus-stack \
  &
