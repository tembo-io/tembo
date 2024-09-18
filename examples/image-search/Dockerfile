FROM python:3.12-slim-bookworm
COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/uv

COPY . /app

WORKDIR /app
RUN uv sync --frozen

ENV PATH="/app/.venv/bin:$PATH"

CMD ["uv", "run", "jupyter", "notebook", "--allow-root", "--ip 0.0.0.0"]