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
  --set-env-vars "SERVER_PORT=8080,GCS_BUCKET=rust-logi-files,DVR_NOTIFICATION_ENABLED=true,DVR_LINEWORKS_BOT_URL=https://lineworks-bot-rust-566bls5vfq-an.a.run.app,FLICKR_CONSUMER_KEY=ce52fba1099524b9eb0fb0b1913547b5,FLICKR_CONSUMER_SECRET=74de269c7d0b550f,CAM_DIGEST_USER=admin,CAM_DIGEST_PASS=Ohishi55,CAM_MACHINE_NAME=TS-NA230WP-48,CAM_SDCARD_CGI=https://car.mtamaramu.com/camera-cgi/admin/sdcard.cgi?action=generate&pagesize=1000&pagenum=1&dir=,CAM_MP4_CGI=https://car.mtamaramu.com/playmp4.cgi?storage=sd&file=/,CAM_JPG_CGI=https://car.mtamaramu.com/snapshot.cgi?storage=sd&file=/" \
  --port 8080

echo "=== Deploy complete ==="

echo "=== Running health check ==="
SERVICE_URL=$(gcloud run services describe $SERVICE_NAME --region $REGION --format 'value(status.url)')
TOKEN=$(gcloud auth print-identity-token)

# gRPC health check
HEALTH_RESPONSE=$(grpcurl -H "Authorization: Bearer $TOKEN" \
  -d '{"service": ""}' \
  ${SERVICE_URL#https://}:443 \
  grpc.health.v1.Health/Check 2>&1) || true

if echo "$HEALTH_RESPONSE" | grep -q '"status": "SERVING"'; then
  echo "✓ Health check passed: SERVING"
else
  echo "✗ Health check failed:"
  echo "$HEALTH_RESPONSE"
  exit 1
fi
