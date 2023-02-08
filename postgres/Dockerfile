FROM ubuntu:22.04

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
        postgresql-15 \
        postgresql-15-postgis-3 \
        postgresql-15-cron \
        postgresql-15-repack \
        postgresql-15-pgaudit \
        && rm -rf /var/lib/apt/lists/*

COPY extensions/ /extensions

RUN for extension in $(ls /extensions); do dpkg -i /extensions/${extension}; done

COPY docker-entrypoint.sh /usr/local/bin/
ENTRYPOINT ["docker-entrypoint.sh"]

USER postgres
CMD ["postgres"]
