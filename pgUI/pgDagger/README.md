# WIP: pgdagger

A metrics collector-aggregator for Postgres

## Usage

Start the stack:

```bash
docker compose up
```

## Query via REST

```bash
curl http://localhost:3001/pg_locks_count
```

```json
[
 {"time":"2023-02-14T17:55:09.634+00:00","value":1,"series_id":743},
 {"time":"2023-02-14T17:55:09.634+00:00","value":0,"series_id":745},
 {"time":"2023-02-14T17:55:09.634+00:00","value":0,"series_id":746},
 {"time":"2023-02-14T17:55:09.634+00:00","value":0,"series_id":748}
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
