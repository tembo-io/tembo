# Postgres Message Queue

A lightweight message queue extension for Postgres.

## Usage

### Installation

```sql
CREATE EXTENSION pg_smq;
```

### Creating a queue

```sql
SELECT pgmq_create_queue('my_queue');
```

### Enqueueing a message

```sql
SELECT pgmq_enqueue('my_queue', '{"foo": "bar"}');
```

## Read a message
Reads a single message from the queue. If the queue is empty, or all messages are currently invisible, it will immediately return None.
```sql
SELECT pgmq_read('my_queue');
```

### Pop a message
Read a message and immediately delete it from the queue.
```sql
SELECT pgmq_pop('my_queue');
```

### Delete a message
Delete a message with id `2` from queue named `my_queue`.
```sql
SELECT pgmq_delete('my_queue', 2);
```

