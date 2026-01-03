#!/bin/bash

# Cloud SQL Auth Proxy startup script
# Requires: gcloud auth application-default login

INSTANCE="cloudsql-sv:asia-northeast1:postgres-prod"
PORT=5432
PROXY_PATH="./cloud-sql-proxy"

# Download Cloud SQL Proxy if not exists
if [ ! -f "$PROXY_PATH" ]; then
    echo "Cloud SQL Proxy not found. Downloading..."
    curl -o "$PROXY_PATH" https://storage.googleapis.com/cloud-sql-connectors/cloud-sql-proxy/v2.14.3/cloud-sql-proxy.linux.amd64
    chmod +x "$PROXY_PATH"
    echo "Downloaded to $PROXY_PATH"
fi

echo "Starting Cloud SQL Proxy for $INSTANCE on port $PORT..."
echo "Press Ctrl+C to stop"

"$PROXY_PATH" --port $PORT $INSTANCE
