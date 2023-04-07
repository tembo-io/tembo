fmt:
	cargo +nightly fmt
	cargo clippy

run.local:
	PG_CONN_URL=postgresql://postgres:postgres@0.0.0.0:5432/postgres \
	RUST_BACKTRACE=1 \
	RUST_LOG=debug \
	CONTROL_PLANE_EVENTS_QUEUE=saas_queue \
	DATA_PLANE_EVENTS_QUEUE=data_plane_events \
	DATA_PLANE_BASEDOMAIN=coredb-development.com \
	cargo run

cluster.reset:
	kind delete cluster || true
	kind create cluster
	kubectl label namespace default safe-to-run-coredb-tests=true
	coredb-cli install --branch main
	helm install --create-namespace --namespace=traefik-v2 --values ./tests/traefik-values.yaml traefik traefik/traefik

run.test:
	echo "Running conductor with test configuration"
	PG_CONN_URL=postgresql://postgres:postgres@0.0.0.0:5432/postgres \
	RUST_BACKTRACE=1 \
	RUST_LOG=debug \
	CONTROL_PLANE_EVENTS_QUEUE=myqueue_control_plane \
	DATA_PLANE_EVENTS_QUEUE=myqueue_data_plane \
	DATA_PLANE_BASEDOMAIN=coredb-development.com \
	cargo run


setup.traefik:
	helm repo add traefik https://traefik.github.io/charts
	helm repo update
	kubectl create namespace traefik-v2
	helm install --namespace=traefik-v2 --values ./tests/traefik-values.yaml traefik traefik/traefik
