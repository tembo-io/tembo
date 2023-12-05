# Run the entire Tembo platform locally

## Prerequisites

> Make sure you have these things installed before moving forward:

- [Docker](https://docs.docker.com/install/)
- [Just](https://github.com/casey/just)
- [Kind](https://kind.sigs.k8s.io/docs/user/quick-start/#installation)
- [Kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)
- [cargo-watch](`cargo install cargo-watch`)

## Running the platform

### Setup

- Before doing anything, go to https://dashboard.clerk.com/apps/app_2NxxGlmiAzGOwDTIzbObk4cxIOA/instances/ins_2NxxGsVFc3ddBxfVeJ4R55w1lTG/api-keys, copy the CLERK_SECRET_KEY, and add it into the justfile in the root.
- If ur on mac, you may need to stop any running postgres instances on port `5432` with `brew services stop postgresql`g
- Make sure you have your docker daemon running
- Confirm that ports `8080`, `6000`, and `8081` are free and nothing is running on them

### Run databases

```bash
just dbs-start
```

### Run + watch Tembo operator

> The below command will run + watch the tembo operator (if you make any changes inside of `./tembo-operator` it will re-build the binary). It will also create a kind cluster + worker in docker.

```bash
just watch-operator
```

### Run + watch conductor

> Does the same thing as the operator, but for conductor

```bash
just watch-conductor
```

### Run the control-plane

> The below command will run the control-plane's latest image(s) (cp-webserver + cp-service) on `quay.io` (won't watch for any local changes). It will also run in the background so if you want to see logs you will have to run `docker logs <your-container-id>`

```bash
just watch-control-plane
```

### Deploy an instance for testing

Grab a authorization token from dev using the `cloud.cdb-dev.com/generate-jwt` route in mahout and also an organization id from the Clerk development environment on https://dashboard.clerk.com. Then run the following `curl` command to hit the control-plane webserver on http://localhost:8080 to create a new db instance:

```bash
curl -X POST \
  http://localhost:8080/api/v1/orgs/<your-clerk-org-id>/instances \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN_HERE" \
  -d '{
    "cpu": 1,
    "environment": "dev",
    "instance_name": "test-db",
    "memory": "4Gi",
    "stack_type": "Standard",
    "storage": "10Gi"
}'
```

### Accessing your test db

- Check your kind cluster to see if the db pod is running with `kubectl get pods --all-namespaces` or `kubectl get pods` when ur in the correct namespace (it should be named something like `org-your-org-name-inst-test-db`)
- You can then `psql` into your new instance with `psql "postgres://postgres:$(kubectl get secrets -o json org-your-org-name-inst-test-db-connection | jq -r '.data.password' | base64 --decode)@org-your-org-name-inst-test-db.local.tembo-development.com:5432?sslmode=require`
