FROM quay.io/tembo/tembo-local:latest

RUN trunk install pg_partman

COPY custom.conf $PGDATA/extra-configs

COPY startup.sql $PGDATA/startup-scripts
