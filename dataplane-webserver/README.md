# Data Plane Webserver

Docs:
    https://api.data-1.use1.coredb.io/swagger-ui/?urls

Public Routes:
    `/health/ready` : readiness probe
    `/health/lively` : liveliness probe

## Testing

- Connect to VPN
- Export prometheus URL
```
export PROMETHEUS_URL=https://prometheus-data-1.use1.dev.plat.cdb-svc.com
```
- Use cargo to test
```
cargo test
```
