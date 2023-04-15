ARG UBUNTU_VERSION=22.04
FROM quay.io/coredb/ubuntu:${UBUNTU_VERSION}

ARG DEBIAN_FRONTEND=noninteractive
ENV TZ=Etc/UTC
ARG PG_VERSION=15

RUN apt-get update && \
    apt-get install -y \
      curl unzip gpg wget software-properties-common flex \
    && curl https://www.postgresql.org/media/keys/ACCC4CF8.asc | gpg --dearmor | tee /etc/apt/trusted.gpg.d/apt.postgresql.org.gpg > /dev/null \
    && echo "deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list \
    && apt-get update  \
    && apt-get install -y \
      postgresql-${PG_VERSION} postgresql-server-dev-${PG_VERSION} \
      pkg-config libssl-dev build-essential libclang-dev clang && \
    rm -rf /var/lib/apt/lists/*

USER postgres
WORKDIR /app
