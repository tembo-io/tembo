CREATE EXTENSION IF NOT EXISTS postgres_fdw;
CREATE EXTENSION IF NOT EXISTS pg_cron;

CREATE SERVER IF NOT EXISTS cp_queue_server
FOREIGN DATA WRAPPER postgres_fdw
OPTIONS (host 'localhost', port '5432', dbname 'postgres');

CREATE USER MAPPING IF NOT EXISTS FOR postgres
SERVER cp_queue_server
OPTIONS (user 'postgres', password 'postgres');

CREATE FOREIGN TABLE IF NOT EXISTS fdw_paid_organizations (
    organization_id text NOT NULL,
    has_cc boolean not null default false,
    last_updated_at timestamp with time zone not null default now()
)
SERVER cp_queue_server
OPTIONS (schema_name 'billing', table_name 'paid_organizations');

SELECT cron.schedule('refresh-validations', '* * * * *', $$
    BEGIN;
    TRUNCATE inference.org_validation;
    INSERT INTO inference.org_validation
    SELECT 
        organization_id as org_id,
        has_cc as valid,
        now() as last_updated_at
    FROM fdw_paid_organizations;
    COMMIT;
$$);
