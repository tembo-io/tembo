# Publish and subscribe data using the CoreDB operator

How to publish data from one CoreDB operator, and subscribe from another.

## Publisher

- First, create a CoreDB resource we can use as the publisher.

```
apiVersion: kube.rs/v1
kind: CoreDB
metadata:
  name: coredb-sample-publisher
  spec:
    replicas: 1
```
- Or, you can use the coredb CLI `coredb create coredb-sample-publisher`

- Then, connect and add some data.
```
CREATE TABLE customers (
   id serial PRIMARY KEY,
   name VARCHAR(50) NOT NULL,
   email VARCHAR(50) NOT NULL UNIQUE,
   created_at TIMESTAMP DEFAULT NOW()
);

INSERT INTO customers (name, email)
VALUES ('John Doe', 'john.doe@example.com');
```
- In the above example, we create a table "customers" and add one row, in the database named 'postgres'.
- You can connect into the database with `kubectl exec -it pod_name -- /bin/bash`, or you can connect with the coredb CLI `coredb connect coredb-sample-publisher`

### Create a CoreDBPublication

- Create a publication using the CoreDB operator
```
apiVersion: kube.rs/v1
kind: CoreDBPublication
metadata:
  name: coredb-customers
  spec:
    coredbRef: coredb-rds-replica
    dbname: postgres
	tables:
      - customers
```
- Or, you can use the CoreDB CLI `coredb create publication --publisher coredb-rds-replica --database postgres --tables customers coredb-customers`
- List publications with `coredb get publications`

## Subscribers

Subscribers will receive data from the publication.

- Apply an additional CoreDB to be the subscriber.
```
---
apiVersion: kube.rs/v1
kind: CoreDB
metadata:
  name: coredb-sample-subscriber
  spec:
    replicas: 1
```
- Or, you can use the coredb CLI `coredb create coredb-sample-subscriber`


### Create a CoreDBSubscription

```
apiVersion: kube.rs/v1
kind: CoreDBSubscription
metadata:
  name: coredb-sample-subscription-1
  spec:
    connectionInfo: coredb-sample-publisher
    coredbRef: coredb-sample-subscriber-1
    dbname: postgres
    publication: coredb-customers
```
- Or, you can use the CoreDB CLI `coredb create subscription --subscriber coredb-sample-subscription-1 --publication coredb-customers coredb-sample-subscription-1`
