ALTER TABLE inference.requests ADD COLUMN instance_id text not null DEFAULT '_none';
ALTER TABLE inference.requests ALTER COLUMN instance_id DROP DEFAULT, ALTER COLUMN instance_id SET NOT NULL;
