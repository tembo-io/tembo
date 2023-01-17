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

### Create a CoreDBPublication

- Create a publication using the CoreDB operator
```
apiVersion: kube.rs/v1
kind: CoreDBPublication
metadata:
  name: coredb-customers
  spec:
    connectionInfo: coredb-sample-publisher
    coredbRef: coredb-rds-replica
    dbname: postgres
	tables:
      - customers
```

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
