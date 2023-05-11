import random
import time
from typing import Optional

from coredb_pgmq_python import Message, PGMQueue


def bench_send(queue: PGMQueue, queue_name: str, msg: dict, num_messages: int) -> list[dict]:
    all_msg_ids = []
    write_start = time.time()
    results = []
    print("Writing Messages")
    for x in range(num_messages):
        start = time.time()
        msg_id = queue.send(queue_name, msg)
        results.append({"operation": "write", "duration": time.time() - start, "msg_id": msg_id})
        all_msg_ids.append(msg_id)
        if (x + 1) % 10000 == 0:
            print(f"write {x+1} messages")
            elapsed = time.time() - write_start
            avg_write = elapsed / (x + 1)
            print(f"running avg write time (seconds): {avg_write}")
    print(f"Sent {x+1} messages")
    return results


def bench_read_archive(queue: PGMQueue, queue_name: str, num_messages: int) -> list[dict]:
    """Benchmarks the read and archive of messages"""
    read_elapsed = 0.0
    archive_elapsed = 0.0
    results = []
    for x in range(num_messages):
        read_start = time.time()
        message: Message = queue.read(queue_name, vt=2)  # type: ignore
        read_duration = time.time() - read_start
        results.append({"operation": "read", "duration": read_duration, "msg_id": message.msg_id})
        read_elapsed += read_duration

        archive_start = time.time()
        queue.archive(queue_name, message.msg_id)
        archive_duration = time.time() - archive_start
        results.append({"operation": "archive", "duration": archive_duration, "msg_id": message.msg_id})
        archive_elapsed += archive_duration

        if (x + 1) % 10000 == 0:
            avg_read = read_elapsed / (x + 1)
            print(f"read {x+1} messages, avg read time (seconds): {avg_read}")
            avg_archive = archive_elapsed / (x + 1)
            print(f"archived {x+1} messages, avg archive time (seconds): {avg_archive}")
    print(f"Read {x+1} messages")
    return results


def bench_line_item(
    host: str,
    port: str,
    username: str = "postgres",
    num_messages: int = 10000,
    vt=10,
    password: str = "postgres",
    database: str = "postgres",
    partition_interval: int = 10000,
    retention_interval: Optional[int] = None,
) -> list[dict]:
    """records each transaction as a separate line item. Captures results into a list.

    returns:
            [{
                "operation": <operation>,
                "duration": <duration, in seconds>,
                "msg_id": <msg_id>
            }]
    """
    rnd = random.randint(0, 100)
    test_queue = f"bench_queue_{rnd}"
    print(f"Test queue: {test_queue}")

    test_message = {"hello": "world"}
    bench_0_start = time.time()
    queue = PGMQueue(host=host, port=port, username=username, password=password, database=database)
    try:
        print(f"Queue retention: {retention_interval}")
        if retention_interval is None:
            print("Defaulting to retaining all messages: {}")
            retention_interval = num_messages
        queue.create_queue(test_queue, partition_interval=partition_interval, retention_interval=retention_interval)
    except Exception as e:
        print(f"{e}")

    print(
        f"""
    Starting benchmark
    Total messages: {num_messages}
    """
    )

    total_results = []

    # publish messages
    write_results: list[dict] = bench_send(queue, test_queue, test_message, num_messages)
    total_results.extend(write_results)

    # read them all once, each
    print("Reading Messages")
    read_arch_results: list[dict] = bench_read_archive(queue, test_queue, num_messages)
    total_results.extend(read_arch_results)

    # wait for all VT to expire
    while time.time() - bench_0_start < vt:
        print("waiting for all VTs to expire")
        time.sleep(2)

    print("Benchmarking: Message Deletion")
    all_msg_ids = []
    # publish messages
    for x in range(num_messages):
        start = time.time()
        msg_id = queue.send(test_queue, test_message)
        all_msg_ids.append(msg_id)

    print("Deleting Messages")
    for x in all_msg_ids:
        start = time.time()
        queue.delete(test_queue, x)
        total_results.append({"operation": "delete", "duration": time.time() - start, "msg_id": x})

    return total_results
