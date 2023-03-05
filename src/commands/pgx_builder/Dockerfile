ARG PG_VERSION=15
ARG PGX_VERSION

FROM quay.io/coredb/pgx-builder:pg${PG_VERSION}-pgx${PGX_VERSION}

ARG PG_VERSION=15
ARG PGX_VERSION

COPY --chown=postgres:postgres . /app
WORKDIR /app

ARG EXTENSION_NAME
ARG EXTENSION_VERSION

RUN cargo pgx package

ENV OUTPUT_DIR="/app/trunk-output"
ENV BUILD_DIR="/app/target/release/${EXTENSION_NAME}-pg${PG_VERSION}/"
RUN mkdir -p ${OUTPUT_DIR}

RUN cp -r ${BUILD_DIR}$(/usr/bin/pg_config --pkglibdir)/*.so $OUTPUT_DIR && \
    cp -r ${BUILD_DIR}$(/usr/bin/pg_config --sharedir)/extension/* $OUTPUT_DIR