use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// car_ins_sheet_ichiban_cars_a table model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CarInsSheetIchibanCarsAModel {
    pub id: i32,
    pub id_cars: Option<String>,
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
}

/// dtako_cars_ichiban_cars table model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DtakoCarsIchibanCarsModel {
    pub id_dtako: String,
    pub id: Option<String>,
}

/// External API response for home car list
#[derive(Debug, Clone, Deserialize)]
pub struct HomeCarEntry {
    #[serde(rename = "VehicleCD")]
    pub vehicle_cd: i64,
    #[serde(rename = "VehicleName")]
    pub vehicle_name: Option<String>,
    #[serde(rename = "AllState")]
    pub all_state: Option<String>,
}

/// Combined model for the ListRenewHomeTargets query result
#[derive(Debug, Clone, FromRow)]
pub struct CarInspectionWithRelationsModel {
    // CarInspection fields
    #[sqlx(rename = "CertInfoImportFileVersion")]
    pub cert_info_import_file_version: String,
    #[sqlx(rename = "Acceptoutputno")]
    pub acceptoutputno: String,
    #[sqlx(rename = "FormType")]
    pub form_type: String,
    #[sqlx(rename = "ElectCertMgNo")]
    pub elect_cert_mg_no: String,
    #[sqlx(rename = "CarId")]
    pub car_id: String,

    #[sqlx(rename = "ElectCertPublishdateE")]
    pub elect_cert_publishdate_e: String,
    #[sqlx(rename = "ElectCertPublishdateY")]
    pub elect_cert_publishdate_y: String,
    #[sqlx(rename = "ElectCertPublishdateM")]
    pub elect_cert_publishdate_m: String,
    #[sqlx(rename = "ElectCertPublishdateD")]
    pub elect_cert_publishdate_d: String,

    #[sqlx(rename = "GrantdateE")]
    pub grantdate_e: String,
    #[sqlx(rename = "GrantdateY")]
    pub grantdate_y: String,
    #[sqlx(rename = "GrantdateM")]
    pub grantdate_m: String,
    #[sqlx(rename = "GrantdateD")]
    pub grantdate_d: String,

    #[sqlx(rename = "TranspotationBureauchiefName")]
    pub transpotation_bureauchiefname: String,
    #[sqlx(rename = "EntryNoCarNo")]
    pub entry_no_car_no: String,

    #[sqlx(rename = "ReggrantdateE")]
    pub reggrantdate_e: String,
    #[sqlx(rename = "ReggrantdateY")]
    pub reggrantdate_y: String,
    #[sqlx(rename = "ReggrantdateM")]
    pub reggrantdate_m: String,
    #[sqlx(rename = "ReggrantdateD")]
    pub reggrantdate_d: String,

    #[sqlx(rename = "FirstregistdateE")]
    pub firstregistdate_e: String,
    #[sqlx(rename = "FirstregistdateY")]
    pub firstregistdate_y: String,
    #[sqlx(rename = "FirstregistdateM")]
    pub firstregistdate_m: String,

    #[sqlx(rename = "CarName")]
    pub car_name: String,
    #[sqlx(rename = "CarNameCode")]
    pub car_name_code: String,
    #[sqlx(rename = "CarNo")]
    pub car_no: String,
    #[sqlx(rename = "Model")]
    pub model: String,
    #[sqlx(rename = "EngineModel")]
    pub engine_model: String,

    #[sqlx(rename = "OwnernameLowLevelChar")]
    pub ownername_low_level_char: String,
    #[sqlx(rename = "OwnernameHighLevelChar")]
    pub ownername_high_level_char: String,
    #[sqlx(rename = "OwnerAddressChar")]
    pub owner_address_char: String,
    #[sqlx(rename = "OwnerAddressNumValue")]
    pub owner_address_num_value: String,
    #[sqlx(rename = "OwnerAddressCode")]
    pub owner_address_code: String,

    #[sqlx(rename = "UsernameLowLevelChar")]
    pub username_low_level_char: String,
    #[sqlx(rename = "UsernameHighLevelChar")]
    pub username_high_level_char: String,
    #[sqlx(rename = "UserAddressChar")]
    pub user_address_char: String,
    #[sqlx(rename = "UserAddressNumValue")]
    pub user_address_num_value: String,
    #[sqlx(rename = "UserAddressCode")]
    pub user_address_code: String,

    #[sqlx(rename = "UseheadqrterChar")]
    pub useheadqrter_char: String,
    #[sqlx(rename = "UseheadqrterNumValue")]
    pub useheadqrter_num_value: String,
    #[sqlx(rename = "UseheadqrterCode")]
    pub useheadqrter_code: String,

    #[sqlx(rename = "CarKind")]
    pub car_kind: String,
    #[sqlx(rename = "Use")]
    pub use_field: String,
    #[sqlx(rename = "PrivateBusiness")]
    pub private_business: String,
    #[sqlx(rename = "CarShape")]
    pub car_shape: String,
    #[sqlx(rename = "CarShapeCode")]
    pub car_shape_code: String,

    #[sqlx(rename = "NoteCap")]
    pub note_cap: String,
    #[sqlx(rename = "Cap")]
    pub cap: String,
    #[sqlx(rename = "NoteMaxloadage")]
    pub note_maxloadage: String,
    #[sqlx(rename = "Maxloadage")]
    pub maxloadage: String,

    #[sqlx(rename = "NoteCarWgt")]
    pub note_car_wgt: String,
    #[sqlx(rename = "CarWgt")]
    pub car_wgt: String,
    #[sqlx(rename = "NoteCarTotalWgt")]
    pub note_car_total_wgt: String,
    #[sqlx(rename = "CarTotalWgt")]
    pub car_total_wgt: String,

    #[sqlx(rename = "NoteLength")]
    pub note_length: String,
    #[sqlx(rename = "Length")]
    pub length: String,
    #[sqlx(rename = "NoteWidth")]
    pub note_width: String,
    #[sqlx(rename = "Width")]
    pub width: String,
    #[sqlx(rename = "NoteHeight")]
    pub note_height: String,
    #[sqlx(rename = "Height")]
    pub height: String,

    #[sqlx(rename = "FfAxWgt")]
    pub ff_ax_wgt: String,
    #[sqlx(rename = "FrAxWgt")]
    pub fr_ax_wgt: String,
    #[sqlx(rename = "RfAxWgt")]
    pub rf_ax_wgt: String,
    #[sqlx(rename = "RrAxWgt")]
    pub rr_ax_wgt: String,

    #[sqlx(rename = "Displacement")]
    pub displacement: String,
    #[sqlx(rename = "FuelClass")]
    pub fuel_class: String,

    #[sqlx(rename = "ModelSpecifyNo")]
    pub model_specify_no: String,
    #[sqlx(rename = "ClassifyAroundNo")]
    pub classify_around_no: String,

    #[sqlx(rename = "ValidPeriodExpirdateE")]
    pub valid_period_expirdate_e: String,
    #[sqlx(rename = "ValidPeriodExpirdateY")]
    pub valid_period_expirdate_y: String,
    #[sqlx(rename = "ValidPeriodExpirdateM")]
    pub valid_period_expirdate_m: String,
    #[sqlx(rename = "ValidPeriodExpirdateD")]
    pub valid_period_expirdate_d: String,

    #[sqlx(rename = "NoteInfo")]
    pub note_info: String,

    #[sqlx(rename = "TwodimensionCodeInfoEntryNoCarNo")]
    pub twodimension_code_info_entry_no_car_no: String,
    #[sqlx(rename = "TwodimensionCodeInfoCarNo")]
    pub twodimension_code_info_car_no: String,
    #[sqlx(rename = "TwodimensionCodeInfoValidPeriodExpirdate")]
    pub twodimension_code_info_valid_period_expirdate: String,
    #[sqlx(rename = "TwodimensionCodeInfoModel")]
    pub twodimension_code_info_model: String,
    #[sqlx(rename = "TwodimensionCodeInfoModelSpecifyNoClassifyAroundNo")]
    pub twodimension_code_info_model_specify_no_classify_around_no: String,
    #[sqlx(rename = "TwodimensionCodeInfoCharInfo")]
    pub twodimension_code_info_char_info: String,
    #[sqlx(rename = "TwodimensionCodeInfoEngineModel")]
    pub twodimension_code_info_engine_model: String,
    #[sqlx(rename = "TwodimensionCodeInfoCarNoStampPlace")]
    pub twodimension_code_info_car_no_stamp_place: String,
    #[sqlx(rename = "TwodimensionCodeInfoFirstregistdate")]
    pub twodimension_code_info_firstregistdate: String,
    #[sqlx(rename = "TwodimensionCodeInfoFfAxWgt")]
    pub twodimension_code_info_ff_ax_wgt: String,
    #[sqlx(rename = "TwodimensionCodeInfoFrAxWgt")]
    pub twodimension_code_info_fr_ax_wgt: String,
    #[sqlx(rename = "TwodimensionCodeInfoRfAxWgt")]
    pub twodimension_code_info_rf_ax_wgt: String,
    #[sqlx(rename = "TwodimensionCodeInfoRrAxWgt")]
    pub twodimension_code_info_rr_ax_wgt: String,
    #[sqlx(rename = "TwodimensionCodeInfoNoiseReg")]
    pub twodimension_code_info_noise_reg: String,
    #[sqlx(rename = "TwodimensionCodeInfoNearNoiseReg")]
    pub twodimension_code_info_near_noise_reg: String,
    #[sqlx(rename = "TwodimensionCodeInfoDriveMethod")]
    pub twodimension_code_info_drive_method: String,
    #[sqlx(rename = "TwodimensionCodeInfoOpacimeterMeasCar")]
    pub twodimension_code_info_opacimeter_meas_car: String,
    #[sqlx(rename = "TwodimensionCodeInfoNoxPmMeasMode")]
    pub twodimension_code_info_nox_pm_meas_mode: String,
    #[sqlx(rename = "TwodimensionCodeInfoNoxValue")]
    pub twodimension_code_info_nox_value: String,
    #[sqlx(rename = "TwodimensionCodeInfoPmValue")]
    pub twodimension_code_info_pm_value: String,
    #[sqlx(rename = "TwodimensionCodeInfoSafeStdDate")]
    pub twodimension_code_info_safe_std_date: String,
    #[sqlx(rename = "TwodimensionCodeInfoFuelClassCode")]
    pub twodimension_code_info_fuel_class_code: String,

    #[sqlx(rename = "RegistCarLightCar")]
    pub regist_car_light_car: String,

    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: chrono::DateTime<chrono::Utc>,

    // Related data from JOINs
    #[sqlx(default)]
    pub cisa_id_cars: Option<String>,
    #[sqlx(default)]
    pub id_dtako: Option<String>,
    #[sqlx(default)]
    pub files_a_count: Option<i64>,
    #[sqlx(default)]
    pub files_b_count: Option<i64>,
}
