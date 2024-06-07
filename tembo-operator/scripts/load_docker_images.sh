#!/usr/bin/env bash

# List of images
# These are taken from tests/integration_tests.rs and src/defaults.rs
images=(
	"quay.io/tembo/standard-cnpg:16-a0a5ab5"
	"quay.io/tembo/standard-cnpg:14-a0a5ab5"
	"quay.io/tembo/standard-cnpg:15-a0a5ab5"
	"postgrest/postgrest:v10.0.0"
	"crccheck/hello-world:latest"
	"ghcr.io/ferretdb/ferretdb"
	"quay.io/tembo/standard-cnpg:15-120cc24"
	"prom/blackbox-exporter"
)

# Loop through each image
for image in "${images[@]}"; do
	echo "Pulling image: $image"
	docker pull "$image"

	echo "Loading image into kind cluster: $image"
	kind load docker-image "$image"
done

echo "All images have been pulled and loaded into the kind cluster."
