# Postgres Extension to Export Promtheus Metrics

Proof-of-concept


### Developer Setup

If you have not already ran this on your machine, run it now:
```bash
cargo pgx init
```

Enable autoloading of the extension. Note, `data-14` refers to the configuration for Postgres version 14. If you are using a different version, replace `data-14` with the appropriate configuration name, e.g. `data-13` for Postgres 13.

```bash
echo "shared_preload_libraries = 'prometheus_exporter.so'" >> ~/.pgx/data-14/postgresql.conf
```

Compile and run the extension on Postgres 14. This will start Postgres on port 28814. A metrics server will be running on `localhost:8080`.

```bash
cargo pgx run pg14
```

Scrape Prometheus metrics:

```bash
curl localhost:8080/metrics
```
```
# HELP pg_uptime Postgres server uptime.
# TYPE pg_uptime gauge
pg_uptime{} 57
# EOF
```


### Testing

Runs tests:

```bash
cargo pgx test
```
