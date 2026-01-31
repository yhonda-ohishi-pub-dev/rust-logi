use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CarInspectionFileModel {
    pub uuid: uuid::Uuid,
    #[sqlx(rename = "type")]
    pub file_type: String,
    #[sqlx(rename = "ElectCertMgNo")]
    pub elect_cert_mg_no: String,
    #[sqlx(rename = "GrantdateE")]
    pub grantdate_e: String,
    #[sqlx(rename = "GrantdateY")]
    pub grantdate_y: String,
    #[sqlx(rename = "GrantdateM")]
    pub grantdate_m: String,
    #[sqlx(rename = "GrantdateD")]
    pub grantdate_d: String,
    #[sqlx(rename = "created_at")]
    pub created: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "modified_at")]
    pub modified: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(rename = "deleted_at")]
    pub deleted: Option<chrono::DateTime<chrono::Utc>>,
}

impl CarInspectionFileModel {
    pub fn new(
        uuid: uuid::Uuid,
        file_type: String,
        elect_cert_mg_no: String,
        grantdate_e: String,
        grantdate_y: String,
        grantdate_m: String,
        grantdate_d: String,
    ) -> Self {
        Self {
            uuid,
            file_type,
            elect_cert_mg_no,
            grantdate_e,
            grantdate_y,
            grantdate_m,
            grantdate_d,
            created: chrono::Utc::now(),
            modified: None,
            deleted: None,
        }
    }
}
