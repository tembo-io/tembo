CREATE TABLE IF NOT EXISTS custom_secret_table (
    id SERIAL PRIMARY KEY,
    secret_value TEXT NOT NULL
);

INSERT INTO custom_secret_table (secret_value) VALUES (current_setting('tembo.custom_secret'));
