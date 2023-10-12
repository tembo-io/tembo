POSTGRES_PASSWORD := 'postgres'
DATABASE_URL := 'postgres://postgres:postgres@cp-pgmq-pg:5432'
CONDUCTOR_DATABASE_URL := 'postgresql://postgres:postgres@0.0.0.0:5431/postgres'
CLERK_SECRET_KEY := 'clerk-tembo-dev-secret-key'
RUST_LOG := 'info'

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
