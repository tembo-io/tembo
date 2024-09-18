CREATE SCHEMA IF NOT EXISTS billing;

CREATE TABLE billing.reporter_watermark (
    -- Timestamp in which the reporter last ran successfully
    last_reported_at timestamp with time zone NOT NULL
);
