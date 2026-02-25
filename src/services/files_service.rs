use std::sync::Arc;

use sqlx::PgPool;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::db::{get_organization_from_request, set_current_organization, DEFAULT_ORGANIZATION_ID};
use crate::models::FileModel;
use crate::proto::common::Empty;
use crate::proto::files::files_service_server::FilesService;
use crate::proto::files::{
    CreateFileRequest, DeleteFileRequest, DownloadFileRequest, File, FileChunk, FileResponse,
    GetFileRequest, ListFilesRequest, ListFilesResponse, RestoreFileRequest, RestoreFileResponse,
};
use crate::services::file_auto_parser::FileAutoParser;
use crate::storage::{StorageBackend, RestoreStatus};

pub struct FilesServiceImpl {
    pool: PgPool,
    storage: Option<Arc<dyn StorageBackend>>,
    file_auto_parser: Arc<FileAutoParser>,
}

impl FilesServiceImpl {
    pub fn new(pool: PgPool, storage: Option<Arc<dyn StorageBackend>>, file_auto_parser: Arc<FileAutoParser>) -> Self {
        Self { pool, storage, file_auto_parser }
    }

    fn model_to_proto(model: &FileModel) -> File {
        File {
            uuid: model.uuid.clone(),
            filename: model.filename.clone(),
            r#type: model.file_type.clone(),
            created: model.created.clone(),
            deleted: model.deleted.clone(),
            blob: model.blob.clone(),
            // GCS fields
            s3_key: model.s3_key.clone(),
            storage_class: model.storage_class.clone(),
            last_accessed_at: model.last_accessed_at.clone(),
        }
    }

    /// GCSキーを生成（organization_id/uuid形式）
    fn generate_gcs_key(organization_id: &str, uuid: &str) -> String {
        format!("{}/{}", organization_id, uuid)
    }

    /// アクセスを記録し、条件を満たせばSTANDARDに昇格
    /// - 直近7日で3回以上アクセス → STANDARDにrewrite
    async fn record_access_and_maybe_promote(
        &self,
        gcs_key: &str,
        uuid: &str,
        organization_id: &str,
        current_storage_class: Option<&str>,
    ) {
        let pool = self.pool.clone();
        let storage = self.storage.clone();
        let gcs_key = gcs_key.to_string();
        let uuid = uuid.to_string();
        let organization_id = organization_id.to_string();
        let storage_class = current_storage_class.map(|s| s.to_string());

        tokio::spawn(async move {
            // アクセスを記録し、カウントを取得
            let access_result = sqlx::query_as::<_, crate::models::FileAccessResult>(
                "SELECT * FROM record_file_access($1::uuid, $2::uuid, $3)",
            )
            .bind(&uuid)
            .bind(&organization_id)
            .bind(&storage_class)
            .fetch_one(&pool)
            .await;

            match access_result {
                Ok(result) => {
                    tracing::debug!(
                        "File access recorded: uuid={}, weekly={}, total={}, recent_7day={}",
                        uuid,
                        result.weekly_count,
                        result.total_count,
                        result.recent_7day_count
                    );

                    // 直近7日で3回以上 && STANDARDでない場合は昇格
                    let should_promote = result.recent_7day_count >= 3
                        && storage_class.as_deref() != Some("STANDARD");

                    if should_promote {
                        if let Some(storage) = storage {
                            match storage.rewrite_to_standard(&gcs_key).await {
                                Ok(_) => {
                                    tracing::info!(
                                        "Promoted to STANDARD: uuid={}, access_count_7day={}",
                                        uuid,
                                        result.recent_7day_count
                                    );

                                    // DB更新
                                    let now = chrono::Utc::now();
                                    if let Err(e) = sqlx::query(
                                        "UPDATE files SET storage_class = 'STANDARD', promoted_to_standard_at = $1 WHERE uuid = $2::uuid",
                                    )
                                    .bind(&now)
                                    .bind(&uuid)
                                    .execute(&pool)
                                    .await
                                    {
                                        tracing::error!(
                                            "Failed to update storage_class: uuid={}, error={}",
                                            uuid,
                                            e
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to promote to STANDARD: uuid={}, error={}",
                                        uuid,
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to record file access: uuid={}, error={}", uuid, e);
                }
            }
        });
    }
}

#[tonic::async_trait]
impl FilesService for FilesServiceImpl {
    async fn create_file(
        &self,
        request: Request<CreateFileRequest>,
    ) -> Result<Response<FileResponse>, Status> {
        // Extract organization_id from gRPC metadata before consuming request
        // Falls back to DEFAULT_ORGANIZATION_ID if not provided
        let organization_id = get_organization_from_request(&request);
        if organization_id == DEFAULT_ORGANIZATION_ID {
            tracing::debug!("Using default organization_id for file upload");
        }
        let req = request.into_inner();
        let uuid = Uuid::new_v4().to_string();
        let created = chrono::Utc::now();

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        tracing::info!(
            "Creating file: uuid={}, filename={}, org={}",
            uuid,
            req.filename,
            organization_id
        );

        // GCSが有効な場合はGCSにアップロード
        if let Some(storage) = &self.storage {
            let gcs_key = Self::generate_gcs_key(&organization_id, &uuid);

            // ファイルデータを取得
            let data = if !req.content.is_empty() {
                req.content
            } else if let Some(blob_base64) = &req.blob_base64 {
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, blob_base64)
                    .map_err(|e| Status::invalid_argument(format!("Invalid base64: {}", e)))?
            } else {
                return Err(Status::invalid_argument("No content or blob_base64 provided"));
            };

            // ストレージにアップロード
            storage
                .upload(&gcs_key, &data, &req.r#type)
                .await
                .map_err(|e| Status::internal(format!("GCS upload failed: {}", e)))?;

            // DBにメタデータのみ保存（blobはNULL）
            let result = sqlx::query_as::<_, FileModel>(
                r#"
                INSERT INTO files (uuid, organization_id, filename, type, created_at, s3_key, storage_class, last_accessed_at)
                VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, 'STANDARD', $5)
                RETURNING uuid::text, filename, type as file_type,
                          to_char(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                          to_char(deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                          NULL as blob, s3_key, storage_class,
                          to_char(last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                          access_count_weekly, access_count_total,
                          to_char(promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
                "#,
            )
            .bind(&uuid)
            .bind(&organization_id)
            .bind(&req.filename)
            .bind(&req.r#type)
            .bind(&created)
            .bind(&gcs_key)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

            // 自動解析（バックグラウンド）— JSON or PDF
            if req.r#type == "application/json" {
                let parser = self.file_auto_parser.clone();
                let uuid_clone = uuid.clone();
                let org_clone = organization_id.clone();
                tokio::spawn(async move {
                    if let Err(e) = parser.process_json_upload(&uuid_clone, &data, &org_clone).await {
                        tracing::error!("JSON auto-parse failed for {}: {}", uuid_clone, e);
                    }
                });
            } else if req.r#type == "application/pdf" {
                let parser = self.file_auto_parser.clone();
                let uuid_clone = uuid.clone();
                let org_clone = organization_id.clone();
                tokio::spawn(async move {
                    if let Err(e) = parser.process_pdf_upload(&uuid_clone, &data, &org_clone).await {
                        tracing::error!("PDF auto-parse failed for {}: {}", uuid_clone, e);
                    }
                });
            }

            return Ok(Response::new(FileResponse {
                file: Some(Self::model_to_proto(&result)),
            }));
        }

        // GCSが無効な場合は従来通りDBにblobを保存
        let raw_content = req.content;
        let blob = if !raw_content.is_empty() {
            Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &raw_content,
            ))
        } else {
            req.blob_base64
        };

        let result = sqlx::query_as::<_, FileModel>(
            r#"
            INSERT INTO files (uuid, organization_id, filename, type, created_at, blob)
            VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6)
            RETURNING uuid::text, filename, type as file_type,
                      to_char(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                      to_char(deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                      blob, s3_key, storage_class,
                      to_char(last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                      access_count_weekly, access_count_total,
                      to_char(promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
            "#,
        )
        .bind(&uuid)
        .bind(&organization_id)
        .bind(&req.filename)
        .bind(&req.r#type)
        .bind(&created)
        .bind(&blob)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        // 自動解析（バックグラウンド）— JSON or PDF
        if req.r#type == "application/json" && !raw_content.is_empty() {
            let parser = self.file_auto_parser.clone();
            let uuid_clone = uuid.clone();
            let org_clone = organization_id.clone();
            tokio::spawn(async move {
                if let Err(e) = parser.process_json_upload(&uuid_clone, &raw_content, &org_clone).await {
                    tracing::error!("JSON auto-parse failed for {}: {}", uuid_clone, e);
                }
            });
        } else if req.r#type == "application/pdf" && !raw_content.is_empty() {
            let parser = self.file_auto_parser.clone();
            let uuid_clone = uuid.clone();
            let org_clone = organization_id.clone();
            tokio::spawn(async move {
                if let Err(e) = parser.process_pdf_upload(&uuid_clone, &raw_content, &org_clone).await {
                    tracing::error!("PDF auto-parse failed for {}: {}", uuid_clone, e);
                }
            });
        }

        Ok(Response::new(FileResponse {
            file: Some(Self::model_to_proto(&result)),
        }))
    }

    async fn list_files(
        &self,
        request: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        let files = if let Some(type_filter) = req.type_filter {
            sqlx::query_as::<_, FileModel>(
                r#"
                SELECT uuid::text, filename, type as file_type,
                       to_char(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                       to_char(deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                       NULL as blob, s3_key, storage_class,
                       to_char(last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                       access_count_weekly, access_count_total,
                       to_char(promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
                FROM files
                WHERE deleted_at IS NULL AND type = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(&type_filter)
            .fetch_all(&mut *conn)
            .await
        } else {
            sqlx::query_as::<_, FileModel>(
                r#"
                SELECT uuid::text, filename, type as file_type,
                       to_char(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                       to_char(deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                       NULL as blob, s3_key, storage_class,
                       to_char(last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                       access_count_weekly, access_count_total,
                       to_char(promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
                FROM files
                WHERE deleted_at IS NULL
                ORDER BY created_at DESC
                "#,
            )
            .fetch_all(&mut *conn)
            .await
        }
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_files: Vec<File> = files.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListFilesResponse {
            files: proto_files,
            pagination: None,
        }))
    }

    async fn get_file(
        &self,
        request: Request<GetFileRequest>,
    ) -> Result<Response<FileResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        let query = if req.include_blob {
            r#"
            SELECT uuid::text, filename, type as file_type,
                   to_char(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                   to_char(deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                   blob, s3_key, storage_class,
                   to_char(last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                   access_count_weekly, access_count_total,
                   to_char(promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
            FROM files WHERE uuid = $1::uuid
            "#
        } else {
            r#"
            SELECT uuid::text, filename, type as file_type,
                   to_char(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                   to_char(deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                   NULL as blob, s3_key, storage_class,
                   to_char(last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                   access_count_weekly, access_count_total,
                   to_char(promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
            FROM files WHERE uuid = $1::uuid
            "#
        };

        let file = sqlx::query_as::<_, FileModel>(query)
            .bind(&req.uuid)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::not_found(format!("File not found: {}", req.uuid)))?;

        Ok(Response::new(FileResponse {
            file: Some(Self::model_to_proto(&file)),
        }))
    }

    type DownloadFileStream = tokio_stream::wrappers::ReceiverStream<Result<FileChunk, Status>>;

    async fn download_file(
        &self,
        request: Request<DownloadFileRequest>,
    ) -> Result<Response<Self::DownloadFileStream>, Status> {
        // Extract organization_id from gRPC metadata before consuming request
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        let file = sqlx::query_as::<_, FileModel>(
            r#"
            SELECT uuid::text, filename, type as file_type,
                   to_char(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                   to_char(deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                   blob, s3_key, storage_class,
                   to_char(last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                   access_count_weekly, access_count_total,
                   to_char(promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
            FROM files WHERE uuid = $1::uuid
            "#,
        )
        .bind(&req.uuid)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?
        .ok_or_else(|| Status::not_found(format!("File not found: {}", req.uuid)))?;

        let (tx, rx) = tokio::sync::mpsc::channel(4);

        // ストレージからダウンロード
        if let (Some(storage), Some(gcs_key)) = (&self.storage, &file.s3_key) {
            // オブジェクト情報を取得
            let info = storage
                .get_object_info(gcs_key)
                .await
                .map_err(|e| Status::internal(format!("Storage error: {}", e)))?;

            // ストレージからダウンロード
            let data = storage
                .download(gcs_key)
                .await
                .map_err(|e| Status::internal(format!("Storage download failed: {}", e)))?;

            let total_size = data.len() as i64;
            let chunk_size = 64 * 1024; // 64KB chunks

            // アクセスを記録し、条件を満たせばSTANDARDに昇格
            self.record_access_and_maybe_promote(
                gcs_key,
                &file.uuid,
                &organization_id,
                info.storage_class.as_deref(),
            )
            .await;

            tokio::spawn(async move {
                let mut offset = 0i64;
                for chunk in data.chunks(chunk_size) {
                    let file_chunk = FileChunk {
                        data: chunk.to_vec(),
                        offset,
                        total_size,
                    };
                    if tx.send(Ok(file_chunk)).await.is_err() {
                        break;
                    }
                    offset += chunk.len() as i64;
                }
            });

            return Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
                rx,
            )));
        }

        // 従来のblobからダウンロード（後方互換）
        if let Some(blob) = file.blob {
            let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &blob)
                .map_err(|e| Status::internal(format!("Failed to decode blob: {}", e)))?;

            let total_size = data.len() as i64;
            let chunk_size = 64 * 1024; // 64KB chunks

            tokio::spawn(async move {
                let mut offset = 0i64;
                for chunk in data.chunks(chunk_size) {
                    let file_chunk = FileChunk {
                        data: chunk.to_vec(),
                        offset,
                        total_size,
                    };
                    if tx.send(Ok(file_chunk)).await.is_err() {
                        break;
                    }
                    offset += chunk.len() as i64;
                }
            });
        }

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    async fn delete_file(
        &self,
        request: Request<DeleteFileRequest>,
    ) -> Result<Response<Empty>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        let deleted = chrono::Utc::now();

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        // ソフトデリート（GCSからは削除しない）
        sqlx::query("UPDATE files SET deleted_at = $1 WHERE uuid = $2::uuid")
            .bind(&deleted)
            .bind(&req.uuid)
            .execute(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(Empty {}))
    }

    async fn list_not_attached_files(
        &self,
        request: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        let organization_id = get_organization_from_request(&request);

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        // Files that are not attached to any car inspection
        let files = sqlx::query_as::<_, FileModel>(
            r#"
            SELECT f.uuid::text, f.filename, f.type as file_type,
                   to_char(f.created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                   to_char(f.deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                   NULL as blob, f.s3_key, f.storage_class,
                   to_char(f.last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                   f.access_count_weekly, f.access_count_total,
                   to_char(f.promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
            FROM files f
            LEFT JOIN car_inspection_files_a cif ON f.uuid = cif.uuid
            WHERE f.deleted_at IS NULL AND cif.uuid IS NULL
            ORDER BY f.created_at DESC
            "#,
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_files: Vec<File> = files.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListFilesResponse {
            files: proto_files,
            pagination: None,
        }))
    }

    async fn list_recent_uploaded_files(
        &self,
        request: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        let organization_id = get_organization_from_request(&request);

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        let files = sqlx::query_as::<_, FileModel>(
            r#"
            SELECT uuid::text, filename, type as file_type,
                   to_char(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                   to_char(deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                   NULL as blob, s3_key, storage_class,
                   to_char(last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                   access_count_weekly, access_count_total,
                   to_char(promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
            FROM files
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT 50
            "#,
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_files: Vec<File> = files.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListFilesResponse {
            files: proto_files,
            pagination: None,
        }))
    }

    /// ストレージクラスの変更をリクエスト（GCSでは即座にアクセス可能なので主にコスト最適化用）
    async fn restore_file(
        &self,
        request: Request<RestoreFileRequest>,
    ) -> Result<Response<RestoreFileResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        // ファイル情報を取得
        let file = sqlx::query_as::<_, FileModel>(
            r#"
            SELECT uuid::text, filename, type as file_type,
                   to_char(created_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as created,
                   to_char(deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as deleted,
                   NULL as blob, s3_key, storage_class,
                   to_char(last_accessed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as last_accessed_at,
                   access_count_weekly, access_count_total,
                   to_char(promoted_to_standard_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as promoted_to_standard_at
            FROM files WHERE uuid = $1::uuid
            "#,
        )
        .bind(&req.uuid)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?
        .ok_or_else(|| Status::not_found(format!("File not found: {}", req.uuid)))?;

        let Some(storage) = &self.storage else {
            return Err(Status::failed_precondition("Storage backend not configured"));
        };

        let Some(gcs_key) = &file.s3_key else {
            return Err(Status::failed_precondition(
                "File is stored in database, not object storage",
            ));
        };

        // オブジェクト情報を取得
        let info = storage
            .get_object_info(gcs_key)
            .await
            .map_err(|e| Status::internal(format!("Storage error: {}", e)))?;

        // GCSではすべてのストレージクラスが即座にアクセス可能
        let (restore_status, message) = match info.restore_status {
            RestoreStatus::NotNeeded => {
                ("NOT_NEEDED".to_string(), "File is accessible immediately (GCS does not require restoration)".to_string())
            }
            _ => {
                ("NOT_NEEDED".to_string(), "File is accessible immediately (GCS does not require restoration)".to_string())
            }
        };

        Ok(Response::new(RestoreFileResponse {
            uuid: file.uuid,
            restore_status,
            message,
            storage_class: info.storage_class,
        }))
    }
}
