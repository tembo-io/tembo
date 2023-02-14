import logging
import os

from sqlalchemy import create_engine, text
from sqlalchemy.exc import ProgrammingError, InternalError
from tenacity import retry, stop_after_attempt, wait_fixed

POSTGRES_CONNECTION = os.getenv("POSTGRES_CONNECTION", "postgresql://postgres:postgres@0.0.0.0:5432/postgres")

# Connect to the CoreDB postgres
engine = create_engine(POSTGRES_CONNECTION)

# retry 5 times, wait 2 seconds between each retry
@retry(stop=stop_after_attempt(5), wait=wait_fixed(2))
def main():
    # create extension
    with engine.connect() as con:
        prom_data_tables = con.execute(text( "SELECT table_name FROM information_schema.tables WHERE table_schema = 'prom_data'")).fetchall()
        
        # create view on each table, including metrics labels
        for t in prom_data_tables:
            table_name = t[0]
            if "pg_" in table_name:
                query = f"""
                    CREATE OR REPLACE VIEW prom_data.prom_{table_name} as
                    SELECT
                        time, value, jsonb(labels) as labels
                    FROM
                        prom_data.{table_name}
                """
                try:
                    q = con.execute(text(query))
                except (ProgrammingError) as e:
                    logging.warning(f"error creating view for table {table_name}")
                except InternalError:
                    logging.info("view already exists")

        con.commit()
