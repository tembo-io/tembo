# Tembo Stacks

Tembo Stacks are pre-built use-case-specific Postgres deployments which are optimized and tuned to serve a specific workload. They are a replacement for other databases which you consider because you don’t know how to solve that problem with Postgres.

## Why Stacks?

Adopting a new database adds significant complexity and costs to an engineering organization. Organizations spend a huge amount of time evaluating, benchmarking or migrating databases and setting up complicated pipelines keeping those databases in sync.

Most of these use cases can be served by Postgres, thanks to its stability, feature completeness and extensibility. However, optimizing Postgres for each use case is a non-trivial task and requires domain expertise, use case understanding and deep Postgres expertise, making it hard for most developers to adopt this.

Tembo Stacks solve that problem by providing pre-built, use case optimized Postgres deployments.

A tembo stack is a pre-built, use case specific Postgres deployment which enables you to quickly deploy specialized data services that can replace external, non-Postgres data services. They help you avoid the pains associated with adopting, operationalizing, optimizing and managing new databases.

|Name|Replacement for|
|----|---------------|
|[OLTP](./src/stacks/specs/oltp.yaml)| Amazon RDS |
|[OLAP](./src/stacks/specs/olap.yaml)| Snowflake, Bigquery |
|[Machine Learning](./src/stacks/specs/machine_learning.yaml)| MindsDB |
|[Message Queue](./src/stacks/specs/message_queue.yaml)| Amazon SQS, RabbitMQ, Redis |
|[Data Warehouse](./src/stacks/specs/data_warehouse.yaml)| Snowflake, Bigquery |
|[Mongo Alternative on Postgres](./src/stacks/specs/mongo_alternative.yaml)| MongoDB |
|[Geospatial](./src/stacks/specs/gis.yaml)| ESRI, Oracle |
|[Vector DB](./src/stacks/specs/vectordb.yaml)| Pinecone, Weaviate |
|[Time Series](./src/stacks/specs/timeseries.yaml)| InfluxDB, TimescaleDB |
|[Standard](./src/stacks/specs/standard.yaml)| Amazon RDS |

We are actively working on additional Stacks. Check out the [Tembo Roadmap](https://roadmap.tembo.io/roadmap) and upvote the stacks you'd like to see next.

## Anatomy of a Stack

A stack consists of a number of components that are optimized for a particular use case. A stack includes:

* Docker Base Image containing a particular version of Postgres.
* Curated Set of extensions which turn Postgres into best-in-class for that workload.
* Hardware (CPU::Memory ratios, Storage tiers) optimized for the workload.
* Postgres configs optimized according to hardware and use cases.
* Use case specific metrics, alerts and recommendations.
* On-instance application deployments to add additional tools required for the use case.

## Generating a CoreDB Spec from a Stack Spec

```bash
cargo run -- --stack VectorDB --name MyResource --pg-version 16
```

```text
Wrote to spec: MyResource-VectorDB-coredb.json
```

Then apply the generated spec to a Kubernetes cluster:

```bash
kubectl apply -f  MyResource-VectorDB-coredb.json
```
