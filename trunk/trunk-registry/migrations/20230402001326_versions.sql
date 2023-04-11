CREATE TABLE IF NOT EXISTS versions (
    id BIGSERIAL PRIMARY KEY,
    extension_id INT4,
    num VARCHAR,
    updated_at TIMESTAMP,
    created_at TIMESTAMP,
    downloads INT4,
    features JSONB,
    yanked BOOL,
    license VARCHAR,
    extension_size INT4,
    published_by INT4,
    checksum BPCHAR(64),
    links VARCHAR
);
