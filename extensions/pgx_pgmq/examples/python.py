import json
import pprint

from sqlalchemy import create_engine, text

# Connect to the database
engine = create_engine("postgresql://postgres:postgres@localhost:28814/pgx_pgmq")


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

with engine.connect() as con:
    # send a message
    msg = json.dumps({"yolo": 42})
    msg_id = con.execute(text(f"select * from pgmq_send('x', '{msg}') as msg_id;"))
    column_names = msg_id.keys()
    rows = msg_id.fetchall()
    print("### Message ID ###")
    for row in rows:
        pprint.pprint(dict(zip(column_names, row)))
    
with engine.connect() as con:
    # read a message, make it unavailable to be read again for 5 seconds
    read = con.execute(text("select * from pgmq_read('x', 5);"))
    column_names = read.keys()
    rows = read.fetchall()
    print("### Read Message ###")
    for row in rows:
        pprint.pprint(dict(zip(column_names, row)))

with engine.connect() as con:
    # delete a message
    deleted = con.execute(text("select pgmq_delete('x', 1);"))
    column_names = deleted.keys()
    rows = deleted.fetchall()
    print("### Message Deleted ###")
    for row in rows:
        pprint.pprint(dict(zip(column_names, row)))

