# WIP: pgdagger

A metrics collector-aggregator for Postgres

## Usage


Start the stack:

```
docker compose up
```

Connect to Dagger's Postgres:

```psql

psql postgres://postgres:postgres@localhost:5433/postgres
```

Query Postgres:

```sql
select time, value, jsonb(labels) as labels
from prom_metric.pg_stat_activity_count psac
limit 1
```

<table>
<tr>
<td> time </td> <td> value </td> <td> labels </td>
</tr>
<tr>
<td> 2023-02-08 23:34:31.394+00 </td>
<td> 1 </td>
<td>

```json
{
  "job": "metrics_exporter",
  "group": "postgres",
  "state": "active",
  "server": "arbitrary_postgres:5432",
  "datname": "postgres",
  "monitor": "pgDagger-prometheus",
  "__name__": "pg_stat_activity_count",
  "instance": "metrics_exporter:9187"
}
```

</td>
</tr>
<tr>
