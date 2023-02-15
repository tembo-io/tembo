import logging
import os

from sqlalchemy import create_engine, text
from tenacity import retry, stop_after_attempt, wait_fixed

POSTGRES_CONNECTION = os.getenv("POSTGRES_CONNECTION", "postgresql://postgres:postgres@0.0.0.0:5432/postgres")
SCHEMA = os.getenv("PGRST_DB_SCHEMA", "public")

logging.basicConfig(level=logging.DEBUG)

# retry 5 times, wait 2 seconds between each retry
@retry(stop=stop_after_attempt(10), wait=wait_fixed(10))
def initialize() -> list[str]:
    engine = create_engine(POSTGRES_CONNECTION)

    with engine.connect() as con:
        prom_metric_tables = con.execute(text(f"""
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = 'prom_metric'
        """)).fetchall()
    
    if len(prom_metric_tables) < 300:
        # promscale likely not done initializing
        # let retry logic handle it
        msg = "No prometheus data tables found, retrying..."
        logging.warning(msg)
        raise Exception(msg)

    # filter to only "pg_ tables"
    tables = [t[0] for t in prom_metric_tables if "pg_" in t[0]]
    logging.debug(f"prom_metric_tables: {tables}")
    return tables

def create_views(tables):
    engine = create_engine(POSTGRES_CONNECTION)
    for table_name in tables:
        query = f"""
            CREATE OR REPLACE VIEW {SCHEMA}.prom_{table_name} as
            SELECT
                time, value, jsonb(labels) as labels
            FROM
                prom_metric.{table_name}
        """
        with engine.connect() as con:
            con.execute(text(query))
            con.commit()
        logging.info(f"created view for table {table_name}")

if __name__ == "__main__":
    logging.info("Creating views for prometheus data tables")
    tables = initialize()
    create_views(tables)
