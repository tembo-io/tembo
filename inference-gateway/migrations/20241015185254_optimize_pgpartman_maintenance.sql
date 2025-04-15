-- This migration addresses the slow query issue with pg_partman's run_maintenance function
-- by optimizing its performance through parallel maintenance and resource management

-- First, create a custom wrapper function for better maintenance control and performance
CREATE OR REPLACE FUNCTION optimized_run_maintenance(
    p_schema text DEFAULT 'partman', 
    p_batch_count int DEFAULT 1,
    p_analyze boolean DEFAULT false,
    p_jobmon boolean DEFAULT true,
    p_debug boolean DEFAULT false
) RETURNS void AS $$
DECLARE
    v_current_maintenance_work_mem text;
    v_maintenance_start_time timestamptz := clock_timestamp();
    v_duration interval;
BEGIN
    -- Log start of maintenance
    RAISE NOTICE 'Starting optimized partition maintenance at %', v_maintenance_start_time;
    
    -- Save current maintenance_work_mem setting
    SELECT setting INTO v_current_maintenance_work_mem FROM pg_settings WHERE name = 'maintenance_work_mem';
    
    -- Temporarily increase maintenance_work_mem for better performance
    -- Only for this session, this won't affect other connections
    EXECUTE 'SET maintenance_work_mem TO ''1GB''';
    
    -- Run partman maintenance with custom parameters
    PERFORM pg_partman.run_maintenance(
        p_schema := p_schema,
        p_batch_count := p_batch_count,
        p_analyze := p_analyze,
        p_jobmon := p_jobmon,
        p_debug := p_debug
    );
    
    -- Restore original maintenance_work_mem
    EXECUTE 'SET maintenance_work_mem TO ' || quote_literal(v_current_maintenance_work_mem);
    
    -- Calculate and log duration
    v_duration := clock_timestamp() - v_maintenance_start_time;
    RAISE NOTICE 'Optimized partition maintenance completed in %', v_duration;
    
    -- Log performance data
    PERFORM log_maintenance_performance(
        'optimized_run_maintenance',
        v_maintenance_start_time,
        NULL, -- rows_processed (not available from pg_partman)
        NULL, -- partitions_created
        NULL, -- partitions_dropped
        jsonb_build_object(
            'schema', p_schema,
            'batch_count', p_batch_count,
            'analyze', p_analyze,
            'maintenance_work_mem', current_setting('maintenance_work_mem')
        )
    );
END;
$$ LANGUAGE plpgsql;

-- Create a function to manage pgpartman maintenance in parallel
CREATE OR REPLACE FUNCTION parallel_run_maintenance(
    p_schemas text[] DEFAULT ARRAY['partman'],
    p_batch_count int DEFAULT 1
) RETURNS void AS $$
BEGIN
    -- Using pg_background_launch to run maintenance in parallel for multiple schemas
    -- This requires PostgreSQL 14 or later
    IF pg_catalog.current_setting('server_version_num')::integer >= 140000 THEN
        FOR i IN 1..array_length(p_schemas, 1) LOOP
            PERFORM pg_background_launch(
                format('SELECT optimized_run_maintenance(%L, %s, false, true, false)',
                p_schemas[i], p_batch_count)
            );
        END LOOP;
        RAISE NOTICE 'Launched % parallel maintenance jobs', array_length(p_schemas, 1);
    ELSE
        -- Fallback for PostgreSQL 13 or earlier
        RAISE NOTICE 'Parallel maintenance requires PostgreSQL 14 or later; running serially';
        FOR i IN 1..array_length(p_schemas, 1) LOOP
            PERFORM optimized_run_maintenance(p_schemas[i], p_batch_count, false, true, false);
        END LOOP;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Create a table to track maintenance performance
CREATE TABLE IF NOT EXISTS maintenance_performance_log (
    id serial PRIMARY KEY,
    function_name text NOT NULL,
    start_time timestamptz NOT NULL,
    end_time timestamptz NOT NULL,
    duration interval NOT NULL,
    rows_processed bigint,
    partitions_created int,
    partitions_dropped int,
    additional_info jsonb
);

-- Create a function to monitor maintenance performance
CREATE OR REPLACE FUNCTION log_maintenance_performance(
    p_function_name text,
    p_start_time timestamptz,
    p_rows_processed bigint DEFAULT NULL,
    p_partitions_created int DEFAULT NULL,
    p_partitions_dropped int DEFAULT NULL,
    p_additional_info jsonb DEFAULT NULL
) RETURNS void AS $$
DECLARE
    v_end_time timestamptz := clock_timestamp();
    v_duration interval;
BEGIN
    v_duration := v_end_time - p_start_time;
    
    INSERT INTO maintenance_performance_log (
        function_name, 
        start_time, 
        end_time, 
        duration, 
        rows_processed,
        partitions_created,
        partitions_dropped,
        additional_info
    ) VALUES (
        p_function_name,
        p_start_time,
        v_end_time,
        v_duration,
        p_rows_processed,
        p_partitions_created,
        p_partitions_dropped,
        p_additional_info
    );
    
    -- Alert if maintenance takes too long
    IF v_duration > interval '5 minutes' THEN
        RAISE WARNING 'Maintenance operation % took % to complete, which exceeds the 5 minute threshold',
            p_function_name, v_duration;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Create a cron job to run the maintenance at a less busy time
-- Adjust this schedule based on your database usage patterns
-- This example runs at 2:00 AM daily with reduced frequency
DO $$
BEGIN
    -- Remove existing maintenance cron job if it exists
    PERFORM cron.unschedule(job_name) 
    FROM cron.job 
    WHERE command LIKE '%run_maintenance%';
    
    -- Schedule optimized maintenance during off-peak hours
    PERFORM cron.schedule(
        'pgpartman-maintenance',
        '0 2 * * *',  -- Run at 2 AM daily
        'SELECT optimized_run_maintenance()',
        'Optimized pg_partman maintenance'
    );
    
    RAISE NOTICE 'Scheduled optimized maintenance to run at 2:00 AM daily';
END;
$$;

-- Comment explaining the optimization approach
COMMENT ON FUNCTION optimized_run_maintenance IS 
'Optimized wrapper for pg_partman.run_maintenance that manages resources better 
and provides more detailed logging. Use this instead of calling pg_partman.run_maintenance directly.';

COMMENT ON FUNCTION parallel_run_maintenance IS
'Runs maintenance operations in parallel using pg_background_launch for improved performance
when maintaining multiple partition schemas.';

-- Create a view to analyze maintenance performance
CREATE OR REPLACE VIEW maintenance_performance_analysis AS
SELECT 
    function_name,
    date_trunc('day', start_time) AS day,
    count(*) AS executions,
    avg(extract(epoch from duration)) AS avg_duration_seconds,
    max(extract(epoch from duration)) AS max_duration_seconds,
    min(extract(epoch from duration)) AS min_duration_seconds,
    sum(rows_processed) AS total_rows_processed,
    sum(partitions_created) AS total_partitions_created,
    sum(partitions_dropped) AS total_partitions_dropped
FROM maintenance_performance_log
GROUP BY function_name, date_trunc('day', start_time)
ORDER BY day DESC, avg_duration_seconds DESC;