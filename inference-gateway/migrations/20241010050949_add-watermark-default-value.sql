ALTER TABLE billing.reporter_watermark 
ALTER COLUMN last_reported_at SET DATA TYPE TIMESTAMP WITH TIME ZONE;

ALTER TABLE billing.reporter_watermark 
ALTER COLUMN last_reported_at SET DEFAULT '1970-01-01';

ALTER TABLE billing.reporter_watermark 
ALTER COLUMN last_reported_at SET NOT NULL;

INSERT INTO billing.reporter_watermark (last_reported_at)
VALUES (DEFAULT);