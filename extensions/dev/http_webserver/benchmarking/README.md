# Benchmarking webserver interface to postgres

Three implementations of an http webserver which exposes data in a Postgres table.
1. webserver implemented as a Postgres extension written in Rust
2. stand-alone webserver implemented in Rust
3. stand-alone webserver implemented in Python

# Start the postgres database and extension
If not already running, start the postgres database and extension out of the coredb/extensions/dev/http_webserver directory. See that [README](../../http_webserver/README.md) for setup instruction. 


# Update .env
Update the .env file with the correct database connection information.

# Run stand-alone rust webserver

```bash
cargo run --release
```

# Run stand-alone python webserver

```bash
poetry install
poetry run gunicorn app:app -k uvicorn.workers.UvicornWorker --bind 0.0.0.0:8000
```

# Loadtest against the stand-alone webserver

```bash
poetry run locust \
    -f ./locustfile.py \
    --host=http://0.0.0.0:8000/read \
    -u 100 -r 100 -t 120s --stop-timeout 1 --headless --csv=stand-alone-actix
```


# Loadtest against the extension webserver

```bash
poetry run locust \
    -f ./locustfile.py \
    --host=http://0.0.0.0:8080/ \
    -u 100 -r 100 -t 120s --stop-timeout 1 --headless --csv=pgext-tcplistener
```


# Loadtest against the python fastapi
```bash
poetry run locust \
    -f ./locustfile.py \
    --host=http://0.0.0.0:8000/ \
    -u 100 -r 100 -t 120s --stop-timeout 1 --headless --csv=fastapi
```


# Visualize Results

After all benchmarks have compelted, run the command below. Benchmark plot will be written to `./benchmark.png`

```bash
poetry run python plot.py
```
