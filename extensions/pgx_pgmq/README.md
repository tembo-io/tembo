# Postgres Message Queue


A lightweight message queue extension for Postgres. Provides similar experience to [AWS SQS](https://aws.amazon.com/sqs/) and [Redis Simple Message Queue](https://github.com/smrchy/rsmq), but on Postgres.

- [Postgres Message Queue](#postgres-message-queue)
  - [Installation](#installation)
  - [Python Examples](#python-examples)
    - [Connect to postgres](#connect-to-postgres)
    - [Create and list queues](#create-and-list-queues)
    - [Send a message to the queue](#send-a-message-to-the-queue)
    - [Read a message from the queue](#read-a-message-from-the-queue)
    - [Delete a message from the queue](#delete-a-message-from-the-queue)
  - [SQL Examples](#sql-examples)
    - [Creating a queue](#creating-a-queue)
    - [Send a message](#send-a-message)
    - [Read a message](#read-a-message)
    - [Pop a message](#pop-a-message)
    - [Archive a message](#archive-a-message)
    - [Delete a message](#delete-a-message)
  - [Development](#development)

## Installation

TODO
`docker run ...`


## Python Examples

### Connect to postgres

```python
import json
import pprint

from sqlalchemy import create_engine, text

engine = create_engine("postgresql://postgres:postrgres@localhost:28814/pgx_pgmq")
```

### Create and list queues

```python
with engine.connect() as con:
    # create a queue
    created = con.execute(text( "select * from pgmq_create('myqueue');"))
    # list queues
    list_queues = con.execute(text( "select * from pgmq_list_queues()"))
    column_names = list_queues.keys()
    rows = list_queues.fetchall()
    print("### Queues ###")
    for row in rows:
        pprint.pprint(dict(zip(column_names, row)))
```
```
'### Queues ###'
{'created_at': datetime.datetime(2023, 2, 7, 2, 5, 39, 946356, tzinfo=datetime.timezone(datetime.timedelta(days=-1, seconds=64800))),
 'queue_name': 'myqueue'}
 ```


### Send a message to the queue

```python
with engine.connect() as con:
    # send a message
    msg = json.dumps({"yolo": 42})
    msg_id = con.execute(text(f"select * from pgmq_send('x', '{msg}') as msg_id;"))
    column_names = msg_id.keys()
    rows = msg_id.fetchall()
    print("### Message ID ###")
    for row in rows:
        pprint.pprint(dict(zip(column_names, row)))
```
```
'### Message ID ###'
{'msg_id': 1}
```

### Read a message from the queue

```python
with engine.connect() as con:
    # read a message, make it unavailable to be read again for 5 seconds
    read = con.execute(text("select * from pgmq_read('x', 5);"))
    column_names = read.keys()
    rows = read.fetchall()
    print("### Read Message ###")
    for row in rows:
        pprint.pprint(dict(zip(column_names, row)))
```
```
'### Read Message ###'
{'enqueued_at': datetime.datetime(2023, 2, 7, 2, 51, 50, 468837, tzinfo=datetime.timezone(datetime.timedelta(days=-1, seconds=64800))),
 'message': {'myqueue': 42},
 'msg_id': 1,
 'read_ct': 1,
 'vt': datetime.datetime(2023, 2, 7, 16, 9, 4, 826669, tzinfo=datetime.timezone(datetime.timedelta(days=-1, seconds=64800)))}
 ```

### Delete a message from the queue

```python
with engine.connect() as con:
    # delete a message
    deleted = con.execute(text("select pgmq_delete('x', 1);"))
    column_names = deleted.keys()
    rows = deleted.fetchall()
    print("### Message Deleted ###")
    for row in rows:
        pprint.pprint(dict(zip(column_names, row)))
```
```
'### Message Deleted ###'
{'pgmq_delete': True}
```

## SQL Examples

```sql
CREATE EXTENSION pgmq;
```

### Creating a queue

```sql
SELECT pgmq_create('my_queue');

 pgmq_create
-------------

```

### Send a message

```sql
pgmq=# SELECT * from pgmq_send('my_queue', '{"foo": "bar"}');
 pgmq_send
--------------
            1
```

### Read a message
Reads a single message from the queue. Make it invisible for 30 seconds.
```sql
pgmq=# SELECT * from pgmq_read('my_queue', 30);

 msg_id | read_ct |              vt               |          enqueued_at          |    message
--------+---------+-------------------------------+-------------------------------+---------------
      1 |       2 | 2023-02-07 04:56:00.650342-06 | 2023-02-07 04:54:51.530818-06 | {"foo":"bar"}
```

If the queue is empty, or if all messages are currently invisible, no rows will be returned.

```sql
pgx_pgmq=# SELECT * from pgmq_read('my_queue', 30);
 msg_id | read_ct | vt | enqueued_at | message
--------+---------+----+-------------+---------

```

### Pop a message
Read a message and immediately delete it from the queue. Returns `None` if the queue is empty.
```sql
pgmq=# SELECT * from pgmq_pop('my_queue');

 msg_id | read_ct |              vt               |          enqueued_at          |    message
--------+---------+-------------------------------+-------------------------------+---------------
      1 |       2 | 2023-02-07 04:56:00.650342-06 | 2023-02-07 04:54:51.530818-06 | {"foo":"bar"}
```

### Archive a message

Archiving a message removes it from the queue, and inserts it to the archive table.
TODO:


### Delete a message
Delete a message with id `1` from queue named `my_queue`.
```sql
pgmq=# select pgmq_delete('my_queue', 1);
 pgmq_delete
-------------
 t
 ```




## Development

Setup `pgx`.

```bash
cargo install --locked cargo-pgx
cargo pgx init
```

Then, clone this repo and change into this directory.

```bash
git clone git@github.com:CoreDB-io/coredb.git
cd coredb/extensions/pgmq/
```

Run the dev environment

```bash
cargo pgx run pg14
```

## Packaging

Run this script to package into a `.deb` file, which can be installed on Ubuntu.

```
/bin/bash build-extension.sh
```
