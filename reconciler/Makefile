run.local:
	PG_CONN_URL=postgresql://postgres:postgres@0.0.0.0:5432/postgres \
	RUST_BACKTRACE=1 \
	RUST_LOG=debug \
	CONTROL_PLANE_EVENTS_QUEUE=saas_queue \
	DATA_PLANE_EVENTS_QUEUE=data_plane_events \
	cargo run

cluster.reset:
	kind delete cluster || true
	kind create cluster
	kubectl label namespace default safe-to-run-coredb-tests=true
	coredb-cli install --branch main
	helm install --create-namespace --namespace=traefik-v2 --values ./tests/traefik-values.yaml traefik traefik/traefik

run.test:
	echo "Running reconciler with test configuration"
	PG_CONN_URL=postgresql://postgres:postgres@0.0.0.0:5432/postgres \
	RUST_BACKTRACE=1 \
	RUST_LOG=debug \
	CONTROL_PLANE_EVENTS_QUEUE=myqueue_control_plane \
	DATA_PLANE_EVENTS_QUEUE=myqueue_data_plane \
	cargo run