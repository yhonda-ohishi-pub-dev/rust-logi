use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::models::{CarInspectionFileModel, CarInspectionModel};
use crate::proto::car_inspection::car_inspection_files_service_server::CarInspectionFilesService;
use crate::proto::car_inspection::car_inspection_service_server::CarInspectionService;
use crate::proto::car_inspection::{
    CarInspection, CarInspectionFile, CarInspectionFileResponse, CarInspectionResponse,
    CreateCarInspectionFileRequest, CreateCarInspectionRequest, DeleteCarInspectionRequest,
    GetCarInspectionRequest, ListCarInspectionFilesRequest, ListCarInspectionFilesResponse,
    ListCarInspectionsRequest, ListCarInspectionsResponse,
};
use crate::proto::common::Empty;

pub struct CarInspectionServiceImpl {
    pool: PgPool,
}

impl CarInspectionServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn model_to_proto(model: &CarInspectionModel) -> CarInspection {
        CarInspection {
            cert_info_import_file_version: model.cert_info_import_file_version.clone(),
            acceptoutputno: model.acceptoutputno.clone(),
            form_type: model.form_type.clone(),
            elect_cert_mg_no: model.elect_cert_mg_no.clone(),
            car_id: model.car_id.clone(),
            elect_cert_publishdate_e: model.elect_cert_publishdate_e.clone(),
            elect_cert_publishdate_y: model.elect_cert_publishdate_y.clone(),
            elect_cert_publishdate_m: model.elect_cert_publishdate_m.clone(),
            elect_cert_publishdate_d: model.elect_cert_publishdate_d.clone(),
            grantdate_e: model.grantdate_e.clone(),
            grantdate_y: model.grantdate_y.clone(),
            grantdate_m: model.grantdate_m.clone(),
            grantdate_d: model.grantdate_d.clone(),
            transpotation_bureauchiefname: model.transpotation_bureauchiefname.clone(),
            entry_no_car_no: model.entry_no_car_no.clone(),
            reggrantdate_e: model.reggrantdate_e.clone(),
            reggrantdate_y: model.reggrantdate_y.clone(),
            reggrantdate_m: model.reggrantdate_m.clone(),
            reggrantdate_d: model.reggrantdate_d.clone(),
            firstregistdate_e: model.firstregistdate_e.clone(),
            firstregistdate_y: model.firstregistdate_y.clone(),
            firstregistdate_m: model.firstregistdate_m.clone(),
            car_name: model.car_name.clone(),
            car_name_code: model.car_name_code.clone(),
            car_no: model.car_no.clone(),
            model: model.model.clone(),
            engine_model: model.engine_model.clone(),
            ownername_low_level_char: model.ownername_low_level_char.clone(),
            ownername_high_level_char: model.ownername_high_level_char.clone(),
            owner_address_char: model.owner_address_char.clone(),
            owner_address_num_value: model.owner_address_num_value.clone(),
            owner_address_code: model.owner_address_code.clone(),
            username_low_level_char: model.username_low_level_char.clone(),
            username_high_level_char: model.username_high_level_char.clone(),
            user_address_char: model.user_address_char.clone(),
            user_address_num_value: model.user_address_num_value.clone(),
            user_address_code: model.user_address_code.clone(),
            useheadqrter_char: model.useheadqrter_char.clone(),
            useheadqrter_num_value: model.useheadqrter_num_value.clone(),
            useheadqrter_code: model.useheadqrter_code.clone(),
            car_kind: model.car_kind.clone(),
            r#use: model.use_field.clone(),
            private_business: model.private_business.clone(),
            car_shape: model.car_shape.clone(),
            car_shape_code: model.car_shape_code.clone(),
            note_cap: model.note_cap.clone(),
            cap: model.cap.clone(),
            note_maxloadage: model.note_maxloadage.clone(),
            maxloadage: model.maxloadage.clone(),
            note_car_wgt: model.note_car_wgt.clone(),
            car_wgt: model.car_wgt.clone(),
            note_car_total_wgt: model.note_car_total_wgt.clone(),
            car_total_wgt: model.car_total_wgt.clone(),
            note_length: model.note_length.clone(),
            length: model.length.clone(),
            note_width: model.note_width.clone(),
            width: model.width.clone(),
            note_height: model.note_height.clone(),
            height: model.height.clone(),
            ff_ax_wgt: model.ff_ax_wgt.clone(),
            fr_ax_wgt: model.fr_ax_wgt.clone(),
            rf_ax_wgt: model.rf_ax_wgt.clone(),
            rr_ax_wgt: model.rr_ax_wgt.clone(),
            displacement: model.displacement.clone(),
            fuel_class: model.fuel_class.clone(),
            model_specify_no: model.model_specify_no.clone(),
            classify_around_no: model.classify_around_no.clone(),
            valid_period_expirdate_e: model.valid_period_expirdate_e.clone(),
            valid_period_expirdate_y: model.valid_period_expirdate_y.clone(),
            valid_period_expirdate_m: model.valid_period_expirdate_m.clone(),
            valid_period_expirdate_d: model.valid_period_expirdate_d.clone(),
            note_info: model.note_info.clone(),
            twodimension_code_info_entry_no_car_no: model
                .twodimension_code_info_entry_no_car_no
                .clone(),
            twodimension_code_info_car_no: model.twodimension_code_info_car_no.clone(),
            twodimension_code_info_valid_period_expirdate: model
                .twodimension_code_info_valid_period_expirdate
                .clone(),
            twodimension_code_info_model: model.twodimension_code_info_model.clone(),
            twodimension_code_info_model_specify_no_classify_around_no: model
                .twodimension_code_info_model_specify_no_classify_around_no
                .clone(),
            twodimension_code_info_char_info: model.twodimension_code_info_char_info.clone(),
            twodimension_code_info_engine_model: model.twodimension_code_info_engine_model.clone(),
            twodimension_code_info_car_no_stamp_place: model
                .twodimension_code_info_car_no_stamp_place
                .clone(),
            twodimension_code_info_firstregistdate: model
                .twodimension_code_info_firstregistdate
                .clone(),
            twodimension_code_info_ff_ax_wgt: model.twodimension_code_info_ff_ax_wgt.clone(),
            twodimension_code_info_fr_ax_wgt: model.twodimension_code_info_fr_ax_wgt.clone(),
            twodimension_code_info_rf_ax_wgt: model.twodimension_code_info_rf_ax_wgt.clone(),
            twodimension_code_info_rr_ax_wgt: model.twodimension_code_info_rr_ax_wgt.clone(),
            twodimension_code_info_noise_reg: model.twodimension_code_info_noise_reg.clone(),
            twodimension_code_info_near_noise_reg: model
                .twodimension_code_info_near_noise_reg
                .clone(),
            twodimension_code_info_drive_method: model.twodimension_code_info_drive_method.clone(),
            twodimension_code_info_opacimeter_meas_car: model
                .twodimension_code_info_opacimeter_meas_car
                .clone(),
            twodimension_code_info_nox_pm_meas_mode: model
                .twodimension_code_info_nox_pm_meas_mode
                .clone(),
            twodimension_code_info_nox_value: model.twodimension_code_info_nox_value.clone(),
            twodimension_code_info_pm_value: model.twodimension_code_info_pm_value.clone(),
            twodimension_code_info_safe_std_date: model
                .twodimension_code_info_safe_std_date
                .clone(),
            twodimension_code_info_fuel_class_code: model
                .twodimension_code_info_fuel_class_code
                .clone(),
            regist_car_light_car: model.regist_car_light_car.clone(),
            created: model.created_at.to_rfc3339(),
            modified: model.modified_at.to_rfc3339(),
            pdf_uuid: model.pdf_uuid.clone(),
            json_uuid: model.json_uuid.clone(),
        }
    }
}

#[tonic::async_trait]
impl CarInspectionService for CarInspectionServiceImpl {
    async fn create_car_inspection(
        &self,
        request: Request<CreateCarInspectionRequest>,
    ) -> Result<Response<CarInspectionResponse>, Status> {
        let req = request.into_inner();
        let ci = req
            .car_inspection
            .ok_or_else(|| Status::invalid_argument("car_inspection is required"))?;

        // Use ON CONFLICT DO UPDATE for upsert
        // Note: created_at and modified_at use DB defaults (NOW())
        let result = sqlx::query_as::<_, CarInspectionModel>(
            r#"
            INSERT INTO car_inspection (
                organization_id,
                "CertInfoImportFileVersion", "Acceptoutputno", "FormType", "ElectCertMgNo", "CarId",
                "ElectCertPublishdateE", "ElectCertPublishdateY", "ElectCertPublishdateM", "ElectCertPublishdateD",
                "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD",
                "TranspotationBureauchiefName", "EntryNoCarNo",
                "ReggrantdateE", "ReggrantdateY", "ReggrantdateM", "ReggrantdateD",
                "FirstregistdateE", "FirstregistdateY", "FirstregistdateM",
                "CarName", "CarNameCode", "CarNo", "Model", "EngineModel",
                "OwnernameLowLevelChar", "OwnernameHighLevelChar", "OwnerAddressChar", "OwnerAddressNumValue", "OwnerAddressCode",
                "UsernameLowLevelChar", "UsernameHighLevelChar", "UserAddressChar", "UserAddressNumValue", "UserAddressCode",
                "UseheadqrterChar", "UseheadqrterNumValue", "UseheadqrterCode",
                "CarKind", "Use", "PrivateBusiness", "CarShape", "CarShapeCode",
                "NoteCap", "Cap", "NoteMaxloadage", "Maxloadage",
                "NoteCarWgt", "CarWgt", "NoteCarTotalWgt", "CarTotalWgt",
                "NoteLength", "Length", "NoteWidth", "Width", "NoteHeight", "Height",
                "FfAxWgt", "FrAxWgt", "RfAxWgt", "RrAxWgt",
                "Displacement", "FuelClass", "ModelSpecifyNo", "ClassifyAroundNo",
                "ValidPeriodExpirdateE", "ValidPeriodExpirdateY", "ValidPeriodExpirdateM", "ValidPeriodExpirdateD",
                "NoteInfo",
                "TwodimensionCodeInfoEntryNoCarNo", "TwodimensionCodeInfoCarNo", "TwodimensionCodeInfoValidPeriodExpirdate",
                "TwodimensionCodeInfoModel", "TwodimensionCodeInfoModelSpecifyNoClassifyAroundNo",
                "TwodimensionCodeInfoCharInfo", "TwodimensionCodeInfoEngineModel", "TwodimensionCodeInfoCarNoStampPlace",
                "TwodimensionCodeInfoFirstregistdate",
                "TwodimensionCodeInfoFfAxWgt", "TwodimensionCodeInfoFrAxWgt", "TwodimensionCodeInfoRfAxWgt", "TwodimensionCodeInfoRrAxWgt",
                "TwodimensionCodeInfoNoiseReg", "TwodimensionCodeInfoNearNoiseReg", "TwodimensionCodeInfoDriveMethod",
                "TwodimensionCodeInfoOpacimeterMeasCar", "TwodimensionCodeInfoNoxPmMeasMode",
                "TwodimensionCodeInfoNoxValue", "TwodimensionCodeInfoPmValue",
                "TwodimensionCodeInfoSafeStdDate", "TwodimensionCodeInfoFuelClassCode",
                "RegistCarLightCar"
            ) VALUES (
                current_setting('app.current_organization_id')::uuid,
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                $31, $32, $33, $34, $35, $36, $37, $38, $39, $40,
                $41, $42, $43, $44, $45, $46, $47, $48, $49, $50,
                $51, $52, $53, $54, $55, $56, $57, $58, $59, $60,
                $61, $62, $63, $64, $65, $66, $67, $68, $69, $70,
                $71, $72, $73, $74, $75, $76, $77, $78, $79, $80,
                $81, $82, $83, $84, $85, $86, $87, $88, $89, $90,
                $91, $92, $93, $94, $95
            )
            ON CONFLICT (organization_id, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD")
            DO UPDATE SET modified_at = NOW()
            RETURNING *
            "#,
        )
        .bind(&ci.cert_info_import_file_version)
        .bind(&ci.acceptoutputno)
        .bind(&ci.form_type)
        .bind(&ci.elect_cert_mg_no)
        .bind(&ci.car_id)
        .bind(&ci.elect_cert_publishdate_e)
        .bind(&ci.elect_cert_publishdate_y)
        .bind(&ci.elect_cert_publishdate_m)
        .bind(&ci.elect_cert_publishdate_d)
        .bind(&ci.grantdate_e)
        .bind(&ci.grantdate_y)
        .bind(&ci.grantdate_m)
        .bind(&ci.grantdate_d)
        .bind(&ci.transpotation_bureauchiefname)
        .bind(&ci.entry_no_car_no)
        .bind(&ci.reggrantdate_e)
        .bind(&ci.reggrantdate_y)
        .bind(&ci.reggrantdate_m)
        .bind(&ci.reggrantdate_d)
        .bind(&ci.firstregistdate_e)
        .bind(&ci.firstregistdate_y)
        .bind(&ci.firstregistdate_m)
        .bind(&ci.car_name)
        .bind(&ci.car_name_code)
        .bind(&ci.car_no)
        .bind(&ci.model)
        .bind(&ci.engine_model)
        .bind(&ci.ownername_low_level_char)
        .bind(&ci.ownername_high_level_char)
        .bind(&ci.owner_address_char)
        .bind(&ci.owner_address_num_value)
        .bind(&ci.owner_address_code)
        .bind(&ci.username_low_level_char)
        .bind(&ci.username_high_level_char)
        .bind(&ci.user_address_char)
        .bind(&ci.user_address_num_value)
        .bind(&ci.user_address_code)
        .bind(&ci.useheadqrter_char)
        .bind(&ci.useheadqrter_num_value)
        .bind(&ci.useheadqrter_code)
        .bind(&ci.car_kind)
        .bind(&ci.r#use)
        .bind(&ci.private_business)
        .bind(&ci.car_shape)
        .bind(&ci.car_shape_code)
        .bind(&ci.note_cap)
        .bind(&ci.cap)
        .bind(&ci.note_maxloadage)
        .bind(&ci.maxloadage)
        .bind(&ci.note_car_wgt)
        .bind(&ci.car_wgt)
        .bind(&ci.note_car_total_wgt)
        .bind(&ci.car_total_wgt)
        .bind(&ci.note_length)
        .bind(&ci.length)
        .bind(&ci.note_width)
        .bind(&ci.width)
        .bind(&ci.note_height)
        .bind(&ci.height)
        .bind(&ci.ff_ax_wgt)
        .bind(&ci.fr_ax_wgt)
        .bind(&ci.rf_ax_wgt)
        .bind(&ci.rr_ax_wgt)
        .bind(&ci.displacement)
        .bind(&ci.fuel_class)
        .bind(&ci.model_specify_no)
        .bind(&ci.classify_around_no)
        .bind(&ci.valid_period_expirdate_e)
        .bind(&ci.valid_period_expirdate_y)
        .bind(&ci.valid_period_expirdate_m)
        .bind(&ci.valid_period_expirdate_d)
        .bind(&ci.note_info)
        .bind(&ci.twodimension_code_info_entry_no_car_no)
        .bind(&ci.twodimension_code_info_car_no)
        .bind(&ci.twodimension_code_info_valid_period_expirdate)
        .bind(&ci.twodimension_code_info_model)
        .bind(&ci.twodimension_code_info_model_specify_no_classify_around_no)
        .bind(&ci.twodimension_code_info_char_info)
        .bind(&ci.twodimension_code_info_engine_model)
        .bind(&ci.twodimension_code_info_car_no_stamp_place)
        .bind(&ci.twodimension_code_info_firstregistdate)
        .bind(&ci.twodimension_code_info_ff_ax_wgt)
        .bind(&ci.twodimension_code_info_fr_ax_wgt)
        .bind(&ci.twodimension_code_info_rf_ax_wgt)
        .bind(&ci.twodimension_code_info_rr_ax_wgt)
        .bind(&ci.twodimension_code_info_noise_reg)
        .bind(&ci.twodimension_code_info_near_noise_reg)
        .bind(&ci.twodimension_code_info_drive_method)
        .bind(&ci.twodimension_code_info_opacimeter_meas_car)
        .bind(&ci.twodimension_code_info_nox_pm_meas_mode)
        .bind(&ci.twodimension_code_info_nox_value)
        .bind(&ci.twodimension_code_info_pm_value)
        .bind(&ci.twodimension_code_info_safe_std_date)
        .bind(&ci.twodimension_code_info_fuel_class_code)
        .bind(&ci.regist_car_light_car)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(CarInspectionResponse {
            car_inspection: Some(Self::model_to_proto(&result)),
        }))
    }

    async fn list_car_inspections(
        &self,
        _request: Request<ListCarInspectionsRequest>,
    ) -> Result<Response<ListCarInspectionsResponse>, Status> {
        let inspections = sqlx::query_as::<_, CarInspectionModel>(
            r#"SELECT * FROM car_inspection ORDER BY "GrantdateY" DESC, "GrantdateM" DESC, "GrantdateD" DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_inspections: Vec<CarInspection> =
            inspections.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListCarInspectionsResponse {
            car_inspections: proto_inspections,
            pagination: None,
        }))
    }

    async fn list_current_car_inspections(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListCarInspectionsResponse>, Status> {
        // Get current car inspections (valid period not expired) with file UUIDs
        let inspections = sqlx::query_as::<_, CarInspectionModel>(
            r#"
            SELECT
                ci.*,
                pdf.uuid as pdf_uuid,
                json_file.uuid as json_uuid
            FROM car_inspection ci
            LEFT JOIN car_inspection_files_a pdf
                ON ci."ElectCertMgNo" = pdf."ElectCertMgNo"
                AND ci."GrantdateE" = pdf."GrantdateE"
                AND ci."GrantdateY" = pdf."GrantdateY"
                AND ci."GrantdateM" = pdf."GrantdateM"
                AND ci."GrantdateD" = pdf."GrantdateD"
                AND pdf.type = 'application/pdf'
                AND pdf.deleted IS NULL
            LEFT JOIN car_inspection_files_a json_file
                ON ci."ElectCertMgNo" = json_file."ElectCertMgNo"
                AND ci."GrantdateE" = json_file."GrantdateE"
                AND ci."GrantdateY" = json_file."GrantdateY"
                AND ci."GrantdateM" = json_file."GrantdateM"
                AND ci."GrantdateD" = json_file."GrantdateD"
                AND json_file.type = 'application/json'
                AND json_file.deleted IS NULL
            WHERE ci."TwodimensionCodeInfoValidPeriodExpirdate" >= to_char(CURRENT_DATE, 'YYYYMMDD')
            ORDER BY ci."TwodimensionCodeInfoValidPeriodExpirdate" ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_inspections: Vec<CarInspection> =
            inspections.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListCarInspectionsResponse {
            car_inspections: proto_inspections,
            pagination: None,
        }))
    }

    async fn get_car_inspection(
        &self,
        request: Request<GetCarInspectionRequest>,
    ) -> Result<Response<CarInspectionResponse>, Status> {
        let req = request.into_inner();

        let inspection = sqlx::query_as::<_, CarInspectionModel>(
            r#"
            SELECT * FROM car_inspection
            WHERE "ElectCertMgNo" = $1
              AND "GrantdateE" = $2
              AND "GrantdateY" = $3
              AND "GrantdateM" = $4
              AND "GrantdateD" = $5
            "#,
        )
        .bind(&req.elect_cert_mg_no)
        .bind(&req.grantdate_e)
        .bind(&req.grantdate_y)
        .bind(&req.grantdate_m)
        .bind(&req.grantdate_d)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?
        .ok_or_else(|| Status::not_found("Car inspection not found"))?;

        Ok(Response::new(CarInspectionResponse {
            car_inspection: Some(Self::model_to_proto(&inspection)),
        }))
    }

    async fn delete_car_inspection(
        &self,
        request: Request<DeleteCarInspectionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();

        sqlx::query(
            r#"
            DELETE FROM car_inspection
            WHERE "ElectCertMgNo" = $1
              AND "GrantdateE" = $2
              AND "GrantdateY" = $3
              AND "GrantdateM" = $4
              AND "GrantdateD" = $5
            "#,
        )
        .bind(&req.elect_cert_mg_no)
        .bind(&req.grantdate_e)
        .bind(&req.grantdate_y)
        .bind(&req.grantdate_m)
        .bind(&req.grantdate_d)
        .execute(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(Empty {}))
    }

    async fn list_expired_or_about_to_expire(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListCarInspectionsResponse>, Status> {
        // Expired or expiring within 30 days
        let inspections = sqlx::query_as::<_, CarInspectionModel>(
            r#"
            SELECT * FROM car_inspection
            WHERE "TwodimensionCodeInfoValidPeriodExpirdate" <= to_char(CURRENT_DATE + INTERVAL '30 days', 'YYYYMMDD')
            ORDER BY "TwodimensionCodeInfoValidPeriodExpirdate" ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_inspections: Vec<CarInspection> =
            inspections.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListCarInspectionsResponse {
            car_inspections: proto_inspections,
            pagination: None,
        }))
    }

    async fn list_renew_targets(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListCarInspectionsResponse>, Status> {
        // Vehicles that need renewal (expiring within 60 days)
        let inspections = sqlx::query_as::<_, CarInspectionModel>(
            r#"
            SELECT * FROM car_inspection
            WHERE "TwodimensionCodeInfoValidPeriodExpirdate" >= to_char(CURRENT_DATE, 'YYYYMMDD')
              AND "TwodimensionCodeInfoValidPeriodExpirdate" <= to_char(CURRENT_DATE + INTERVAL '60 days', 'YYYYMMDD')
            ORDER BY "TwodimensionCodeInfoValidPeriodExpirdate" ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_inspections: Vec<CarInspection> =
            inspections.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListCarInspectionsResponse {
            car_inspections: proto_inspections,
            pagination: None,
        }))
    }
}

// CarInspectionFilesService implementation
pub struct CarInspectionFilesServiceImpl {
    pool: PgPool,
}

impl CarInspectionFilesServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn model_to_proto(model: &CarInspectionFileModel) -> CarInspectionFile {
        CarInspectionFile {
            uuid: model.uuid.clone(),
            r#type: model.file_type.clone(),
            elect_cert_mg_no: model.elect_cert_mg_no.clone(),
            grantdate_e: model.grantdate_e.clone(),
            grantdate_y: model.grantdate_y.clone(),
            grantdate_m: model.grantdate_m.clone(),
            grantdate_d: model.grantdate_d.clone(),
            created: model.created.clone(),
            modified: model.modified.clone(),
            deleted: model.deleted.clone(),
        }
    }
}

#[tonic::async_trait]
impl CarInspectionFilesService for CarInspectionFilesServiceImpl {
    async fn create_car_inspection_file(
        &self,
        request: Request<CreateCarInspectionFileRequest>,
    ) -> Result<Response<CarInspectionFileResponse>, Status> {
        let req = request.into_inner();
        let file = req
            .file
            .ok_or_else(|| Status::invalid_argument("file is required"))?;

        let created = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query_as::<_, CarInspectionFileModel>(
            r#"
            INSERT INTO car_inspection_files_a (uuid, type, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD", created)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (uuid) DO UPDATE SET modified = $8
            RETURNING *
            "#,
        )
        .bind(&file.uuid)
        .bind(&file.r#type)
        .bind(&file.elect_cert_mg_no)
        .bind(&file.grantdate_e)
        .bind(&file.grantdate_y)
        .bind(&file.grantdate_m)
        .bind(&file.grantdate_d)
        .bind(&created)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(CarInspectionFileResponse {
            file: Some(Self::model_to_proto(&result)),
        }))
    }

    async fn list_car_inspection_files(
        &self,
        request: Request<ListCarInspectionFilesRequest>,
    ) -> Result<Response<ListCarInspectionFilesResponse>, Status> {
        let req = request.into_inner();

        let files = if let Some(elect_cert_mg_no) = req.elect_cert_mg_no {
            sqlx::query_as::<_, CarInspectionFileModel>(
                r#"SELECT * FROM car_inspection_files_a WHERE "ElectCertMgNo" = $1 AND deleted IS NULL ORDER BY created DESC"#,
            )
            .bind(&elect_cert_mg_no)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, CarInspectionFileModel>(
                r#"SELECT * FROM car_inspection_files_a WHERE deleted IS NULL ORDER BY created DESC"#,
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_files: Vec<CarInspectionFile> = files.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListCarInspectionFilesResponse {
            files: proto_files,
            pagination: None,
        }))
    }

    async fn list_current_car_inspection_files(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListCarInspectionFilesResponse>, Status> {
        let files = sqlx::query_as::<_, CarInspectionFileModel>(
            r#"
            SELECT cif.*
            FROM car_inspection_files_a cif
            INNER JOIN car_inspection ci ON
                cif."ElectCertMgNo" = ci."ElectCertMgNo"
                AND cif."GrantdateE" = ci."GrantdateE"
                AND cif."GrantdateY" = ci."GrantdateY"
                AND cif."GrantdateM" = ci."GrantdateM"
                AND cif."GrantdateD" = ci."GrantdateD"
            WHERE cif.deleted IS NULL
              AND ci."TwodimensionCodeInfoValidPeriodExpirdate" >= to_char(CURRENT_DATE, 'YYYYMMDD')
            ORDER BY cif.created DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_files: Vec<CarInspectionFile> = files.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListCarInspectionFilesResponse {
            files: proto_files,
            pagination: None,
        }))
    }
}
