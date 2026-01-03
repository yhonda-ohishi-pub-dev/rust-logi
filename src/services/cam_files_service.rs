use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::models::{CamFileExeModel, CamFileExeStageModel, CamFileModel};
use crate::proto::cam_files::cam_file_exe_stage_service_server::CamFileExeStageService;
use crate::proto::cam_files::cam_files_service_server::CamFilesService;
use crate::proto::cam_files::{
    CamFile, CamFileExe, CamFileExeResponse, CamFileExeStage, CreateCamFileExeRequest,
    CreateStageRequest, ListCamFileDatesResponse, ListCamFilesRequest, ListCamFilesResponse,
    ListStagesResponse, StageResponse,
};
use crate::proto::common::Empty;

pub struct CamFilesServiceImpl {
    pool: PgPool,
}

impl CamFilesServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn model_to_proto(model: &CamFileModel) -> CamFile {
        CamFile {
            name: model.name.clone(),
            date: model.date.clone(),
            hour: model.hour.clone(),
            r#type: model.file_type.clone(),
            cam: model.cam.clone(),
            flickr_id: model.flickr_id.clone(),
        }
    }
}

#[tonic::async_trait]
impl CamFilesService for CamFilesServiceImpl {
    async fn list_cam_files(
        &self,
        request: Request<ListCamFilesRequest>,
    ) -> Result<Response<ListCamFilesResponse>, Status> {
        let req = request.into_inner();

        let files = match (req.date, req.cam) {
            (Some(date), Some(cam)) => {
                sqlx::query_as::<_, CamFileModel>(
                    "SELECT * FROM cam_files WHERE date = $1 AND cam = $2 ORDER BY hour",
                )
                .bind(&date)
                .bind(&cam)
                .fetch_all(&self.pool)
                .await
            }
            (Some(date), None) => {
                sqlx::query_as::<_, CamFileModel>(
                    "SELECT * FROM cam_files WHERE date = $1 ORDER BY hour",
                )
                .bind(&date)
                .fetch_all(&self.pool)
                .await
            }
            (None, Some(cam)) => {
                sqlx::query_as::<_, CamFileModel>(
                    "SELECT * FROM cam_files WHERE cam = $1 ORDER BY date DESC, hour",
                )
                .bind(&cam)
                .fetch_all(&self.pool)
                .await
            }
            (None, None) => {
                sqlx::query_as::<_, CamFileModel>(
                    "SELECT * FROM cam_files ORDER BY date DESC, hour LIMIT 100",
                )
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_files: Vec<CamFile> = files.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListCamFilesResponse {
            files: proto_files,
            pagination: None,
        }))
    }

    async fn list_cam_file_dates(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListCamFileDatesResponse>, Status> {
        let dates: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT date FROM cam_files ORDER BY date DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ListCamFileDatesResponse {
            dates: dates.into_iter().map(|(d,)| d).collect(),
        }))
    }

    async fn create_cam_file_exe(
        &self,
        request: Request<CreateCamFileExeRequest>,
    ) -> Result<Response<CamFileExeResponse>, Status> {
        let req = request.into_inner();
        let exe = req
            .exe
            .ok_or_else(|| Status::invalid_argument("exe is required"))?;

        let result = sqlx::query_as::<_, CamFileExeModel>(
            r#"
            INSERT INTO cam_file_exe (name, cam, stage)
            VALUES ($1, $2, $3)
            ON CONFLICT (name, cam) DO UPDATE SET stage = $3
            RETURNING *
            "#,
        )
        .bind(&exe.name)
        .bind(&exe.cam)
        .bind(exe.stage)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(CamFileExeResponse {
            exe: Some(CamFileExe {
                name: result.name,
                cam: result.cam,
                stage: result.stage,
            }),
        }))
    }
}

pub struct CamFileExeStageServiceImpl {
    pool: PgPool,
}

impl CamFileExeStageServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[tonic::async_trait]
impl CamFileExeStageService for CamFileExeStageServiceImpl {
    async fn list_stages(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListStagesResponse>, Status> {
        let stages = sqlx::query_as::<_, CamFileExeStageModel>(
            "SELECT * FROM cam_file_exe_stage ORDER BY stage",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_stages: Vec<CamFileExeStage> = stages
            .iter()
            .map(|s| CamFileExeStage {
                stage: s.stage,
                name: s.name.clone(),
            })
            .collect();

        Ok(Response::new(ListStagesResponse {
            stages: proto_stages,
        }))
    }

    async fn create_stage(
        &self,
        request: Request<CreateStageRequest>,
    ) -> Result<Response<StageResponse>, Status> {
        let req = request.into_inner();
        let stage = req
            .stage
            .ok_or_else(|| Status::invalid_argument("stage is required"))?;

        let result = sqlx::query_as::<_, CamFileExeStageModel>(
            r#"
            INSERT INTO cam_file_exe_stage (stage, name)
            VALUES ($1, $2)
            ON CONFLICT (stage) DO UPDATE SET name = $2
            RETURNING *
            "#,
        )
        .bind(stage.stage)
        .bind(&stage.name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(StageResponse {
            stage: Some(CamFileExeStage {
                stage: result.stage,
                name: result.name,
            }),
        }))
    }
}
