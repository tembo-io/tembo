# Postgres Message Queue

A lightweight message queue extension for Postgres.

## Usage

### Installation

Setup `pgx`.
```bash
cargo install --locked cargo-pgx
cargo pgx init
```

Then, clone this repo and change into this directory.

```
git clone git@github.com:CoreDB-io/coredb.git
cd coredb/extensions/pgmq/
```

Run the dev environment
```bash
cargo pgx run pg14
```


```sql
CREATE EXTENSION pgmq;

```

### Creating a queue

```sql
SELECT pgmq_create('my_queue');

 pgmq_create 
-------------
 
```

### Enqueueing a message

```sql
pgmq=# SELECT * from pgmq_send('my_queue', '{"foo": "bar"}');
 pgmq_send
--------------
            1
```

## Read a message
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

