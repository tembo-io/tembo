# Postgres Extension to Export Promtheus Metrics

Proof-of-concept


```bash
echo "shared_preload_libraries = 'prometheus_exporter.so'" >> ~/.pgx/data-14/postgresql.conf
```