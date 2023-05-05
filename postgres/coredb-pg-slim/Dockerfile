FROM quay.io/coredb/ubuntu:22.04 as builder

ARG WALG_VER=2.0.1
ARG TRUNK_VER=0.4.6

RUN apt-get update && apt-get install -y curl pkg-config libssl-dev build-essential libclang-dev clang software-properties-common
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
RUN /root/.cargo/bin/cargo install --version $TRUNK_VER pg-trunk

# Grab wal-g binary
RUN curl -L -o /tmp/wal-g "https://github.com/wal-g/wal-g/releases/download/v$WALG_VER/wal-g-pg-ubuntu-20.04-amd64"

FROM quay.io/coredb/ubuntu:22.04

ARG PG_PARTMAN_VER=4.7.4
ARG PGMQ_VER=0.4.2
ARG PG_VECTOR_VER=0.4.1

COPY --from=builder /root/.cargo/bin/trunk /usr/bin/trunk
COPY --from=builder /tmp/wal-g /usr/bin/wal-g

ARG DEBIAN_FRONTEND=noninteractive
ENV TZ=Etc/UTC

# Set the postgres user's permissions
RUN set -eux; \
	groupadd -r postgres --gid=999; \
	useradd -r -g postgres --uid=999 --home-dir=/var/lib/postgresql --shell=/bin/bash postgres; \
	mkdir -p /var/lib/postgresql; \
	chown -R postgres:postgres /var/lib/postgresql

# Installs the postgres APT repository
# https://wiki.postgresql.org/wiki/Apt
RUN apt-get update && apt-get install -y \
        curl ca-certificates gnupg lsb-release \
        && rm -rf /var/lib/apt/lists/*
RUN curl https://www.postgresql.org/media/keys/ACCC4CF8.asc | gpg --dearmor | tee /etc/apt/trusted.gpg.d/apt.postgresql.org.gpg > /dev/null
RUN echo "deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list

STOPSIGNAL SIGINT

ENV PGDATA /var/lib/postgresql/data
ENV PG_MAJOR 15
ENV PATH $PATH:/usr/lib/postgresql/$PG_MAJOR/bin

RUN set -eux; \
	apt-get update; apt-get install -y --no-install-recommends locales; rm -rf /var/lib/apt/lists/*; \
	localedef -i en_US -c -f UTF-8 -A /usr/share/locale/locale.alias en_US.UTF-8
ENV LANG en_US.utf8

RUN mkdir /docker-entrypoint-initdb.d

# Install postgres and some extensions
RUN apt-get update && apt-get install -y \
        # build deps for pg_partman
        build-essential \
        git \
        postgresql-server-dev-${PG_MAJOR} \
        # pg_stat_statement's package
        postgresql-contrib \
        # postgresql server
        postgresql-${PG_MAJOR} \
        # extensions
        postgresql-${PG_MAJOR}-postgis-3 \
        postgresql-${PG_MAJOR}-cron \
        postgresql-${PG_MAJOR}-repack \
        postgresql-${PG_MAJOR}-pgaudit \
        && rm -rf /var/lib/apt/lists/*

COPY ./postgresql.conf /usr/share/postgresql/${PG_MAJOR}/postgresql.conf.sample

WORKDIR /git
RUN git clone --branch "v${PG_VECTOR_VER}" https://github.com/pgvector/pgvector.git
WORKDIR /git/pgvector
RUN set -xe; \
      make; \
      make install;

WORKDIR / 
RUN rm -rf /git
  
RUN trunk install --version ${PG_PARTMAN_VER} pg_partman

# cache extensions and shared libraries
RUN mkdir /tmp/pg_sharedir && \
        mkdir /tmp/pg_pkglibdir && \
        cp -r $(pg_config --sharedir)/* /tmp/pg_sharedir && \
        cp -r $(pg_config --pkglibdir)/* /tmp/pg_pkglibdir

COPY docker-entrypoint.sh /usr/local/bin/
ENTRYPOINT ["docker-entrypoint.sh"]

USER postgres
CMD ["postgres"]
