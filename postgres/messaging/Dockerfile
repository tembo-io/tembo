FROM quay.io/coredb/coredb-pg-slim:f57e860

USER root

RUN trunk install pg_partman
RUN trunk install pgmq

COPY ./postgresql.conf /usr/share/postgresql/15/postgresql.conf.sample

USER postgres
CMD ["postgres"]
