use std::sync::Arc;

use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::config::Config;
use crate::db::{get_organization_from_request, set_current_organization};
use crate::http_client::HttpClient;
use crate::proto::dvr_notifications::dvr_notifications_service_server::DvrNotificationsService;
use crate::proto::dvr_notifications::{
    BulkCreateDvrNotificationsRequest, BulkCreateDvrNotificationsResponse, DvrNotification,
};

pub struct DvrNotificationsServiceImpl {
    pool: PgPool,
    config: Config,
    http_client: Arc<HttpClient>,
}

impl DvrNotificationsServiceImpl {
    pub fn new(pool: PgPool, config: Config, http_client: Arc<HttpClient>) -> Self {
        Self {
            pool,
            config,
            http_client,
        }
    }

    /// Send LINE WORKS notification via lineworks-bot-rust
    async fn send_line_notification(&self, notification: &DvrNotification) -> Result<(), String> {
        let bot_url = match &self.config.dvr_lineworks_bot_url {
            Some(url) => url,
            None => {
                tracing::debug!("LINE WORKS bot URL not configured, skipping notification");
                return Ok(());
            }
        };

        if !self.config.dvr_notification_enabled {
            tracing::debug!("DVR notifications disabled, skipping LINE notification");
            return Ok(());
        }

        // Build message text for LINE WORKS
        let message = format!(
            "【DVR通知】\n車両: {} ({})\n運転手: {}\nイベント: {}\n日時: {}\nシリアル: {}\nファイル: {}\n動画URL: {}",
            notification.vehicle_name,
            notification.vehicle_cd,
            notification.driver_name,
            notification.event_type,
            notification.dvr_datetime,
            notification.serial_no,
            notification.file_name,
            notification.mp4_url
        );

        // Call lineworks-bot-rust API
        let payload = serde_json::json!({
            "test": "sendTextMessageLine",
            "message": message
        });

        let api_url = format!("{}/api/tasks", bot_url.trim_end_matches('/'));

        match self.http_client.post_json(&api_url, &payload).await {
            Ok(response) => {
                if response.status().is_success() {
                    tracing::info!(
                        "LINE notification sent for DVR: mp4_url={}",
                        notification.mp4_url
                    );
                    Ok(())
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    tracing::error!("LINE notification failed: {} - {}", status, body);
                    Err(format!("LINE notification failed: {}", status))
                }
            }
            Err(e) => {
                tracing::error!("Failed to send LINE notification: {}", e);
                Err(format!("Failed to send LINE notification: {}", e))
            }
        }
    }

    /// Check if a notification with the given mp4_url already exists
    async fn exists(&self, conn: &mut sqlx::PgConnection, mp4_url: &str) -> Result<bool, sqlx::Error> {
        let result: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM dvr_notifications WHERE mp4_url = $1 LIMIT 1"
        )
        .bind(mp4_url)
        .fetch_optional(conn)
        .await?;

        Ok(result.is_some())
    }
}

#[tonic::async_trait]
impl DvrNotificationsService for DvrNotificationsServiceImpl {
    /// DVR通知一括作成
    async fn bulk_create(
        &self,
        request: Request<BulkCreateDvrNotificationsRequest>,
    ) -> Result<Response<BulkCreateDvrNotificationsResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        let total_records = req.notifications.len() as i32;

        tracing::info!(
            "BulkCreate DVR notifications called for organization: {}, records: {}",
            organization_id,
            total_records
        );

        if req.notifications.is_empty() {
            return Ok(Response::new(BulkCreateDvrNotificationsResponse {
                success: true,
                records_added: 0,
                total_records: 0,
                message: "No records to insert".to_string(),
            }));
        }

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;

        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        let mut records_added = 0;
        let mut errors = Vec::new();

        for notification in req.notifications {
            // Check if mp4_url already exists
            let exists = self
                .exists(&mut conn, &notification.mp4_url)
                .await
                .map_err(|e| Status::internal(format!("Failed to check existence: {}", e)))?;

            if exists {
                tracing::debug!(
                    "DVR notification already exists, skipping: mp4_url={}",
                    notification.mp4_url
                );
                continue;
            }

            // Insert new record
            let result = sqlx::query(
                r#"
                INSERT INTO dvr_notifications (
                    organization_id, mp4_url, vehicle_cd, vehicle_name,
                    serial_no, file_name, event_type, dvr_datetime, driver_name
                ) VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(&organization_id)
            .bind(&notification.mp4_url)
            .bind(notification.vehicle_cd)
            .bind(&notification.vehicle_name)
            .bind(&notification.serial_no)
            .bind(&notification.file_name)
            .bind(&notification.event_type)
            .bind(&notification.dvr_datetime)
            .bind(&notification.driver_name)
            .execute(&mut *conn)
            .await;

            match result {
                Ok(_) => {
                    records_added += 1;
                    tracing::info!(
                        "DVR notification created: mp4_url={}, vehicle={}",
                        notification.mp4_url,
                        notification.vehicle_name
                    );

                    // Send LINE WORKS notification for the new record
                    if let Err(e) = self.send_line_notification(&notification).await {
                        tracing::warn!("LINE notification failed but record was saved: {}", e);
                    }
                }
                Err(e) => {
                    let error_msg = format!("mp4_url={}: {}", notification.mp4_url, e);
                    tracing::error!("Failed to insert DVR notification: {}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        let success = errors.is_empty();
        let message = if success {
            format!(
                "Inserted {} new records out of {} total",
                records_added, total_records
            )
        } else {
            format!(
                "Inserted {} records with {} errors: {}",
                records_added,
                errors.len(),
                errors.join("; ")
            )
        };

        Ok(Response::new(BulkCreateDvrNotificationsResponse {
            success,
            records_added,
            total_records,
            message,
        }))
    }
}
