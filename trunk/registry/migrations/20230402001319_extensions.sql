CREATE TABLE IF NOT EXISTS extensions (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR,
    updated_at TIMESTAMP,
    created_at TIMESTAMP,
    downloads INT4,
    description VARCHAR,
    homepage VARCHAR,
    documentation VARCHAR,
    repository VARCHAR
);
