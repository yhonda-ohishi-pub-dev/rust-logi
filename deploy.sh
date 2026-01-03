#!/bin/bash
set -e

PROJECT_ID="cloudsql-sv"
REGION="asia-northeast1"
SERVICE_NAME="rust-logi"
REPOSITORY="rust-logi"
IMAGE="$REGION-docker.pkg.dev/$PROJECT_ID/$REPOSITORY/$SERVICE_NAME"

echo "=== Building Docker image ==="
docker build -t $IMAGE:latest .

echo "=== Pushing to Artifact Registry ==="
docker push $IMAGE:latest

echo "=== Deploying to Cloud Run ==="
gcloud run deploy $SERVICE_NAME \
  --image $IMAGE:latest \
  --region $REGION \
  --platform managed \
  --no-allow-unauthenticated \
  --add-cloudsql-instances cloudsql-sv:asia-northeast1:postgres-prod \
  --set-secrets "DATABASE_URL=rust-logi-database-url:latest" \
  --set-env-vars "SERVER_PORT=8080" \
  --port 8080

echo "=== Deploy complete ==="
