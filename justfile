NAME := "tembo-pod-init"
#VERSION := `git rev-parse HEAD`
CERT_MANAGER_VERSION := "v1.12.1"
SEMVER_VERSION := `grep version Cargo.toml | awk -F"\"" '{print $2}' | head -n 1`
NAMESPACE := "default"
KUBE_VERSION := "1.26"
RUST_LOG := "debug"
LOG_LEVEL := "debug"

default:
  @just --list --unsorted --color=always | rg -v "    default"

# delete kind
delete-kind:
	kind delete cluster && sleep 5

# start kind
start-kind:
  delete-kind
  kind create cluster
  sleep 10
  kubectl wait pods --for=condition=Ready --timeout=300s --all --all-namespaces

# install cert-manager
cert-manager:
  kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/{{CERT_MANAGER_VERSION}}/cert-manager.yaml
  sleep 7
  kubectl wait pods --for=condition=Ready --timeout=300s --all --all-namespaces

# run
run:
  LOG_LEVEL={{LOG_LEVEL}} cargo run

# run cargo watch
watch:
  LOG_LEVEL={{LOG_LEVEL}} cargo watch -x 'run'

# format with nightly rustfmt
fmt:
  cargo +nightly fmt

# run integration testing
test:
  RUST_LOG={{RUST_LOG}} cargo test -- --ignored --nocapture
