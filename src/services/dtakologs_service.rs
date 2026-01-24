use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::db::{get_organization_from_request, set_current_organization};
use crate::models::DtakologModel;
use crate::proto::common::Empty;
use crate::proto::dtakologs::dtakologs_service_server::DtakologsService;
use crate::proto::dtakologs::{
    CreateDtakologRequest, CreateDtakologResponse, CurrentListSelectRequest, DeleteResponse,
    Dtakolog, GetDateRequest, ListDtakologsResponse,
};

pub struct DtakologsServiceImpl {
    pool: PgPool,
}

impl DtakologsServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn model_to_proto(model: &DtakologModel) -> Dtakolog {
        model.to_proto()
    }
}

#[tonic::async_trait]
impl DtakologsService for DtakologsServiceImpl {
    /// 全運行ログ取得
    async fn list_all(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ListDtakologsResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        tracing::info!("ListAll called for organization: {}", organization_id);

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;

        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        let dtakologs = sqlx::query_as::<_, DtakologModel>(
            r#"
            SELECT
                data_date_time, vehicle_cd, type, all_state_font_color_index,
                all_state_ryout_color, branch_cd, branch_name, current_work_cd,
                data_filter_type, disp_flag, driver_cd, gps_direction, gps_enable,
                gps_latitude, gps_longitude, gps_satellite_num, operation_state,
                recive_event_type, recive_packet_type, recive_work_cd, revo,
                setting_temp, setting_temp1, setting_temp3, setting_temp4, speed,
                sub_driver_cd, temp_state, vehicle_name, address_disp_c, address_disp_p,
                all_state, all_state_ex, all_state_font_color, comu_date_time,
                current_work_name, driver_name, event_val, gps_lati_and_long, odometer,
                recive_type_color_name, recive_type_name, start_work_date_time, state,
                state1, state2, state3, state_flag, temp1, temp2, temp3, temp4,
                vehicle_icon_color, vehicle_icon_label_for_datetime,
                vehicle_icon_label_for_driver, vehicle_icon_label_for_vehicle
            FROM dtakologs
            ORDER BY data_date_time DESC
            "#,
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to fetch dtakologs: {}", e)))?;

        let proto_dtakologs: Vec<Dtakolog> =
            dtakologs.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListDtakologsResponse {
            dtakologs: proto_dtakologs,
            pagination: None,
        }))
    }

    /// VehicleCD毎の最新運行ログ取得
    async fn current_list_all(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ListDtakologsResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        tracing::info!("CurrentListAll called for organization: {}", organization_id);

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;

        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        // サブクエリでVehicleCD毎の最新DataDateTimeを取得してJOIN
        let dtakologs = sqlx::query_as::<_, DtakologModel>(
            r#"
            SELECT d.*
            FROM dtakologs d
            INNER JOIN (
                SELECT vehicle_cd, MAX(data_date_time) as max_data_date_time
                FROM dtakologs
                GROUP BY vehicle_cd
            ) latest ON d.vehicle_cd = latest.vehicle_cd
                     AND d.data_date_time = latest.max_data_date_time
            ORDER BY d.vehicle_cd ASC
            "#,
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to fetch dtakologs: {}", e)))?;

        let proto_dtakologs: Vec<Dtakolog> =
            dtakologs.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListDtakologsResponse {
            dtakologs: proto_dtakologs,
            pagination: None,
        }))
    }

    /// ホーム車両の最新運行ログ取得 (AddressDispP="本社営業所")
    async fn current_list_all_home(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ListDtakologsResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        tracing::info!(
            "CurrentListAllHome called for organization: {}",
            organization_id
        );

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;

        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        // サブクエリでVehicleCD毎の最新DataDateTimeを取得してJOIN
        // AddressDispPでフィルタ
        let dtakologs = sqlx::query_as::<_, DtakologModel>(
            r#"
            SELECT d.*
            FROM dtakologs d
            INNER JOIN (
                SELECT vehicle_cd, MAX(data_date_time) as max_data_date_time
                FROM dtakologs
                GROUP BY vehicle_cd
            ) latest ON d.vehicle_cd = latest.vehicle_cd
                     AND d.data_date_time = latest.max_data_date_time
            WHERE d.address_disp_p LIKE '%本社営業所%'
            ORDER BY d.vehicle_cd ASC
            "#,
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to fetch dtakologs: {}", e)))?;

        let proto_dtakologs: Vec<Dtakolog> =
            dtakologs.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListDtakologsResponse {
            dtakologs: proto_dtakologs,
            pagination: None,
        }))
    }

    /// 指定条件での最新運行ログ取得
    async fn current_list_select(
        &self,
        request: Request<CurrentListSelectRequest>,
    ) -> Result<Response<ListDtakologsResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        tracing::info!(
            "CurrentListSelect called for organization: {}, address_disp_p: {:?}, branch_cd: {:?}, vehicle_cds: {:?}",
            organization_id,
            req.address_disp_p,
            req.branch_cd,
            req.vehicle_cds
        );

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;

        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        // 動的クエリ構築
        let mut conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(ref address) = req.address_disp_p {
            conditions.push(format!("d.address_disp_p LIKE '%' || ${} || '%'", params.len() + 1));
            params.push(address.clone());
        }

        if let Some(branch_cd) = req.branch_cd {
            conditions.push(format!("d.branch_cd = ${}", params.len() + 1));
            params.push(branch_cd.to_string());
        }

        if !req.vehicle_cds.is_empty() {
            let placeholders: Vec<String> = req
                .vehicle_cds
                .iter()
                .enumerate()
                .map(|(i, _)| format!("${}", params.len() + i + 1))
                .collect();
            conditions.push(format!("d.vehicle_cd IN ({})", placeholders.join(", ")));
            for cd in &req.vehicle_cds {
                params.push(cd.to_string());
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // 動的クエリは将来的に使用予定
        let _query = format!(
            r#"
            SELECT d.*
            FROM dtakologs d
            INNER JOIN (
                SELECT vehicle_cd, MAX(data_date_time) as max_data_date_time
                FROM dtakologs
                GROUP BY vehicle_cd
            ) latest ON d.vehicle_cd = latest.vehicle_cd
                     AND d.data_date_time = latest.max_data_date_time
            {}
            ORDER BY d.vehicle_cd ASC
            "#,
            where_clause
        );

        // 動的バインドが複雑なため、シンプルにフィルタなしで取得してからフィルタ
        let dtakologs = sqlx::query_as::<_, DtakologModel>(
            r#"
            SELECT d.*
            FROM dtakologs d
            INNER JOIN (
                SELECT vehicle_cd, MAX(data_date_time) as max_data_date_time
                FROM dtakologs
                GROUP BY vehicle_cd
            ) latest ON d.vehicle_cd = latest.vehicle_cd
                     AND d.data_date_time = latest.max_data_date_time
            ORDER BY d.vehicle_cd ASC
            "#,
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to fetch dtakologs: {}", e)))?;

        // アプリケーション側でフィルタ
        let filtered: Vec<DtakologModel> = dtakologs
            .into_iter()
            .filter(|d| {
                let address_ok = req.address_disp_p.as_ref().map_or(true, |addr| {
                    d.address_disp_p
                        .as_ref()
                        .map_or(false, |a| a.contains(addr))
                });
                let branch_ok = req.branch_cd.map_or(true, |b| d.branch_cd == b);
                let vehicle_ok = req.vehicle_cds.is_empty()
                    || req.vehicle_cds.contains(&d.vehicle_cd);
                address_ok && branch_ok && vehicle_ok
            })
            .collect();

        let proto_dtakologs: Vec<Dtakolog> =
            filtered.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListDtakologsResponse {
            dtakologs: proto_dtakologs,
            pagination: None,
        }))
    }

    /// 日付指定で運行ログ取得
    async fn get_date(
        &self,
        request: Request<GetDateRequest>,
    ) -> Result<Response<ListDtakologsResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        tracing::info!(
            "GetDate called for organization: {}, date_time: {}, vehicle_cd: {:?}",
            organization_id,
            req.date_time,
            req.vehicle_cd
        );

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;

        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        let dtakologs = if let Some(vehicle_cd) = req.vehicle_cd {
            sqlx::query_as::<_, DtakologModel>(
                r#"
                SELECT
                    data_date_time, vehicle_cd, type, all_state_font_color_index,
                    all_state_ryout_color, branch_cd, branch_name, current_work_cd,
                    data_filter_type, disp_flag, driver_cd, gps_direction, gps_enable,
                    gps_latitude, gps_longitude, gps_satellite_num, operation_state,
                    recive_event_type, recive_packet_type, recive_work_cd, revo,
                    setting_temp, setting_temp1, setting_temp3, setting_temp4, speed,
                    sub_driver_cd, temp_state, vehicle_name, address_disp_c, address_disp_p,
                    all_state, all_state_ex, all_state_font_color, comu_date_time,
                    current_work_name, driver_name, event_val, gps_lati_and_long, odometer,
                    recive_type_color_name, recive_type_name, start_work_date_time, state,
                    state1, state2, state3, state_flag, temp1, temp2, temp3, temp4,
                    vehicle_icon_color, vehicle_icon_label_for_datetime,
                    vehicle_icon_label_for_driver, vehicle_icon_label_for_vehicle
                FROM dtakologs
                WHERE data_date_time = $1 AND vehicle_cd = $2
                "#,
            )
            .bind(&req.date_time)
            .bind(vehicle_cd)
            .fetch_all(&mut *conn)
            .await
        } else {
            sqlx::query_as::<_, DtakologModel>(
                r#"
                SELECT
                    data_date_time, vehicle_cd, type, all_state_font_color_index,
                    all_state_ryout_color, branch_cd, branch_name, current_work_cd,
                    data_filter_type, disp_flag, driver_cd, gps_direction, gps_enable,
                    gps_latitude, gps_longitude, gps_satellite_num, operation_state,
                    recive_event_type, recive_packet_type, recive_work_cd, revo,
                    setting_temp, setting_temp1, setting_temp3, setting_temp4, speed,
                    sub_driver_cd, temp_state, vehicle_name, address_disp_c, address_disp_p,
                    all_state, all_state_ex, all_state_font_color, comu_date_time,
                    current_work_name, driver_name, event_val, gps_lati_and_long, odometer,
                    recive_type_color_name, recive_type_name, start_work_date_time, state,
                    state1, state2, state3, state_flag, temp1, temp2, temp3, temp4,
                    vehicle_icon_color, vehicle_icon_label_for_datetime,
                    vehicle_icon_label_for_driver, vehicle_icon_label_for_vehicle
                FROM dtakologs
                WHERE data_date_time = $1
                ORDER BY vehicle_cd ASC
                "#,
            )
            .bind(&req.date_time)
            .fetch_all(&mut *conn)
            .await
        }
        .map_err(|e| Status::internal(format!("Failed to fetch dtakologs: {}", e)))?;

        let proto_dtakologs: Vec<Dtakolog> =
            dtakologs.iter().map(Self::model_to_proto).collect();

        Ok(Response::new(ListDtakologsResponse {
            dtakologs: proto_dtakologs,
            pagination: None,
        }))
    }

    /// 運行ログ作成
    async fn create(
        &self,
        request: Request<CreateDtakologRequest>,
    ) -> Result<Response<CreateDtakologResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        let dtakolog = req
            .dtakolog
            .ok_or_else(|| Status::invalid_argument("dtakolog is required"))?;

        tracing::info!(
            "Create called for organization: {}, vehicle_cd: {}, data_date_time: {}",
            organization_id,
            dtakolog.vehicle_cd,
            dtakolog.data_date_time
        );

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;

        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        sqlx::query(
            r#"
            INSERT INTO dtakologs (
                organization_id, data_date_time, vehicle_cd, type,
                all_state_font_color_index, all_state_ryout_color, branch_cd, branch_name,
                current_work_cd, data_filter_type, disp_flag, driver_cd,
                gps_direction, gps_enable, gps_latitude, gps_longitude, gps_satellite_num,
                operation_state, recive_event_type, recive_packet_type, recive_work_cd, revo,
                setting_temp, setting_temp1, setting_temp3, setting_temp4, speed,
                sub_driver_cd, temp_state, vehicle_name,
                address_disp_c, address_disp_p, all_state, all_state_ex, all_state_font_color,
                comu_date_time, current_work_name, driver_name, event_val, gps_lati_and_long,
                odometer, recive_type_color_name, recive_type_name, start_work_date_time,
                state, state1, state2, state3, state_flag,
                temp1, temp2, temp3, temp4,
                vehicle_icon_color, vehicle_icon_label_for_datetime,
                vehicle_icon_label_for_driver, vehicle_icon_label_for_vehicle
            ) VALUES (
                $1::uuid, $2, $3, $4,
                $5, $6, $7, $8,
                $9, $10, $11, $12,
                $13, $14, $15, $16, $17,
                $18, $19, $20, $21, $22,
                $23, $24, $25, $26, $27,
                $28, $29, $30,
                $31, $32, $33, $34, $35,
                $36, $37, $38, $39, $40,
                $41, $42, $43, $44,
                $45, $46, $47, $48, $49,
                $50, $51, $52, $53,
                $54, $55, $56, $57
            )
            ON CONFLICT (organization_id, data_date_time, vehicle_cd) DO UPDATE SET
                type = EXCLUDED.type,
                all_state_font_color_index = EXCLUDED.all_state_font_color_index,
                all_state_ryout_color = EXCLUDED.all_state_ryout_color,
                branch_cd = EXCLUDED.branch_cd,
                branch_name = EXCLUDED.branch_name,
                current_work_cd = EXCLUDED.current_work_cd,
                data_filter_type = EXCLUDED.data_filter_type,
                disp_flag = EXCLUDED.disp_flag,
                driver_cd = EXCLUDED.driver_cd,
                gps_direction = EXCLUDED.gps_direction,
                gps_enable = EXCLUDED.gps_enable,
                gps_latitude = EXCLUDED.gps_latitude,
                gps_longitude = EXCLUDED.gps_longitude,
                gps_satellite_num = EXCLUDED.gps_satellite_num,
                operation_state = EXCLUDED.operation_state,
                recive_event_type = EXCLUDED.recive_event_type,
                recive_packet_type = EXCLUDED.recive_packet_type,
                recive_work_cd = EXCLUDED.recive_work_cd,
                revo = EXCLUDED.revo,
                setting_temp = EXCLUDED.setting_temp,
                setting_temp1 = EXCLUDED.setting_temp1,
                setting_temp3 = EXCLUDED.setting_temp3,
                setting_temp4 = EXCLUDED.setting_temp4,
                speed = EXCLUDED.speed,
                sub_driver_cd = EXCLUDED.sub_driver_cd,
                temp_state = EXCLUDED.temp_state,
                vehicle_name = EXCLUDED.vehicle_name,
                address_disp_c = EXCLUDED.address_disp_c,
                address_disp_p = EXCLUDED.address_disp_p,
                all_state = EXCLUDED.all_state,
                all_state_ex = EXCLUDED.all_state_ex,
                all_state_font_color = EXCLUDED.all_state_font_color,
                comu_date_time = EXCLUDED.comu_date_time,
                current_work_name = EXCLUDED.current_work_name,
                driver_name = EXCLUDED.driver_name,
                event_val = EXCLUDED.event_val,
                gps_lati_and_long = EXCLUDED.gps_lati_and_long,
                odometer = EXCLUDED.odometer,
                recive_type_color_name = EXCLUDED.recive_type_color_name,
                recive_type_name = EXCLUDED.recive_type_name,
                start_work_date_time = EXCLUDED.start_work_date_time,
                state = EXCLUDED.state,
                state1 = EXCLUDED.state1,
                state2 = EXCLUDED.state2,
                state3 = EXCLUDED.state3,
                state_flag = EXCLUDED.state_flag,
                temp1 = EXCLUDED.temp1,
                temp2 = EXCLUDED.temp2,
                temp3 = EXCLUDED.temp3,
                temp4 = EXCLUDED.temp4,
                vehicle_icon_color = EXCLUDED.vehicle_icon_color,
                vehicle_icon_label_for_datetime = EXCLUDED.vehicle_icon_label_for_datetime,
                vehicle_icon_label_for_driver = EXCLUDED.vehicle_icon_label_for_driver,
                vehicle_icon_label_for_vehicle = EXCLUDED.vehicle_icon_label_for_vehicle
            "#,
        )
        .bind(&organization_id)
        .bind(&dtakolog.data_date_time)
        .bind(dtakolog.vehicle_cd)
        .bind(&dtakolog.r#type)
        .bind(dtakolog.all_state_font_color_index)
        .bind(&dtakolog.all_state_ryout_color)
        .bind(dtakolog.branch_cd)
        .bind(&dtakolog.branch_name)
        .bind(dtakolog.current_work_cd)
        .bind(dtakolog.data_filter_type)
        .bind(dtakolog.disp_flag)
        .bind(dtakolog.driver_cd)
        .bind(dtakolog.gps_direction)
        .bind(dtakolog.gps_enable)
        .bind(dtakolog.gps_latitude)
        .bind(dtakolog.gps_longitude)
        .bind(dtakolog.gps_satellite_num)
        .bind(dtakolog.operation_state)
        .bind(dtakolog.recive_event_type)
        .bind(dtakolog.recive_packet_type)
        .bind(dtakolog.recive_work_cd)
        .bind(dtakolog.revo)
        .bind(&dtakolog.setting_temp)
        .bind(&dtakolog.setting_temp1)
        .bind(&dtakolog.setting_temp3)
        .bind(&dtakolog.setting_temp4)
        .bind(dtakolog.speed)
        .bind(dtakolog.sub_driver_cd)
        .bind(dtakolog.temp_state)
        .bind(&dtakolog.vehicle_name)
        .bind(&dtakolog.address_disp_c)
        .bind(&dtakolog.address_disp_p)
        .bind(&dtakolog.all_state)
        .bind(&dtakolog.all_state_ex)
        .bind(&dtakolog.all_state_font_color)
        .bind(&dtakolog.comu_date_time)
        .bind(&dtakolog.current_work_name)
        .bind(&dtakolog.driver_name)
        .bind(&dtakolog.event_val)
        .bind(&dtakolog.gps_lati_and_long)
        .bind(&dtakolog.odometer)
        .bind(&dtakolog.recive_type_color_name)
        .bind(&dtakolog.recive_type_name)
        .bind(&dtakolog.start_work_date_time)
        .bind(&dtakolog.state)
        .bind(&dtakolog.state1)
        .bind(&dtakolog.state2)
        .bind(&dtakolog.state3)
        .bind(&dtakolog.state_flag)
        .bind(&dtakolog.temp1)
        .bind(&dtakolog.temp2)
        .bind(&dtakolog.temp3)
        .bind(&dtakolog.temp4)
        .bind(&dtakolog.vehicle_icon_color)
        .bind(&dtakolog.vehicle_icon_label_for_datetime)
        .bind(&dtakolog.vehicle_icon_label_for_driver)
        .bind(&dtakolog.vehicle_icon_label_for_vehicle)
        .execute(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to create dtakolog: {}", e)))?;

        Ok(Response::new(CreateDtakologResponse {
            dtakolog: Some(dtakolog),
        }))
    }

    /// 全運行ログ削除
    async fn delete_all(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        tracing::info!("DeleteAll called for organization: {}", organization_id);

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;

        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        let result = sqlx::query("DELETE FROM dtakologs")
            .execute(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Failed to delete dtakologs: {}", e)))?;

        let deleted_count = result.rows_affected() as i32;

        Ok(Response::new(DeleteResponse {
            deleted_count,
            message: format!("Deleted {} dtakologs", deleted_count),
        }))
    }
}
