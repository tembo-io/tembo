CREATE EXTENSION IF NOT EXISTS timeseries CASCADE;
ALTER TABLE inference.requests ADD COLUMN completed_at timestamp with time zone not null DEFAULT now();
ALTER TABLE inference.requests ADD COLUMN duration_ms integer not null default 0;
