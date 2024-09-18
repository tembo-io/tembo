# Image Search on Postgres

An example of how to use Postgres to search for images using text or an image as the search query.

## Setup

Install [uv](https://github.com/astral-sh/uv?tab=readme-ov-file#installation), a modern package manager for Python.

Clone this repo

```bash
git clone https://github.com/tembo-io/tembo.git

cd examples/image-search
```

Create a new Python environment with all the necessary dependencies.

```bash
uv sync
```

Start the Postgres database

```bash
docker compose up postgres -d
```

Run the notebook server.

```bash
uv run jupyter notebook
```

Navigate to `http://localhost:8888/notebooks/demo.ipynb` in your browser and follow the instructions in the notebook.

