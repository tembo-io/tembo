import logging
import os

from sqlalchemy import create_engine, text
from tenacity import retry, stop_after_attempt, wait_fixed

POSTGRES_CONNECTION = os.getenv("POSTGRES_CONNECTION", "postgresql://postgres:postgres@0.0.0.0:5433/postgres")

logging.basicConfig(level=logging.DEBUG)

# retry 5 times, wait 2 seconds between each retry
@retry(stop=stop_after_attempt(5), wait=wait_fixed(2))
def create_extensions():
    engine = create_engine(POSTGRES_CONNECTION)
    query = "CREATE EXTENSION pg_stat_statements"
    with engine.connect() as con:
        con.execute(text(query))
        con.commit()
    logging.info("created extension pg_stat_statements")

if __name__ == "__main__":
    logging.info("Creating extensions for arbitrary postgres")
    create_extensions()
