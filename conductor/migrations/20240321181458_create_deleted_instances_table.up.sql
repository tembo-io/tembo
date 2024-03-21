-- Up migration
CREATE TABLE deleted_instances (
    namespace VARCHAR(255) NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (namespace)
);

CREATE UNIQUE INDEX idx_deleted_instances_namespace ON deleted_instances(namespace);
