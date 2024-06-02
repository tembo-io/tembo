-- rename to old
ALTER TABLE inference.requests RENAME to _requests;

-- create new table as partitioned
CREATE TABLE inference.requests (LIKE inference._requests INCLUDING ALL) PARTITION BY RANGE (completed_at);

-- make it a timeseries table
SELECT enable_ts_table(
    target_table_id => 'inference.requests',
    partition_duration => '7 days',
    partition_lead_time => '1 mon',
    initial_table_start => '2024-05-20'
);

-- move the data (there is not much at this point in time)
INSERT INTO inference.requests SELECT * FROM inference._requests;

DROP TABLE inference._requests;
