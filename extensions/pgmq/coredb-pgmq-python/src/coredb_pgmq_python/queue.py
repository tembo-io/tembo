import datetime
from dataclasses import dataclass, field
from typing import Optional, Union

from psycopg.types.json import Jsonb, set_json_dumps, set_json_loads
from psycopg_pool import ConnectionPool
from pydantic import BaseModel


@dataclass
class Message:
    msg_id: int
    read_ct: int
    enqueued_at: datetime
    vt: datetime
    message: dict


@dataclass
class PGMQueue:
    """Base class for interacting with a queue"""

    host: str = "localhost"
    port: str = "5432"
    database: str = "postgres"
    delay: int = 0
    vt: int = 30
    partition_size: int = 5000

    username: str = "postgres"
    password: str = "postgres"

    pool_size: int = 10

    kwargs: Optional[dict] = field(default_factory=dict)

    pool: ConnectionPool = field(init=False)

    def __post_init__(self) -> None:
        conninfo = f"host={self.host} port={self.port} dbname={self.database} user={self.username} password={self.password}"
        self.pool = ConnectionPool(conninfo, **self.kwargs)

        with self.pool.connection() as conn:
            rows = conn.execute("create extension if not exists pgmq cascade;")
            print(rows)

    def create_queue(self, queue: str) -> None:
        """Create a queue"""
        with self.pool.connection() as conn:
            conn.execute("select pgmq_create(%s);", [queue])

    def create_partitioned_queue(
        self, queue: str, partition_size: Optional[int] = None
    ) -> None:
        """Create a partitioned queue"""
        with self.pool.connection() as conn:
            conn.execute(
                "select pgmq_create_partitioned(%s, %s);", [queue, partition_size]
            )

    def send(self, queue: str, message: dict, delay: int = None) -> None:
        """Send a message to a queue"""

        with self.pool.connection() as conn:
            if delay is not None:
                # TODO(chuckend): implement send_delay in pgmq
                raise NotImplementedError("send_delay is not implemented in pgmq")
            message = conn.execute(
                "select * from pgmq_send(%s, %s);",
                [queue, Jsonb(message)],
            ).fetchall()
            return message

    def read(
        self, queue: str, vt: Optional[int] = None, limit: int = 1
    ) -> Union[Message, list[Message]]:
        """Read a message from a queue"""
        with self.pool.connection() as conn:
            rows = conn.execute(
                "select * from pgmq_read(%s, %s, %s);", [queue, vt or self.vt, limit]
            ).fetchall()

        messages = [
            Message(msg_id=x[0], read_ct=x[1], enqueued_at=x[2], vt=x[3], message=x[4])
            for x in rows
        ]
        return messages[0] if len(messages) == 1 else messages


if __name__ == "__main__":
    q = PGMQueue(host="0.0.0.0")
    q.create_queue("test")

    msg_id = q.send("test", {"hello": "world"})
    print(msg_id)
