FROM quay.io/tembo/tembo-pg-cnpg:latest

USER root

# Install GDAL (which includes ogr2ogr)
RUN apt-get update && \
    apt-get install -y gdal-bin && \
    rm -rf /var/lib/apt/lists/*

RUN chown -R postgres:postgres $PGDATA && \
    chmod -R 0700 $PGDATA
ENV PGDATA /var/lib/postgresql/data2
RUN mkdir -p $PGDATA
RUN chown -R postgres:postgres $PGDATA && \
    chmod -R 0700 $PGDATA
USER postgres
RUN pg_ctl init
# Set permissive authentication
RUN echo "host all all 0.0.0.0/0 trust" >> ${PGDATA}/pg_hba.conf
COPY postgresql.conf ${PGDATA}/postgresql.conf
COPY Elephant_Research_Ivory_Coast_Collar_5990 /Users/evanstanton/Documents/tembo/qgis/dataset_movebank/Elephant_Research_Ivory_Coast_Collar_5990


