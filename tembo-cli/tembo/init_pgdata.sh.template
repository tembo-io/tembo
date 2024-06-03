#!/bin/bash
set -e

# Initialize the database if PGDATA is empty
if [ ! -s "$PGDATA/PG_VERSION" ]; then
    echo "Initializing database in $PGDATA"
    initdb -D "$PGDATA" > /var/log/initdb.log 2>&1
else
    echo "Database already initialized in $PGDATA"
fi

# Start PostgreSQL
exec postgres
