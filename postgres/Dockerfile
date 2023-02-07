FROM postgres:15.1

RUN apt-get update && apt-get install -y \
        postgresql-15-postgis-3 \
        postgresql-15-cron \
        postgresql-15-repack \
        postgresql-15-pgaudit

COPY postgresql-pgmq-15_0.0.1-ubuntu22.04-arm64.deb .

RUN apt install ./postgresql-pgmq-15_0.0.1-ubuntu22.04-arm64.deb
