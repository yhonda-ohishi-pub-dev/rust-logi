use sqlx::PgPool;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::models::FileModel;
use crate::proto::common::Empty;
use crate::proto::files::files_service_server::FilesService;
use crate::proto::files::{
    CreateFileRequest, DeleteFileRequest, DownloadFileRequest, File, FileChunk, FileResponse,
    GetFileRequest, ListFilesRequest, ListFilesResponse,
};

pub struct FilesServiceImpl {
    pool: PgPool,
}

impl FilesServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn model_to_proto(model: &FileModel) -> File {
        File {
            uuid: model.uuid.clone(),
            filename: model.filename.clone(),
            r#type: model.file_type.clone(),
            created: model.created.clone(),
            deleted: model.deleted.clone(),
            blob: model.blob.clone(),
        }
    }
}

#[tonic::async_trait]
impl FilesService for FilesServiceImpl {
    async fn create_file(
        &self,
        request: Request<CreateFileRequest>,
    ) -> Result<Response<FileResponse>, Status> {
        let req = request.into_inner();
        let uuid = Uuid::new_v4().to_string();
        let created = chrono::Utc::now().to_rfc3339();

        // Convert content to base64 if provided
        let blob = if !req.content.is_empty() {
            Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &req.content,
            ))
        } else {
            req.blob_base64
        };

        let result = sqlx::query_as::<_, FileModel>(
            r#"
            INSERT INTO files (uuid, filename, type, created, blob)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING uuid, filename, type, created, deleted, blob
            "#,
        )
        .bind(&uuid)
        .bind(&req.filename)
        .bind(&req.r#type)
        .bind(&created)
        .bind(&blob)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(FileResponse {
            file: Some(Self::model_to_proto(&result)),
        }))
    }

    async fn list_files(
        &self,
        request: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        let req = request.into_inner();

        let files = if let Some(type_filter) = req.type_filter {
            sqlx::query_as::<_, FileModel>(
                r#"
                SELECT uuid, filename, type, created, deleted, NULL as blob
                FROM files
                WHERE deleted IS NULL AND type = $1
                ORDER BY created DESC
                "#,
            )
            .bind(&type_filter)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, FileModel>(
                r#"
                SELECT uuid, filename, type, created, deleted, NULL as blob
                FROM files
                WHERE deleted IS NULL
                ORDER BY created DESC
                "#,
            )
            .fetch_all(&self.pool)
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
        let req = request.into_inner();

        let query = if req.include_blob {
            "SELECT uuid, filename, type, created, deleted, blob FROM files WHERE uuid = $1"
        } else {
            "SELECT uuid, filename, type, created, deleted, NULL as blob FROM files WHERE uuid = $1"
        };

        let file = sqlx::query_as::<_, FileModel>(query)
            .bind(&req.uuid)
            .fetch_optional(&self.pool)
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
        let req = request.into_inner();

        let file = sqlx::query_as::<_, FileModel>(
            "SELECT uuid, filename, type, created, deleted, blob FROM files WHERE uuid = $1",
        )
        .bind(&req.uuid)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?
        .ok_or_else(|| Status::not_found(format!("File not found: {}", req.uuid)))?;

        let (tx, rx) = tokio::sync::mpsc::channel(4);

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
        let req = request.into_inner();
        let deleted = chrono::Utc::now().to_rfc3339();

        sqlx::query("UPDATE files SET deleted = $1 WHERE uuid = $2")
            .bind(&deleted)
            .bind(&req.uuid)
            .execute(&self.pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(Empty {}))
    }

    async fn list_not_attached_files(
        &self,
        _request: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        // Files that are not attached to any car inspection
        let files = sqlx::query_as::<_, FileModel>(
            r#"
            SELECT f.uuid, f.filename, f.type, f.created, f.deleted, NULL as blob
            FROM files f
            LEFT JOIN car_inspection_files_a cif ON f.uuid = cif.uuid
            WHERE f.deleted IS NULL AND cif.uuid IS NULL
            ORDER BY f.created DESC
            "#,
        )
        .fetch_all(&self.pool)
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
        _request: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        let files = sqlx::query_as::<_, FileModel>(
            r#"
            SELECT uuid, filename, type, created, deleted, NULL as blob
            FROM files
            WHERE deleted IS NULL
            ORDER BY created DESC
            LIMIT 50
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_files: Vec<File> = files.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListFilesResponse {
            files: proto_files,
            pagination: None,
        }))
    }
}
