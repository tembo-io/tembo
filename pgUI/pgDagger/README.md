# WIP: pgdagger

A metrics collector-aggregator for Postgres

## Usage

Start the stack:

```bash
docker compose up
```

## Query via REST

Postgres data types are documented [here](https://postgrest.org/en/stable/how-tos/working-with-postgresql-data-types.html#timestamps)

```bash
curl http://localhost:3001/prom_pg_stat_database_sessions_killed?time=gte.2023-02-14+21:40:15
```

```json
[
 {"time":"2023-02-14T21:40:55.18+00:00","value":15,"labels":{"job": "metrics_exporter", "group": "postgres", "server": "arbitrary_postgres:5433", "monitor": "pgDagger-prometheus", "__name__": "pg_stat_bgwriter_checkpoints_timed_total", "instance": "metrics_exporter:9187"}}, 
 {"time":"2023-02-14T21:41:05.18+00:00","value":15,"labels":{"job": "metrics_exporter", "group": "postgres", "server": "arbitrary_postgres:5433", "monitor": "pgDagger-prometheus", "__name__": "pg_stat_bgwriter_checkpoints_timed_total", "instance": "metrics_exporter:9187"}}, 
 {"time":"2023-02-14T21:41:15.18+00:00","value":15,"labels":{"job": "metrics_exporter", "group": "postgres", "server": "arbitrary_postgres:5433", "monitor": "pgDagger-prometheus", "__name__": "pg_stat_bgwriter_checkpoints_timed_total", "instance": "metrics_exporter:9187"}}, 
 {"time":"2023-02-14T21:41:20.18+00:00","value":15,"labels":{"job": "metrics_exporter", "group": "postgres", "server": "arbitrary_postgres:5433", "monitor": "pgDagger-prometheus", "__name__": "pg_stat_bgwriter_checkpoints_timed_total", "instance": "metrics_exporter:9187"}}, 

]
```

Connect to the UI's backend postgres:

```psql
psql postgres://postgres:postgres@localhost:5432/postgres
```

Query Postgres:

```sql
select time, value, jsonb(labels) as labels
from prom_metric.pg_locks_count psac
limit 1;
```

<table>
<tr>
<td> time </td> <td> value </td> <td> labels </td>
</tr>
<tr>
<td> 2023-02-08 23:34:31.394+00 </td>
<td> 0 </td>
<td>

```json
{
  "job": "metrics_exporter",
  "mode": "accessexclusivelock",
  "group": "postgres",
  "server": "arbitrary_postgres:5433",
  "datname": "postgres",
  "monitor": "pgDagger-prometheus",
  "__name__": "pg_locks_count",
  "instance": "metrics_exporter:9187"
}
```

</td>
</tr>
<tr>
