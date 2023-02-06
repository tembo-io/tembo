import os

from fastapi import FastAPI
from sqlalchemy import create_engine
from dotenv import load_dotenv

load_dotenv()


PG_URL = os.environ["DATABASE_URL"]

_engine = None

def connect_pg(timeout=10):
    global _engine
    if _engine is None:
        _engine = create_engine(PG_URL)
    return _engine


app = FastAPI()


@app.get("/")
def read_root():
    with connect_pg().connect() as connection:
        resp = connection.execute(
            f"""
            SELECT id, title
            FROM items
        """
        )
        return [
            {column: value for column, value in rowproxy.items()}
            for rowproxy in resp
        ]



if __name__ == "__main__":
    import uvicorn

    uvicorn.run("app:app", host="0.0.0.0", port=5000, reload=True)