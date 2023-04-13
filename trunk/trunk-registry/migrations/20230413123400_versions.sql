-- several records already exist with null values
UPDATE versions SET updated_at = (NOW() at time zone 'UTC') WHERE updated_at IS NULL;
UPDATE versions SET created_at = (NOW() at time zone 'UTC') WHERE created_at IS NULL;

UPDATE extensions SET updated_at = (NOW() at time zone 'UTC') WHERE updated_at IS NULL;
UPDATE extensions SET created_at = (NOW() at time zone 'UTC') WHERE created_at IS NULL;

-- put not null constraints on timestamp columns, at utc defaulting to current time
ALTER TABLE versions
ALTER COLUMN updated_at TYPE timestamp with time zone USING updated_at AT TIME ZONE 'UTC',
ALTER COLUMN updated_at SET DEFAULT (now() at time zone 'UTC'),
ALTER COLUMN updated_at SET NOT NULL,
ALTER COLUMN created_at TYPE timestamp with time zone USING created_at AT TIME ZONE 'UTC',
ALTER COLUMN created_at SET NOT NULL,
ALTER COLUMN created_at SET DEFAULT (now() at time zone 'UTC');

ALTER TABLE extensions
ALTER COLUMN updated_at TYPE timestamp with time zone USING updated_at AT TIME ZONE 'UTC',
ALTER COLUMN updated_at SET DEFAULT (now() at time zone 'UTC'),
ALTER COLUMN updated_at SET NOT NULL,
ALTER COLUMN created_at TYPE timestamp with time zone USING created_at AT TIME ZONE 'UTC',
ALTER COLUMN created_at SET NOT NULL,
ALTER COLUMN created_at SET DEFAULT (now() at time zone 'UTC');
