use std::collections::HashMap;
use md5::{Md5, Digest as Md5Digest};
use quick_xml::events::Event;
use quick_xml::Reader;
use sqlx::{FromRow, PgPool};
use tonic::{Request, Response, Status};

use crate::config::CamConfig;
use crate::db::{get_organization_from_request, set_current_organization};
use crate::models::{CamFileExeModel, CamFileExeStageModel, CamFileModel};
use crate::proto::cam_files::cam_file_exe_stage_service_server::CamFileExeStageService;
use crate::proto::cam_files::cam_files_service_server::CamFilesService;
use crate::proto::cam_files::{
    CamFile, CamFileExe, CamFileExeResponse, CamFileExeStage, CreateCamFileExeRequest,
    CreateStageRequest, ListCamFileDatesResponse, ListCamFilesRequest, ListCamFilesResponse,
    ListStagesResponse, StageResponse, SyncCamFilesRequest, SyncCamFilesResponse,
};
use crate::proto::common::Empty;
use crate::proto::flickr::FlickrPhoto;
use crate::services::flickr_service::{FlickrConfig, FlickrServiceImpl, FlickrTokenRow};

/// cam_files LEFT JOIN flickr_photo の結果行
#[derive(FromRow)]
struct CamFileWithFlickrRow {
    name: String,
    date: String,
    hour: String,
    #[sqlx(rename = "type")]
    file_type: String,
    cam: String,
    flickr_id: Option<String>,
    fp_secret: Option<String>,
    fp_server: Option<String>,
}

pub struct CamFilesServiceImpl {
    pool: PgPool,
    http_client: reqwest::Client,
    cam_config: Option<CamConfig>,
    flickr_config: Option<FlickrConfig>,
}

impl CamFilesServiceImpl {
    pub fn new(pool: PgPool, cam_config: Option<CamConfig>, flickr_config: Option<FlickrConfig>) -> Self {
        Self {
            pool,
            http_client: reqwest::Client::new(),
            cam_config,
            flickr_config,
        }
    }

    fn row_to_proto(row: &CamFileWithFlickrRow) -> CamFile {
        let flickr_photo = row.flickr_id.as_ref().and_then(|fid| {
            row.fp_secret.as_ref().map(|secret| FlickrPhoto {
                id: fid.clone(),
                secret: secret.clone(),
                server: row.fp_server.clone().unwrap_or_default(),
            })
        });
        CamFile {
            name: row.name.clone(),
            date: row.date.clone(),
            hour: row.hour.clone(),
            r#type: row.file_type.clone(),
            cam: row.cam.clone(),
            flickr_id: row.flickr_id.clone(),
            flickr_photo,
        }
    }

    // ---- Digest認証 ----

    /// www-authenticate ヘッダーをパースして Digest Authorization ヘッダーを生成
    /// hono-logi createCam.ts L235-264 相当
    fn create_digest_auth_header(
        username: &str,
        password: &str,
        method: &str,
        uri: &str,
        www_auth: &str,
    ) -> String {
        let mut params = HashMap::new();
        for part in www_auth.split(',') {
            let part = part.trim();
            if let Some(eq_pos) = part.find('=') {
                let key = part[..eq_pos].trim().trim_start_matches("Digest ");
                let value = part[eq_pos + 1..].trim().trim_matches('"');
                params.insert(key.to_string(), value.to_string());
            }
        }

        let realm = params.get("realm").map(|s| s.as_str()).unwrap_or("");
        let nonce = params.get("nonce").map(|s| s.as_str()).unwrap_or("");
        let qop = params.get("qop").map(|s| s.as_str());

        let nc = "00000001";
        let cnonce = uuid::Uuid::new_v4().to_string().replace('-', "");
        let cnonce = &cnonce[..13];

        let ha1 = format!("{:x}", Md5::digest(format!("{}:{}:{}", username, realm, password)));
        let ha2 = format!("{:x}", Md5::digest(format!("{}:{}", method, uri)));

        let response = if let Some(qop_val) = qop {
            format!("{:x}", Md5::digest(format!(
                "{}:{}:{}:{}:{}:{}", ha1, nonce, nc, cnonce, qop_val, ha2
            )))
        } else {
            format!("{:x}", Md5::digest(format!("{}:{}:{}", ha1, nonce, ha2)))
        };

        let mut header = format!(
            "Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", response=\"{}\"",
            username, realm, nonce, uri, response
        );
        if let Some(qop_val) = qop {
            header.push_str(&format!(
                ", qop={}, nc={}, cnonce=\"{}\"",
                qop_val, nc, cnonce
            ));
        }
        header
    }

    /// Digest認証付きHTTP GET
    /// hono-logi createCam.ts L267-285 相当
    fn apply_cf_access_headers(
        builder: reqwest::RequestBuilder,
        cam_config: &CamConfig,
    ) -> reqwest::RequestBuilder {
        let builder = if let Some(ref id) = cam_config.cf_access_client_id {
            builder.header("CF-Access-Client-Id", id)
        } else { builder };
        if let Some(ref secret) = cam_config.cf_access_client_secret {
            builder.header("CF-Access-Client-Secret", secret)
        } else { builder }
    }

    async fn authenticated_fetch(
        client: &reqwest::Client,
        url: &str,
        cam_config: &CamConfig,
    ) -> Result<reqwest::Response, String> {
        let response = Self::apply_cf_access_headers(client.get(url), cam_config)
            .send().await
            .map_err(|e| format!("HTTP request failed for {}: {}", url, e))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            let www_auth = response.headers()
                .get("www-authenticate")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if www_auth.contains("Digest") {
                let auth_header = Self::create_digest_auth_header(
                    &cam_config.digest_user,
                    &cam_config.digest_pass,
                    "GET",
                    url,
                    www_auth,
                );
                return Self::apply_cf_access_headers(client.get(url), cam_config)
                    .header("Authorization", auth_header)
                    .send()
                    .await
                    .map_err(|e| format!("Authenticated request failed for {}: {}", url, e));
            }
        }
        Ok(response)
    }

    // ---- XML解析 ----

    /// <Dir Name="20250323"/> のName属性を抽出
    /// hono-logi createCam.ts L320-336 相当
    fn parse_dir_names(xml_text: &str) -> Vec<String> {
        let mut reader = Reader::from_str(xml_text);
        let mut dirs = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    if e.name().as_ref() == b"Dir" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"name" {
                                if let Ok(val) = String::from_utf8(attr.value.to_vec()) {
                                    dirs.push(val);
                                }
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    tracing::warn!("XML parse error (Dir): {}", e);
                    break;
                }
                _ => {}
            }
            buf.clear();
        }
        dirs
    }

    /// <Name>Event20250323_005902.jpg</Name> のテキストを抽出
    /// _! を含むファイル名はスキップ (カメラ一時ファイル)
    /// hono-logi createCam.ts L386-416 相当
    fn parse_file_names(xml_text: &str) -> Vec<String> {
        let mut reader = Reader::from_str(xml_text);
        let mut files = Vec::new();
        let mut buf = Vec::new();
        let mut in_name = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    if e.name().as_ref() == b"Name" {
                        in_name = true;
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_name {
                        if let Ok(text) = e.unescape() {
                            let filename = text.to_string();
                            if !filename.contains("_!") {
                                files.push(filename);
                            }
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == b"Name" {
                        in_name = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    tracing::warn!("XML parse error (Name): {}", e);
                    break;
                }
                _ => {}
            }
            buf.clear();
        }
        files
    }

    // ---- Flickr アップロード (バックグラウンド) ----

    /// flickr_id IS NULL のファイルをバックグラウンドで Flickr アップロード
    /// hono-logi createCam.ts L430-478 相当
    async fn spawn_flickr_uploads(
        &self,
        conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
        start_date: &str,
        organization_id: &str,
        cam_config: &CamConfig,
    ) -> Result<i32, Status> {
        let flickr_config = match self.flickr_config.as_ref() {
            Some(c) => c.clone(),
            None => {
                tracing::info!("Flickr not configured, skipping uploads");
                return Ok(0);
            }
        };

        let token: Option<FlickrTokenRow> = sqlx::query_as(
            "SELECT access_token, access_token_secret FROM flickr_tokens LIMIT 1"
        )
        .fetch_optional(&mut **conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to query flickr_tokens: {}", e)))?;

        let token = match token {
            Some(t) => t,
            None => {
                tracing::info!("No Flickr access token, skipping uploads");
                return Ok(0);
            }
        };

        let unuploaded: Vec<CamFileModel> = sqlx::query_as(
            r#"
            SELECT name, date, hour, type, cam, flickr_id
            FROM cam_files
            WHERE date >= $1 AND flickr_id IS NULL
            LIMIT 100
            "#,
        )
        .bind(start_date)
        .fetch_all(&mut **conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to query unuploaded files: {}", e)))?;

        let count = unuploaded.len() as i32;
        if count == 0 {
            tracing::info!("No unuploaded files found");
            return Ok(0);
        }

        tracing::info!("Starting {} Flickr uploads in background", count);

        let pool = self.pool.clone();
        let http_client = self.http_client.clone();
        let cam_config = cam_config.clone();
        let org_id = organization_id.to_string();

        tokio::spawn(async move {
            for file in unuploaded {
                match upload_file_to_flickr(
                    &pool,
                    &http_client,
                    &cam_config,
                    &flickr_config,
                    &token,
                    &file,
                    &org_id,
                ).await {
                    Ok(flickr_id) => {
                        tracing::info!("Flickr upload success: {} -> {}", file.name, flickr_id);
                    }
                    Err(e) => {
                        tracing::warn!("Flickr upload failed for {}: {}", file.name, e);
                    }
                }
            }
            tracing::info!("Background Flickr uploads completed");
        });

        Ok(count)
    }
}

/// カメラからファイルをダウンロードし Flickr にアップロード
/// hono-logi createCam.ts L446-474 相当
async fn upload_file_to_flickr(
    pool: &PgPool,
    http_client: &reqwest::Client,
    cam_config: &CamConfig,
    flickr_config: &FlickrConfig,
    token: &FlickrTokenRow,
    file: &CamFileModel,
    organization_id: &str,
) -> Result<String, String> {
    let dir_path = "/Event";
    let base_url = if file.name.contains(".mp4") {
        &cam_config.mp4_cgi
    } else {
        &cam_config.jpg_cgi
    };
    let download_url = format!(
        "{}{}{}/{}/{}/{}",
        base_url, cam_config.machine_name, dir_path, file.date, file.hour, file.name
    );

    let response = CamFilesServiceImpl::authenticated_fetch(
        http_client,
        &download_url,
        cam_config,
    ).await?;

    let content_type = response.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if content_type != "application/octet-stream" {
        return Err(format!("Unexpected content type for {}: {}", file.name, content_type));
    }

    let data = response.bytes().await
        .map_err(|e| format!("Failed to read file data for {}: {}", file.name, e))?;

    let flickr_id = upload_to_flickr(
        http_client,
        flickr_config,
        &token.access_token,
        &token.access_token_secret,
        &file.name,
        &data,
    ).await?;

    // RLS用にset_current_organizationが必要
    let mut conn = pool.acquire().await
        .map_err(|e| format!("Failed to acquire connection: {}", e))?;
    set_current_organization(&mut conn, organization_id).await
        .map_err(|e| format!("Failed to set organization: {}", e))?;

    sqlx::query(
        "UPDATE cam_files SET flickr_id = $1 WHERE name = $2"
    )
    .bind(&flickr_id)
    .bind(&file.name)
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("Failed to update flickr_id for {}: {}", file.name, e))?;

    Ok(flickr_id)
}

/// OAuth 1.0a 署名付き multipart POST で Flickr にアップロード
/// エンドポイント: https://up.flickr.com/services/upload/
async fn upload_to_flickr(
    http_client: &reqwest::Client,
    flickr_config: &FlickrConfig,
    access_token: &str,
    access_token_secret: &str,
    title: &str,
    data: &[u8],
) -> Result<String, String> {
    let upload_url = "https://up.flickr.com/services/upload/";

    // OAuth + API パラメータ (photo バイナリは署名に含めない)
    let mut params = HashMap::new();
    params.insert("oauth_consumer_key".to_string(), flickr_config.consumer_key.clone());
    params.insert("oauth_nonce".to_string(), FlickrServiceImpl::generate_nonce());
    params.insert("oauth_signature_method".to_string(), "HMAC-SHA1".to_string());
    params.insert("oauth_timestamp".to_string(), FlickrServiceImpl::generate_timestamp());
    params.insert("oauth_token".to_string(), access_token.to_string());
    params.insert("oauth_version".to_string(), "1.0".to_string());
    params.insert("title".to_string(), title.to_string());
    params.insert("tags".to_string(), "upBySytem".to_string()); // hono-logi互換タイポ

    let signature = FlickrServiceImpl::generate_signature(
        "POST",
        upload_url,
        &params,
        &flickr_config.consumer_secret,
        Some(access_token_secret),
    );
    params.insert("oauth_signature".to_string(), signature);

    let form = reqwest::multipart::Form::new()
        .text("title", title.to_string())
        .text("tags", "upBySytem".to_string())
        .text("oauth_consumer_key", params["oauth_consumer_key"].clone())
        .text("oauth_nonce", params["oauth_nonce"].clone())
        .text("oauth_signature_method", params["oauth_signature_method"].clone())
        .text("oauth_timestamp", params["oauth_timestamp"].clone())
        .text("oauth_token", params["oauth_token"].clone())
        .text("oauth_version", params["oauth_version"].clone())
        .text("oauth_signature", params["oauth_signature"].clone())
        .part("photo", reqwest::multipart::Part::bytes(data.to_vec())
            .file_name(title.to_string())
            .mime_str("application/octet-stream")
            .map_err(|e| format!("Failed to set MIME type: {}", e))?
        );

    let response = http_client
        .post(upload_url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Flickr upload request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Flickr upload error: {} - {}", status, body));
    }

    let body = response.text().await
        .map_err(|e| format!("Failed to read Flickr upload response: {}", e))?;

    parse_flickr_photoid(&body)
        .ok_or_else(|| format!("Failed to parse photoid from Flickr response: {}", body))
}

/// Flickr upload レスポンス XML から <photoid>...</photoid> を抽出
fn parse_flickr_photoid(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut in_photoid = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"photoid" {
                    in_photoid = true;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_photoid {
                    return e.unescape().ok().map(|s| s.to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

#[tonic::async_trait]
impl CamFilesService for CamFilesServiceImpl {
    async fn list_cam_files(
        &self,
        request: Request<ListCamFilesRequest>,
    ) -> Result<Response<ListCamFilesResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        let base_select = r#"
            SELECT cf.name, cf.date, cf.hour, cf.type, cf.cam, cf.flickr_id,
                   fp.secret as fp_secret, fp.server as fp_server
            FROM cam_files cf
            LEFT JOIN flickr_photo fp ON cf.flickr_id = fp.id AND cf.organization_id = fp.organization_id
        "#;

        let files = match (req.date, req.cam) {
            (Some(date), Some(cam)) => {
                sqlx::query_as::<_, CamFileWithFlickrRow>(
                    &format!("{} WHERE cf.date = $1 AND cf.cam = $2 ORDER BY cf.hour", base_select),
                )
                .bind(&date)
                .bind(&cam)
                .fetch_all(&mut *conn)
                .await
            }
            (Some(date), None) => {
                sqlx::query_as::<_, CamFileWithFlickrRow>(
                    &format!("{} WHERE cf.date = $1 ORDER BY cf.hour", base_select),
                )
                .bind(&date)
                .fetch_all(&mut *conn)
                .await
            }
            (None, Some(cam)) => {
                sqlx::query_as::<_, CamFileWithFlickrRow>(
                    &format!("{} WHERE cf.cam = $1 ORDER BY cf.date DESC, cf.hour", base_select),
                )
                .bind(&cam)
                .fetch_all(&mut *conn)
                .await
            }
            (None, None) => {
                sqlx::query_as::<_, CamFileWithFlickrRow>(
                    &format!("{} ORDER BY cf.date DESC, cf.hour LIMIT 100", base_select),
                )
                .fetch_all(&mut *conn)
                .await
            }
        }
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let proto_files: Vec<CamFile> = files.iter().map(Self::row_to_proto).collect();

        Ok(Response::new(ListCamFilesResponse {
            files: proto_files,
            pagination: None,
        }))
    }

    async fn list_cam_file_dates(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ListCamFileDatesResponse>, Status> {
        let organization_id = get_organization_from_request(&request);

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        let dates: Vec<(String,)> =
            sqlx::query_as("SELECT DISTINCT date FROM cam_files ORDER BY date DESC")
                .fetch_all(&mut *conn)
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
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        let exe = req
            .exe
            .ok_or_else(|| Status::invalid_argument("exe is required"))?;

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

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
        .fetch_one(&mut *conn)
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

    /// カメラSD同期 + Flickrアップロード
    /// hono-logi createCam.ts 全体 (L59-500) の移植
    async fn sync_cam_files(
        &self,
        request: Request<SyncCamFilesRequest>,
    ) -> Result<Response<SyncCamFilesResponse>, Status> {
        let organization_id = get_organization_from_request(&request);

        let cam_config = self.cam_config.as_ref().ok_or_else(|| {
            Status::failed_precondition(
                "Camera is not configured. Set CAM_DIGEST_USER, CAM_DIGEST_PASS, \
                 CAM_MACHINE_NAME, CAM_SDCARD_CGI, CAM_MP4_CGI, CAM_JPG_CGI."
            )
        })?;

        tracing::info!("SyncCamFiles called for organization: {}", organization_id);

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        // 1. 最終レコード取得 → 開始日決定
        let last_record: Option<CamFileModel> = sqlx::query_as(
            "SELECT name, date, hour, type, cam, flickr_id FROM cam_files ORDER BY name DESC LIMIT 1"
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let last_record = last_record.ok_or_else(|| {
            Status::failed_precondition("No existing cam_files records found. Cannot determine start date.")
        })?;

        let start_date = last_record.date.clone();
        let start_hour = last_record.hour.clone();
        tracing::info!("SyncCamFiles: start_date={}, start_hour={}, last_name={}", start_date, start_hour, last_record.name);

        // 2. カメラからdate一覧取得
        let dir_path = "/Event";
        let dates_url = format!("{}{}{}", cam_config.sdcard_cgi, cam_config.machine_name, dir_path);
        let dates_response = Self::authenticated_fetch(&self.http_client, &dates_url, cam_config).await
            .map_err(|e| Status::internal(format!("Failed to fetch dates: {}", e)))?;
        let dates_xml = dates_response.text().await
            .map_err(|e| Status::internal(format!("Failed to read dates response: {}", e)))?;

        let all_dates = Self::parse_dir_names(&dates_xml);
        let start_date_int: i64 = start_date.parse().unwrap_or(0);
        let dates: Vec<&str> = all_dates.iter()
            .filter(|d| d.parse::<i64>().unwrap_or(0) >= start_date_int)
            .map(|s| s.as_str())
            .collect();
        let processed_dates = dates.len() as i32;
        tracing::info!("Found {} dates (>= {})", processed_dates, start_date);

        // 3. 各dateからhour一覧取得
        let mut hours: Vec<(String, String)> = Vec::new();
        for date in &dates {
            let hours_url = format!("{}{}{}/{}", cam_config.sdcard_cgi, cam_config.machine_name, dir_path, date);
            match Self::authenticated_fetch(&self.http_client, &hours_url, cam_config).await {
                Ok(resp) => {
                    let xml = resp.text().await.unwrap_or_default();
                    let hour_dirs = Self::parse_dir_names(&xml);
                    for hour in hour_dirs {
                        if *date == start_date.as_str() {
                            let hour_int: i64 = hour.parse().unwrap_or(0);
                            let start_hour_int: i64 = start_hour.parse().unwrap_or(0);
                            if hour_int >= start_hour_int {
                                hours.push((date.to_string(), hour));
                            }
                        } else {
                            hours.push((date.to_string(), hour));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch hours for date {}: {}", date, e);
                }
            }
        }
        let processed_hours = hours.len() as i32;
        tracing::info!("Found {} hours", processed_hours);

        // 4. 各(date, hour)からファイル一覧取得 → UPSERT
        let mut new_files_count = 0i32;
        for (date, hour) in &hours {
            let files_url = format!(
                "{}{}{}/{}/{}",
                cam_config.sdcard_cgi, cam_config.machine_name, dir_path, date, hour
            );
            match Self::authenticated_fetch(&self.http_client, &files_url, cam_config).await {
                Ok(resp) => {
                    let xml = resp.text().await.unwrap_or_default();
                    let filenames = Self::parse_file_names(&xml);
                    for filename in filenames {
                        let file_type = if filename.contains(".mp4") { "mp4" } else { "jpg" };
                        match sqlx::query(
                            r#"
                            INSERT INTO cam_files (name, organization_id, date, hour, type, cam)
                            VALUES ($1, $2::uuid, $3, $4, $5, $6)
                            ON CONFLICT (organization_id, name) DO UPDATE SET
                                date = EXCLUDED.date, hour = EXCLUDED.hour,
                                type = EXCLUDED.type, cam = EXCLUDED.cam
                            "#,
                        )
                        .bind(&filename)
                        .bind(&organization_id)
                        .bind(date)
                        .bind(hour)
                        .bind(file_type)
                        .bind(&cam_config.machine_name)
                        .execute(&mut *conn)
                        .await {
                            Ok(_) => new_files_count += 1,
                            Err(e) => tracing::warn!("Failed to upsert cam_file {}: {}", filename, e),
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch files for {}/{}: {}", date, hour, e);
                }
            }
        }
        tracing::info!("Upserted {} files", new_files_count);

        // 5. Flickr アップロード (バックグラウンド)
        let flickr_upload_started = self.spawn_flickr_uploads(
            &mut conn,
            &start_date,
            &organization_id,
            cam_config,
        ).await.unwrap_or(0);

        Ok(Response::new(SyncCamFilesResponse {
            processed_dates,
            processed_hours,
            new_files: new_files_count,
            flickr_upload_started,
            message: format!(
                "Synced {} dates, {} hours, {} files. {} Flickr uploads started.",
                processed_dates, processed_hours, new_files_count, flickr_upload_started
            ),
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
        request: Request<Empty>,
    ) -> Result<Response<ListStagesResponse>, Status> {
        let organization_id = get_organization_from_request(&request);

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

        let stages = sqlx::query_as::<_, CamFileExeStageModel>(
            "SELECT * FROM cam_file_exe_stage ORDER BY stage",
        )
        .fetch_all(&mut *conn)
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
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        let stage = req
            .stage
            .ok_or_else(|| Status::invalid_argument("stage is required"))?;

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Database connection error: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization context: {}", e)))?;

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
        .fetch_one(&mut *conn)
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
