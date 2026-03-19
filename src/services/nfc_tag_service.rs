use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::db::{get_organization_from_request, set_current_organization};
use crate::models::{CarInspectionModel, NfcTagModel};
use crate::proto::car_inspection::nfc_tag_service_server::NfcTagService;
use crate::proto::car_inspection::{
    DeleteNfcTagRequest, ListNfcTagsRequest, ListNfcTagsResponse, NfcTag, NfcTagResponse,
    RegisterNfcTagRequest, SearchByNfcUuidRequest, SearchByNfcUuidResponse,
};
use crate::proto::common::Empty;
use crate::services::car_inspection_service::CarInspectionServiceImpl;

/// NFC UUID を正規化: 小文字、コロン除去
fn normalize_nfc_uuid(uuid: &str) -> String {
    uuid.to_lowercase().replace(':', "")
}

fn model_to_proto(model: &NfcTagModel) -> NfcTag {
    NfcTag {
        id: model.id,
        nfc_uuid: model.nfc_uuid.clone(),
        car_inspection_id: model.car_inspection_id,
        created_at: model.created_at.to_rfc3339(),
    }
}

pub struct NfcTagServiceImpl {
    pool: PgPool,
}

impl NfcTagServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[tonic::async_trait]
impl NfcTagService for NfcTagServiceImpl {
    async fn search_by_nfc_uuid(
        &self,
        request: Request<SearchByNfcUuidRequest>,
    ) -> Result<Response<SearchByNfcUuidResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        let nfc_uuid = normalize_nfc_uuid(&req.nfc_uuid);

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        // JOIN nfc_tags with car_inspection
        let tag_row = sqlx::query_as::<_, NfcTagModel>(
            "SELECT id, nfc_uuid, car_inspection_id, created_at FROM car_inspection_nfc_tags WHERE nfc_uuid = $1",
        )
        .bind(&nfc_uuid)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match tag_row {
            Some(tag) => {
                // Fetch the associated car inspection
                let ci = sqlx::query_as::<_, CarInspectionModel>(
                    "SELECT * FROM car_inspection WHERE id = $1",
                )
                .bind(tag.car_inspection_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

                Ok(Response::new(SearchByNfcUuidResponse {
                    car_inspection: ci.map(|m| CarInspectionServiceImpl::model_to_proto(&m)),
                    nfc_tag: Some(model_to_proto(&tag)),
                }))
            }
            None => Ok(Response::new(SearchByNfcUuidResponse {
                car_inspection: None,
                nfc_tag: None,
            })),
        }
    }

    async fn register_nfc_tag(
        &self,
        request: Request<RegisterNfcTagRequest>,
    ) -> Result<Response<NfcTagResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        let nfc_uuid = normalize_nfc_uuid(&req.nfc_uuid);

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        let tag = sqlx::query_as::<_, NfcTagModel>(
            r#"
            INSERT INTO car_inspection_nfc_tags (organization_id, nfc_uuid, car_inspection_id)
            VALUES (current_setting('app.current_organization_id')::uuid, $1, $2)
            ON CONFLICT (organization_id, nfc_uuid) DO UPDATE
                SET car_inspection_id = EXCLUDED.car_inspection_id,
                    created_at = NOW()
            RETURNING id, nfc_uuid, car_inspection_id, created_at
            "#,
        )
        .bind(&nfc_uuid)
        .bind(req.car_inspection_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(NfcTagResponse {
            nfc_tag: Some(model_to_proto(&tag)),
        }))
    }

    async fn list_nfc_tags(
        &self,
        request: Request<ListNfcTagsRequest>,
    ) -> Result<Response<ListNfcTagsResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        let tags = if let Some(car_inspection_id) = req.car_inspection_id {
            sqlx::query_as::<_, NfcTagModel>(
                "SELECT id, nfc_uuid, car_inspection_id, created_at FROM car_inspection_nfc_tags WHERE car_inspection_id = $1 ORDER BY created_at DESC",
            )
            .bind(car_inspection_id)
            .fetch_all(&mut *conn)
            .await
        } else {
            sqlx::query_as::<_, NfcTagModel>(
                "SELECT id, nfc_uuid, car_inspection_id, created_at FROM car_inspection_nfc_tags ORDER BY created_at DESC",
            )
            .fetch_all(&mut *conn)
            .await
        }
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ListNfcTagsResponse {
            nfc_tags: tags.iter().map(model_to_proto).collect(),
        }))
    }

    async fn delete_nfc_tag(
        &self,
        request: Request<DeleteNfcTagRequest>,
    ) -> Result<Response<Empty>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        let nfc_uuid = normalize_nfc_uuid(&req.nfc_uuid);

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        sqlx::query("DELETE FROM car_inspection_nfc_tags WHERE nfc_uuid = $1")
            .bind(&nfc_uuid)
            .execute(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(Empty {}))
    }
}
