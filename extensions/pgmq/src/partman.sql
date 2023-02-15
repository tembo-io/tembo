-- should run on install
CREATE SCHEMA IF NOT EXISTS partman;
CREATE EXTENSION IF NOT EXISTS pg_partman SCHEMA partman;

-- creates the role for partman
-- CREATE ROLE partman WITH LOGIN;
-- GRANT ALL ON SCHEMA partman TO partman;
-- GRANT ALL ON ALL TABLES IN SCHEMA partman TO partman;
-- GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA partman TO partman;
-- GRANT EXECUTE ON ALL PROCEDURES IN SCHEMA partman TO partman;
-- GRANT ALL ON SCHEMA my_partition_schema TO partman;
-- GRANT TEMPORARY ON DATABASE postgres to partman; -- allow creation of temp tables to move data out of default 

-- maybe dont need this
-- GRANT CREATE ON DATABASE postgres TO partman;
