use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DvrNotificationModel {
    pub mp4_url: String,
    pub vehicle_cd: i64,
    pub vehicle_name: String,
    pub serial_no: String,
    pub file_name: String,
    pub event_type: String,
    pub dvr_datetime: String,
    pub driver_name: String,
}

impl DvrNotificationModel {
    pub fn to_proto(&self) -> crate::proto::dvr_notifications::DvrNotification {
        crate::proto::dvr_notifications::DvrNotification {
            vehicle_cd: self.vehicle_cd,
            vehicle_name: self.vehicle_name.clone(),
            serial_no: self.serial_no.clone(),
            file_name: self.file_name.clone(),
            event_type: self.event_type.clone(),
            dvr_datetime: self.dvr_datetime.clone(),
            driver_name: self.driver_name.clone(),
            mp4_url: self.mp4_url.clone(),
        }
    }
}
