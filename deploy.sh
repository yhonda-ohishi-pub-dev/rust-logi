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
  --no-invoker-iam-check \
  --clear-cloudsql-instances \
  --set-secrets "DATABASE_URL=rust-logi-database-url:latest,JWT_SECRET=rust-logi-jwt-secret:latest,FLICKR_CONSUMER_KEY=rust-flickr-consumer-key:latest,FLICKR_CONSUMER_SECRET=rust-flickr-consumer-secret:latest,CAM_DIGEST_USER=rust-flickr-cam-digest-user:latest,CAM_DIGEST_PASS=rust-flickr-cam-digest-pass:latest" \
  --set-env-vars "^;^SERVER_PORT=8080;GCS_BUCKET=rust-logi-files;DVR_NOTIFICATION_ENABLED=true;DVR_LINEWORKS_BOT_URL=https://lineworks-bot-rust-566bls5vfq-an.a.run.app;CAM_MACHINE_NAME=TS-NA230WP-48;CAM_SDCARD_CGI=https://car.mtamaramu.com/camera-cgi/admin/sdcard.cgi?action=generate&pagesize=1000&pagenum=1&dir=;CAM_MP4_CGI=https://car.mtamaramu.com/playmp4.cgi?storage=sd&file=/;CAM_JPG_CGI=https://car.mtamaramu.com/snapshot.cgi?storage=sd&file=/;GOOGLE_CLIENT_IDS=747065218280-9httjrj60r8phntvba226vcr1m6u95tg.apps.googleusercontent.com,747065218280-uhaa6rdt54s93qemv56pkfgjc76bgpvn.apps.googleusercontent.com" \
  --port 8080

echo "=== Deploy complete ==="

echo "=== Running health check ==="
SERVICE_URL=$(gcloud run services describe $SERVICE_NAME --region $REGION --format 'value(status.url)')

# gRPC health check (no IAM token needed with --no-invoker-iam-check)
HEALTH_RESPONSE=$(grpcurl \
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
