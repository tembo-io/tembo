#!/usr/bin/env bash

# Check if $1 is not provided
if [ -z "$1" ]; then
    echo "Error: Missing argument for backup name."
    exit 1
fi

# Start the port-forward in the background
kubectl port-forward -n minio svc/minio 9001:9000 &

# Capture the process ID of the background process
PORT_FORWARD_PID=$!

# Give it a second to establish the connection
sleep 1

# Execute the AWS command
AWS_ACCESS_KEY_ID=tembo AWS_SECRET_ACCESS_KEY=tembo12345 aws s3 ls "s3://tembo-backup/" --endpoint-url http://localhost:9001
echo "Removing backup $1 from Minio backup path s3://tembo-backup/"
AWS_ACCESS_KEY_ID=tembo AWS_SECRET_ACCESS_KEY=tembo12345 aws s3 rm "s3://tembo-backup/$1" --recursive --endpoint-url http://localhost:9001

# Kill the port-forward process
kill $PORT_FORWARD_PID

