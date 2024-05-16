CREATE SCHEMA inference;

CREATE TABLE inference.requests(
    organization_id text not null,
    model text not null,
    prompt_tokens integer not null,
    completion_tokens integer not null
);