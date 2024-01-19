FROM quay.io/tembo/tembo-local:latest

# Trunk Installs from stack_type


RUN trunk install --version 1.10.0 pg_stat_statements



# Trunk Installs from tembo.toml


# Optional:
# Specify extra Postgres configurations by copying into this directory
COPY postgres.conf $PGDATA/extra-configs