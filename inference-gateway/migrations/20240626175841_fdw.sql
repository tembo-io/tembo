CREATE EXTENSION IF NOT EXISTS postgres_fdw;
CREATE EXTENSION IF NOT EXISTS pg_cron;

CREATE SERVER IF NOT EXISTS foreign_validation_server
FOREIGN DATA WRAPPER postgres_fdw
OPTIONS (host 'localhost', port '5432', dbname 'postgres');

CREATE USER MAPPING IF NOT EXISTS FOR postgres
SERVER foreign_validation_server
OPTIONS (user 'postgres', password 'postgres');

CREATE FOREIGN TABLE IF NOT EXISTS fdw_paid_organizations (
    organization_id text NOT NULL,
    has_cc boolean not null default false
)
SERVER foreign_validation_server
OPTIONS (schema_name 'public', table_name 'paid_organizations');

SELECT cron.schedule('refresh-validations', '1 second', $$
    INSERT INTO inference.org_validation
    SELECT 
        organization_id as org_id,
        has_cc as valid,
        now() as last_updated_at
    FROM fdw_paid_organizations
    ON CONFLICT (org_id, valid) DO UPDATE SET
        valid = EXCLUDED.valid,
        last_updated_at = EXCLUDED.last_updated_at
$$);
