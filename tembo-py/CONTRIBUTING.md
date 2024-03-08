# Contributing to the `tembo-py` Python client

Welcome!
And thank you for your interest in contributing to this Python library.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Run locally](#run-locally)
3. [Testing](#testing)

## Prerequisites

- [Python](https://www.python.org/) - Programming language and companion `pip` package manager
- [Poetry](https://python-poetry.org/) - Python packaging and dependency management
- [Docker Engine](https://docs.docker.com/engine/install/) - For running local containers

## Running Locally

### 1. Initial Setup

#### 1.1. Clone the GitHub Repository

If you haven't already, go ahead and clone the tembo repository to your local machine and navigate to the `tembo-py` directory.

```bash
git clone https://github.com/tembo-io/tembo.git
```
```bash
cd tembo/tembo-py
```

#### 1.2. Confirm Installation of Python and Poetry

You can run the following to check whether your system has the appropriate software:

```bash
python3 --version
```

```bash
poetry --version
```

```bash
poetry update
```

#### 1.3. Establish Poetry in the Working Directory

From within `tembo/tembo-py` run the following:

```bash
poetry install
```

### 2. Run PostgreSQL

```bash
make run.postgres
```

```bash
psql -h localhost -p 5432 -U postgres -W
```

```bash
CREATE EXTENSION vectorize CASCADE;
```

## Testing

```bash
make test
```



