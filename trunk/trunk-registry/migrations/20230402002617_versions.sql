ALTER TABLE versions
ADD CONSTRAINT fk_extension_id
FOREIGN KEY (extension_id)
REFERENCES extensions(id);
