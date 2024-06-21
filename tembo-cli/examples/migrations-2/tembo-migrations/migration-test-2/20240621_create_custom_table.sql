CREATE TABLE IF NOT EXISTS custom_secret_table (
    id SERIAL PRIMARY KEY,
    secret_value TEXT NOT NULL
);

CREATE OR REPLACE FUNCTION set_custom_parameters() RETURNS void AS $$
BEGIN
    PERFORM set_config('tembo.custom_secret', current_setting('tembo.custom_secret'), false);
END;
$$ LANGUAGE plpgsql;

SELECT set_custom_parameters();

INSERT INTO custom_secret_table (secret_value) VALUES (current_setting('tembo.custom_secret'));

SELECT secret_value FROM custom_secret_table WHERE secret_value = current_setting('tembo.custom_secret');
