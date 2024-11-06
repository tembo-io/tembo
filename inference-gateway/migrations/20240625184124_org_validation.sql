CREATE TABLE IF NOT EXISTS inference.org_validation (
    org_id TEXT UNIQUE NOT NULL,
    valid bool not null default false,
    last_updated_at timestamp with time zone not null default now()
);

CREATE INDEX ON inference.org_validation (org_id);
