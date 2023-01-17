# CoreDB operator as an AWS RDS read replica

It could be useful to self-host a read replica of your AWS RDS database.

## AWS RDS setup

There are a few steps required on the AWS side in order to grant permissions for the operator to read the data.

### Enable logical replication

This step is needed in order to use PostgreSQL logical replication feature to publish data from your AWS RDS primary to the CoreDB operator subscriber.

- Check the write-ahead-log (WAL) level like this:
```
postgres=> SHOW wal_level;
 wal_level
 -----------
  logical
  (1 row)
```
- If it is not already 'logical', then enable logical replication by setting the RDS parameter `rds.logical_replication` to `1`. AWS documentation on parameter groups can be found [here](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/USER_WorkingWithParamGroups.html).


### Create a user with the appropriate permissions

In this step, we can create a user with sufficient permissions to read from your primary database, and to create replication slots.

- Connected to your AWS RDS primary, create a new user (substitute a password for `****`):

```
CREATE USER coredb_replication PASSWORD ****;
```

- Grant permission to read tables to this user.
- In the below example, we grant permission to all present and future tables:
```
GRANT USAGE ON SCHEMA "public" TO coredb_replication;
GRANT SELECT ON ALL TABLES IN SCHEMA "public" TO coredb_replication;
ALTER DEFAULT PRIVILEGES IN SCHEMA "public" GRANT SELECT ON TABLES TO coredb_replication;
```
- It is also possible to grant permisson to only a subset of tables, for which an example is not shown.

### Create a publication

- Then, grant the user permissions to create replication slots using the RDS-specific role name.
```
GRANT rds_replication to coredb_replication;
```
- Then, create a `PUBLICATION` for the tables you want to sync into CoreDB.
- In this example, we create a publication for all tables:
```
CREATE PUBLICATION alltables FOR ALL TABLES;
```

## CoreDB setup

Now, we just need to configure CoreDB as a subscriber of the publication we just created.

### Create a Kubernetes Secret

- Create a secret that CoreDB can use to connect to AWS RDS
  - Substitue `****` for the password you created for the user coredb_replication in your AWS RDS primary.
  - Substitue "your-rds-host.example.com" with a domain name that resolves to the primary AWS RDS endpoint
  - Substitue "your-db-name" with the name of the database in the AWS RDS primary where the replication user was created

```
kubectl create secret generic coredb-replication-connection-info \
--from-literal=user="coredb_replication" \
--from-literal=password="****" \
--from-literal=host="your-rds-host.example.com" \
--from-literal=dbname="your-db-name" \
--from-literal=sslmode="require" \
```

### Create a CoreDB

- Apply a CoreDB into your Kubernetes cluster
```
apiVersion: coredb.io/v1
kind: CoreDB
metadata:
  name: coredb-rds-replica
  spec:
    replicas: 1
```
- Or, you can use the coredb CLI `coredb create db coredb-rds-replica`

### Create a CoreDBSubscription

- In the below example, we create the subscription from your AWS RDS primary into the CoreDB database named 'postgres', but you can create the subscription into any database inside your CoreDB.
```
apiVersion: coredb.io/v1
kind: Subscription
metadata:
  name: coredb-rds-replication-subscription
  spec:
    connectionInfo: coredb-replication-connection-info
    coredbRef: coredb-rds-replica
    dbname: postgres
```
- Or, you can use the CoreDB CLI `coredb create subscription --subscriber coredb-rds-replica --connection-info coredb-replication-connection-info --database postgres coredb-rds-replication-subscription`
