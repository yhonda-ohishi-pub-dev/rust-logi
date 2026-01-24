use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 運行ログモデル
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DtakologModel {
    // 複合主キー
    pub data_date_time: String,
    pub vehicle_cd: i32,

    // 必須フィールド
    #[sqlx(rename = "type")]
    pub dtako_type: String,
    pub all_state_font_color_index: i32,
    pub all_state_ryout_color: String,
    pub branch_cd: i32,
    pub branch_name: String,
    pub current_work_cd: i32,
    pub data_filter_type: i32,
    pub disp_flag: i32,
    pub driver_cd: i32,
    pub gps_direction: i32,
    pub gps_enable: i32,
    pub gps_latitude: i32,
    pub gps_longitude: i32,
    pub gps_satellite_num: i32,
    pub operation_state: i32,
    pub recive_event_type: i32,
    pub recive_packet_type: i32,
    pub recive_work_cd: i32,
    pub revo: i32,
    pub setting_temp: String,
    pub setting_temp1: String,
    pub setting_temp3: String,
    pub setting_temp4: String,
    pub speed: f32,
    pub sub_driver_cd: i32,
    pub temp_state: i32,
    pub vehicle_name: String,

    // オプショナルフィールド
    pub address_disp_c: Option<String>,
    pub address_disp_p: Option<String>,
    pub all_state: Option<String>,
    pub all_state_ex: Option<String>,
    pub all_state_font_color: Option<String>,
    pub comu_date_time: Option<String>,
    pub current_work_name: Option<String>,
    pub driver_name: Option<String>,
    pub event_val: Option<String>,
    pub gps_lati_and_long: Option<String>,
    pub odometer: Option<String>,
    pub recive_type_color_name: Option<String>,
    pub recive_type_name: Option<String>,
    pub start_work_date_time: Option<String>,
    pub state: Option<String>,
    pub state1: Option<String>,
    pub state2: Option<String>,
    pub state3: Option<String>,
    pub state_flag: Option<String>,
    pub temp1: Option<String>,
    pub temp2: Option<String>,
    pub temp3: Option<String>,
    pub temp4: Option<String>,
    pub vehicle_icon_color: Option<String>,
    pub vehicle_icon_label_for_datetime: Option<String>,
    pub vehicle_icon_label_for_driver: Option<String>,
    pub vehicle_icon_label_for_vehicle: Option<String>,
}

impl DtakologModel {
    /// Protoメッセージに変換
    pub fn to_proto(&self) -> crate::proto::dtakologs::Dtakolog {
        crate::proto::dtakologs::Dtakolog {
            r#type: self.dtako_type.clone(),
            address_disp_c: self.address_disp_c.clone(),
            address_disp_p: self.address_disp_p.clone(),
            all_state: self.all_state.clone(),
            all_state_ex: self.all_state_ex.clone(),
            all_state_font_color: self.all_state_font_color.clone(),
            all_state_font_color_index: self.all_state_font_color_index,
            all_state_ryout_color: self.all_state_ryout_color.clone(),
            branch_cd: self.branch_cd,
            branch_name: self.branch_name.clone(),
            comu_date_time: self.comu_date_time.clone(),
            current_work_cd: self.current_work_cd,
            current_work_name: self.current_work_name.clone(),
            data_date_time: self.data_date_time.clone(),
            data_filter_type: self.data_filter_type,
            disp_flag: self.disp_flag,
            driver_cd: self.driver_cd,
            driver_name: self.driver_name.clone(),
            event_val: self.event_val.clone(),
            gps_direction: self.gps_direction,
            gps_enable: self.gps_enable,
            gps_lati_and_long: self.gps_lati_and_long.clone(),
            gps_latitude: self.gps_latitude,
            gps_longitude: self.gps_longitude,
            gps_satellite_num: self.gps_satellite_num,
            odometer: self.odometer.clone(),
            operation_state: self.operation_state,
            recive_event_type: self.recive_event_type,
            recive_packet_type: self.recive_packet_type,
            recive_type_color_name: self.recive_type_color_name.clone(),
            recive_type_name: self.recive_type_name.clone(),
            recive_work_cd: self.recive_work_cd,
            revo: self.revo,
            setting_temp: self.setting_temp.clone(),
            setting_temp1: self.setting_temp1.clone(),
            setting_temp3: self.setting_temp3.clone(),
            setting_temp4: self.setting_temp4.clone(),
            speed: self.speed,
            start_work_date_time: self.start_work_date_time.clone(),
            state: self.state.clone(),
            state1: self.state1.clone(),
            state2: self.state2.clone(),
            state3: self.state3.clone(),
            state_flag: self.state_flag.clone(),
            sub_driver_cd: self.sub_driver_cd,
            temp1: self.temp1.clone(),
            temp2: self.temp2.clone(),
            temp3: self.temp3.clone(),
            temp4: self.temp4.clone(),
            temp_state: self.temp_state,
            vehicle_cd: self.vehicle_cd,
            vehicle_icon_color: self.vehicle_icon_color.clone(),
            vehicle_icon_label_for_datetime: self.vehicle_icon_label_for_datetime.clone(),
            vehicle_icon_label_for_driver: self.vehicle_icon_label_for_driver.clone(),
            vehicle_icon_label_for_vehicle: self.vehicle_icon_label_for_vehicle.clone(),
            vehicle_name: self.vehicle_name.clone(),
        }
    }
}

/// Flickr OAuthセッションモデル
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FlickrOAuthSessionModel {
    pub id: String,
    pub request_token: String,
    pub request_token_secret: String,
    pub created_at: String,
    pub expires_at: String,
}

/// Flickrトークンモデル
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FlickrTokenModel {
    pub id: String,
    pub access_token: String,
    pub access_token_secret: String,
    pub user_nsid: String,
    pub username: String,
    pub created_at: String,
    pub updated_at: String,
}
