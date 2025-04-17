# :construction: Tembo Inference Server :construction:

This is a LLM hosting service for the Tembo Platform. It is built on top of [vLLM](https://github.com/vllm-project/vllm), and provides additional functionality for audit logging to enable tracking usage metrics hosted models.

## Architecture

- `Gateway` : an HTTP webserver that forwards requests to the inference server, and logs organization id, model id, and token counts to a Postgres database.
- `Inference Server`: a vLLM model server that hosts various LLMs
- `Postgres`: a standard Postgres database

```mermaid
graph LR
A[Gateway] --> |forward request| B[Inference Server]
A --> |audit log| D[(Postgres)]
```

## Development

Run Postgres and the Inference Service on CPU:

```bash
docker compose up postgres vllm-cpu -d
```

Run the Gateway:

```bash
make run
```

Send an example request to the gateway

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
    -H "X-TEMBO-ORG: MY-TEST-ORG" \
    -H "X-TEMBO-INSTANCE: MY-TEST-INSTANCE" \
    -H "Content-type: application/json" \
    -d '{
        "model":  "facebook/opt-125m",
        "messages": [{"role": "user", "content": "San Francisco is a..."}]}'
```

## Testing

Set up Postgres and Migrations.

```bash
make run-postgres
make run-migrations
```

Unit tests:

```bash
make unit-test
```

Integration tests:

```bash
make integration-test
```

## Database Maintenance

The Inference Gateway uses the `pg_partman` extension for table partitioning. For optimal performance, a maintenance procedure is scheduled to run daily.

### Optimized Maintenance

The system includes optimized maintenance functionality to ensure partition management doesn't impact performance:

1. Scheduled maintenance runs during off-peak hours (2 AM by default)
2. Resource-optimized execution with increased `maintenance_work_mem` during the operation
3. Parallel execution capabilities for multiple partition schemas
4. Performance monitoring and logging

### Maintenance Functions

- `optimized_run_maintenance()` - An optimized wrapper for pg_partman's maintenance
- `parallel_run_maintenance()` - Runs maintenance in parallel for multiple schemas
- `log_maintenance_performance()` - Tracks and logs maintenance performance metrics

### Performance Analysis

Monitor maintenance performance through the `maintenance_performance_analysis` view:

```sql
SELECT * FROM maintenance_performance_analysis;
```

If you need to manually trigger maintenance outside the scheduled time:

```sql
SELECT optimized_run_maintenance();
```

For parallel maintenance across multiple schemas:

```sql
SELECT parallel_run_maintenance(ARRAY['partman', 'other_schema'], 2);
```
