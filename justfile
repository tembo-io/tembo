POSTGRES_PASSWORD := 'postgres'
DATABASE_URL := 'postgres://postgres:postgres@cp-pgmq-pg:5432'
CONDUCTOR_DATABASE_URL := 'postgresql://postgres:postgres@0.0.0.0:5431/postgres'
CLERK_SECRET_KEY := 'clerk-tembo-dev-secret-key'
RUST_LOG := 'info'
KUBE_VERSION := '1.31'
CERT_MANAGER_VERSION := '1.17.1'

watch-operator:
    docker container rm kind-control-plane --force || true
    docker container rm kind-worker --force || true
    just -f ./tembo-operator/justfile start-kind
    DATA_PLANE_BASEDOMAIN=local.tembo-development.com \
    ENABLE_BACKUP=false RUST_LOG=info,kube=info,controller=info \
    PORT=6000 \
    cargo watch --workdir ./tembo-operator -x 'run'

watch-conductor:
    POSTGRES_QUEUE_CONNECTION={{CONDUCTOR_DATABASE_URL}} \
    PORT=8000 \
    RUST_BACKTRACE=1 \
    RUST_LOG={{RUST_LOG}} \
    CONTROL_PLANE_EVENTS_QUEUE=saas_queue \
    DATA_PLANE_EVENTS_QUEUE=data_plane_events \
    METRICS_EVENTS_QUEUE=metrics_events \
    DATA_PLANE_BASEDOMAIN=local.tembo-development.com \
    CF_TEMPLATE_BUCKET=cdb-plat-use1-dev-eks-data-1-conductor-cf-templates \
    BACKUP_ARCHIVE_BUCKET=cdb-plat-use1-dev-instance-backups \
    IS_CLOUD_FORMATION=false \
    cargo watch --workdir ./conductor -x run

run-control-plane:
    docker run -d -p 8080:8080 --network temboDevSuite --env POSTGRES_CONNECTION='{{DATABASE_URL}}' --env POSTGRES_QUEUE_CONNECTION='{{DATABASE_URL}}' --env CLERK_SECRET_KEY='{{CLERK_SECRET_KEY}}' -it --entrypoint /usr/local/bin/cp-webserver --rm quay.io/coredb/cp-service
    docker run -d -p 8081:8081 --network temboDevSuite --env POSTGRES_CONNECTION='{{DATABASE_URL}}' --env POSTGRES_QUEUE_CONNECTION='{{DATABASE_URL}}' --env CLERK_SECRET_KEY='{{CLERK_SECRET_KEY}}' -it --entrypoint /usr/local/bin/cp-service --rm quay.io/coredb/cp-service

run-dbs:
    docker network create temboDevSuite || true
    docker rm --force cp-pgmq-pg || true
    docker run --network temboDevSuite -d --name cp-pgmq-pg -e POSTGRES_PASSWORD={{POSTGRES_PASSWORD}} -p 5431:5432 quay.io/tembo/pgmq-pg:v0.14.2

dbs-cleanup:
	docker stop cp-pgmq-pg

dbs-start:
    just run-dbs

helm-lint:
  ct lint --config ct.yaml

helm-repo:
	helm repo add cnpg https://cloudnative-pg.github.io/charts

cert-manager:
	helm repo add jetstack https://charts.jetstack.io
	helm repo update
	helm upgrade --install cert-manager jetstack/cert-manager --version={{CERT_MANAGER_VERSION}} --namespace cert-manager --create-namespace --values=tembo-operator/testdata/cert-manager.yaml
	sleep 5
	kubectl wait --timeout=120s --for=condition=ready pod -l app.kubernetes.io/instance=cert-manager -n cert-manager

start-kind:
  kind delete cluster
  kind create cluster --config testdata/kind-{{KUBE_VERSION}}.yaml
  sleep 5
  kubectl wait pods --for=condition=Ready --timeout=300s --all --all-namespaces
  just cert-manager
