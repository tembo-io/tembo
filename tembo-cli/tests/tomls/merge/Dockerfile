FROM quay.io/tembo/tembo-local:latest

# Trunk Installs from stack_type


RUN trunk install --version 1.10.0 pg_stat_statements



# Trunk Installs from tembo.toml


RUN trunk install --version 0.33.3 pgmq



RUN trunk install --version 4.7.4 pg_partman



RUN trunk install --version 0.1.4 pg_jsonschema



# Optional:
# Specify extra Postgres configurations by copying into this directory
COPY postgres.conf $PGDATA/extra-configs