FROM rust:1.70-bookworm as builder

ARG TRUNK_VER=0.6.1

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL sparse 
RUN cargo install --version $TRUNK_VER pg-trunk

FROM quay.io/coredb/coredb-pg-base:latest
USER root

# Install trunk
COPY --from=builder /usr/local/cargo/bin/trunk /usr/bin/trunk
COPY ./requirements.txt .

# Install barman-cloud
RUN set -xe; \
	apt-get update; \
	apt-get install -y --no-install-recommends \
		python3-pip \
		python3-psycopg2 \
		python3-setuptools \
	; \
	pip3 install --upgrade pip; \
# TODO: Remove --no-deps once https://github.com/pypa/pip/issues/9644 is solved
	pip3 install --no-deps -r requirements.txt; \
  apt-get autoremove -y; \
  apt-get clean; \
	rm -rf /var/lib/apt/lists/*;

# Revert the postgres user to id 26
RUN usermod -u 26 postgres
USER 26
