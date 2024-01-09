FROM quay.io/tembo/tembo-local:latest

# Trunk Installs from stack_type


RUN trunk install --version 1.10.0 pg_stat_statements



RUN trunk install --version 1.0.0 hydra_columnar



RUN trunk install --version 4.7.3 pg_partman



RUN trunk install --version 1.5.2 pg_cron



RUN trunk install --version 1.1.0 postgres_fdw



RUN trunk install --version 1.0.0 redis_fdw



RUN trunk install --version 0.2.0 wrappers



RUN trunk install --version 2.5.0 multicorn



# Trunk Installs from tembo.toml


RUN trunk install clerk_fdw



# Optional:
# Specify extra Postgres configurations by copying into this directory
COPY postgres.conf $PGDATA/extra-configs