use serde::Deserialize;
use sqlx::PgPool;
use tonic::{Request, Response, Status};
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::db::{get_organization_from_request, set_current_organization};
use crate::proto::common::Empty;
use crate::proto::flickr::flickr_service_server::FlickrService;
use crate::proto::flickr::{
    AuthorizationUrlResponse, CallbackRequest, FlickrPhoto,
    ImportFlickrPhotosRequest, ImportFlickrPhotosResponse, TokenResponse,
};

/// Flickr API flickr.photos.getInfo レスポンス
#[derive(Deserialize)]
struct FlickrApiResponse {
    photo: Option<FlickrApiPhoto>,
    stat: String,
}

#[derive(Deserialize)]
struct FlickrApiPhoto {
    id: String,
    server: String,
    secret: String,
}

/// flickr_tokens テーブルのアクセストークン
#[derive(sqlx::FromRow)]
pub(crate) struct FlickrTokenRow {
    pub(crate) access_token: String,
    pub(crate) access_token_secret: String,
}

/// Flickr OAuth 1.0a 設定
#[derive(Clone)]
pub struct FlickrConfig {
    pub consumer_key: String,
    pub consumer_secret: String,
    pub callback_url: String,
}

impl FlickrConfig {
    pub fn from_env() -> Option<Self> {
        let consumer_key = std::env::var("FLICKR_CONSUMER_KEY").ok()?;
        let consumer_secret = std::env::var("FLICKR_CONSUMER_SECRET").ok()?;
        let callback_url = std::env::var("FLICKR_CALLBACK_URL")
            .unwrap_or_else(|_| "https://test.mtamaramu.com/flickr/callback".to_string());

        Some(Self {
            consumer_key,
            consumer_secret,
            callback_url,
        })
    }
}

pub struct FlickrServiceImpl {
    pool: PgPool,
    config: Option<FlickrConfig>,
    http_client: reqwest::Client,
}

impl FlickrServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            config: FlickrConfig::from_env(),
            http_client: reqwest::Client::new(),
        }
    }

    /// OAuth 1.0a署名を生成
    pub(crate) fn generate_signature(
        method: &str,
        url: &str,
        params: &HashMap<String, String>,
        consumer_secret: &str,
        token_secret: Option<&str>,
    ) -> String {
        // パラメータをソートしてエンコード
        let mut sorted_params: Vec<(&String, &String)> = params.iter().collect();
        sorted_params.sort_by(|a, b| a.0.cmp(b.0));

        let param_string: String = sorted_params
            .iter()
            .map(|(k, v)| format!("{}={}", Self::percent_encode(k), Self::percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        // シグネチャベース文字列
        let signature_base = format!(
            "{}&{}&{}",
            method.to_uppercase(),
            Self::percent_encode(url),
            Self::percent_encode(&param_string)
        );

        // 署名キー
        let signing_key = format!(
            "{}&{}",
            Self::percent_encode(consumer_secret),
            token_secret.map(Self::percent_encode).unwrap_or_default()
        );

        // HMAC-SHA1
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, signing_key.as_bytes());
        let signature = ring::hmac::sign(&key, signature_base.as_bytes());
        BASE64.encode(signature.as_ref())
    }

    /// パーセントエンコード (OAuth 1.0a仕様)
    pub(crate) fn percent_encode(s: &str) -> String {
        let mut result = String::new();
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    result.push(byte as char);
                }
                _ => {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        result
    }

    /// ノンス生成
    pub(crate) fn generate_nonce() -> String {
        uuid::Uuid::new_v4().to_string().replace("-", "")
    }

    /// タイムスタンプ生成
    pub(crate) fn generate_timestamp() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    }

    /// Flickr API flickr.photos.getInfo を OAuth 1.0a 署名付きで呼び出し
    async fn call_flickr_get_info(
        &self,
        photo_id: &str,
        config: &FlickrConfig,
        access_token: &str,
        access_token_secret: &str,
    ) -> Result<FlickrApiPhoto, String> {
        let api_url = "https://www.flickr.com/services/rest/";

        // OAuth + APIパラメータ
        let mut params = HashMap::new();
        params.insert("oauth_consumer_key".to_string(), config.consumer_key.clone());
        params.insert("oauth_nonce".to_string(), Self::generate_nonce());
        params.insert("oauth_signature_method".to_string(), "HMAC-SHA1".to_string());
        params.insert("oauth_timestamp".to_string(), Self::generate_timestamp());
        params.insert("oauth_token".to_string(), access_token.to_string());
        params.insert("oauth_version".to_string(), "1.0".to_string());
        params.insert("method".to_string(), "flickr.photos.getInfo".to_string());
        params.insert("photo_id".to_string(), photo_id.to_string());
        params.insert("format".to_string(), "json".to_string());
        params.insert("nojsoncallback".to_string(), "1".to_string());

        // 署名生成
        let signature = Self::generate_signature(
            "GET",
            api_url,
            &params,
            &config.consumer_secret,
            Some(access_token_secret),
        );
        params.insert("oauth_signature".to_string(), signature);

        // OAuthパラメータをAuthorizationヘッダーに、APIパラメータをクエリに分離
        let oauth_keys = [
            "oauth_consumer_key", "oauth_nonce", "oauth_signature_method",
            "oauth_timestamp", "oauth_token", "oauth_version", "oauth_signature",
        ];
        let auth_header: String = params.iter()
            .filter(|(k, _)| oauth_keys.contains(&k.as_str()))
            .map(|(k, v)| format!("{}=\"{}\"", Self::percent_encode(k), Self::percent_encode(v)))
            .collect::<Vec<_>>()
            .join(", ");

        let query_params: Vec<(&str, &str)> = params.iter()
            .filter(|(k, _)| !oauth_keys.contains(&k.as_str()))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let response = self.http_client
            .get(api_url)
            .header("Authorization", format!("OAuth {}", auth_header))
            .query(&query_params)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed for photo {}: {}", photo_id, e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Flickr API error for photo {}: {} - {}", photo_id, status, body));
        }

        let api_response: FlickrApiResponse = response.json().await
            .map_err(|e| format!("Failed to parse Flickr response for photo {}: {}", photo_id, e))?;

        if api_response.stat != "ok" {
            return Err(format!("Flickr API returned stat={} for photo {}", api_response.stat, photo_id));
        }

        api_response.photo
            .ok_or_else(|| format!("No photo data in Flickr response for photo {}", photo_id))
    }
}

#[tonic::async_trait]
impl FlickrService for FlickrServiceImpl {
    /// OAuth認可URL取得
    async fn get_authorization_url(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<AuthorizationUrlResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        tracing::info!(
            "GetAuthorizationUrl called for organization: {}",
            organization_id
        );

        let config = self.config.as_ref().ok_or_else(|| {
            Status::failed_precondition("Flickr OAuth is not configured. Set FLICKR_CONSUMER_KEY and FLICKR_CONSUMER_SECRET.")
        })?;

        // OAuth パラメータ
        let request_token_url = "https://www.flickr.com/services/oauth/request_token";
        let mut oauth_params = HashMap::new();
        oauth_params.insert("oauth_callback".to_string(), config.callback_url.clone());
        oauth_params.insert("oauth_consumer_key".to_string(), config.consumer_key.clone());
        oauth_params.insert("oauth_nonce".to_string(), Self::generate_nonce());
        oauth_params.insert("oauth_signature_method".to_string(), "HMAC-SHA1".to_string());
        oauth_params.insert("oauth_timestamp".to_string(), Self::generate_timestamp());
        oauth_params.insert("oauth_version".to_string(), "1.0".to_string());

        // 署名生成
        let signature = Self::generate_signature(
            "GET",
            request_token_url,
            &oauth_params,
            &config.consumer_secret,
            None,
        );
        oauth_params.insert("oauth_signature".to_string(), signature);

        // Authorization ヘッダー構築
        let auth_header: String = oauth_params
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", Self::percent_encode(k), Self::percent_encode(v)))
            .collect::<Vec<_>>()
            .join(", ");

        // リクエスト送信
        let response = self
            .http_client
            .get(request_token_url)
            .header("Authorization", format!("OAuth {}", auth_header))
            .send()
            .await
            .map_err(|e| Status::internal(format!("Failed to request token: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!("Flickr request token failed: status={}, body={}", status, body);
            return Err(Status::internal(format!(
                "Flickr API error: {} - {}",
                status, body
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| Status::internal(format!("Failed to read response: {}", e)))?;

        // レスポンスをパース (oauth_token=xxx&oauth_token_secret=xxx&oauth_callback_confirmed=true)
        let params: HashMap<String, String> = body
            .split('&')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                Some((parts.next()?.to_string(), parts.next()?.to_string()))
            })
            .collect();

        let oauth_token = params.get("oauth_token").ok_or_else(|| {
            Status::internal("oauth_token not found in response")
        })?;
        let oauth_token_secret = params.get("oauth_token_secret").ok_or_else(|| {
            Status::internal("oauth_token_secret not found in response")
        })?;

        // セッションをDBに保存
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
            INSERT INTO flickr_oauth_sessions (organization_id, request_token, request_token_secret)
            VALUES ($1::uuid, $2, $3)
            "#,
        )
        .bind(&organization_id)
        .bind(oauth_token)
        .bind(oauth_token_secret)
        .execute(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to save OAuth session: {}", e)))?;

        // 認可URL
        let authorization_url = format!(
            "https://www.flickr.com/services/oauth/authorize?oauth_token={}&perms=write",
            oauth_token
        );

        Ok(Response::new(AuthorizationUrlResponse {
            authorization_url,
            request_token: oauth_token.clone(),
            request_token_secret: oauth_token_secret.clone(),
        }))
    }

    /// コールバック処理
    async fn handle_callback(
        &self,
        request: Request<CallbackRequest>,
    ) -> Result<Response<TokenResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        tracing::info!(
            "HandleCallback called for organization: {}, oauth_token: {}",
            organization_id,
            req.oauth_token
        );

        let config = self.config.as_ref().ok_or_else(|| {
            Status::failed_precondition("Flickr OAuth is not configured")
        })?;

        // アクセストークン取得
        let access_token_url = "https://www.flickr.com/services/oauth/access_token";
        let mut oauth_params = HashMap::new();
        oauth_params.insert("oauth_consumer_key".to_string(), config.consumer_key.clone());
        oauth_params.insert("oauth_nonce".to_string(), Self::generate_nonce());
        oauth_params.insert("oauth_signature_method".to_string(), "HMAC-SHA1".to_string());
        oauth_params.insert("oauth_timestamp".to_string(), Self::generate_timestamp());
        oauth_params.insert("oauth_token".to_string(), req.oauth_token.clone());
        oauth_params.insert("oauth_verifier".to_string(), req.oauth_verifier.clone());
        oauth_params.insert("oauth_version".to_string(), "1.0".to_string());

        // 署名生成
        let signature = Self::generate_signature(
            "GET",
            access_token_url,
            &oauth_params,
            &config.consumer_secret,
            Some(&req.request_token_secret),
        );
        oauth_params.insert("oauth_signature".to_string(), signature);

        // Authorization ヘッダー構築
        let auth_header: String = oauth_params
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", Self::percent_encode(k), Self::percent_encode(v)))
            .collect::<Vec<_>>()
            .join(", ");

        // リクエスト送信
        let response = self
            .http_client
            .get(access_token_url)
            .header("Authorization", format!("OAuth {}", auth_header))
            .send()
            .await
            .map_err(|e| Status::internal(format!("Failed to get access token: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!("Flickr access token failed: status={}, body={}", status, body);
            return Err(Status::internal(format!(
                "Flickr API error: {} - {}",
                status, body
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| Status::internal(format!("Failed to read response: {}", e)))?;

        // レスポンスをパース
        let params: HashMap<String, String> = body
            .split('&')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                Some((parts.next()?.to_string(), parts.next()?.to_string()))
            })
            .collect();

        let access_token = params.get("oauth_token").ok_or_else(|| {
            Status::internal("oauth_token not found in access token response")
        })?;
        let access_token_secret = params.get("oauth_token_secret").ok_or_else(|| {
            Status::internal("oauth_token_secret not found in access token response")
        })?;
        let user_nsid = params.get("user_nsid").unwrap_or(&String::new()).clone();
        let username = params.get("username").unwrap_or(&String::new()).clone();

        // トークンをDBに保存
        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;

        set_current_organization(&mut conn, &organization_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        // UPSERT
        sqlx::query(
            r#"
            INSERT INTO flickr_tokens (organization_id, access_token, access_token_secret, user_nsid, username)
            VALUES ($1::uuid, $2, $3, $4, $5)
            ON CONFLICT (organization_id) DO UPDATE SET
                access_token = EXCLUDED.access_token,
                access_token_secret = EXCLUDED.access_token_secret,
                user_nsid = EXCLUDED.user_nsid,
                username = EXCLUDED.username,
                updated_at = NOW()
            "#,
        )
        .bind(&organization_id)
        .bind(access_token)
        .bind(access_token_secret)
        .bind(&user_nsid)
        .bind(&username)
        .execute(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to save access token: {}", e)))?;

        // 古いセッションを削除
        sqlx::query("DELETE FROM flickr_oauth_sessions WHERE request_token = $1")
            .bind(&req.oauth_token)
            .execute(&mut *conn)
            .await
            .ok(); // エラーは無視

        Ok(Response::new(TokenResponse {
            access_token: access_token.clone(),
            access_token_secret: access_token_secret.clone(),
            user_nsid,
            username,
        }))
    }

    /// Flickr写真メタデータインポート
    async fn import_flickr_photos(
        &self,
        request: Request<ImportFlickrPhotosRequest>,
    ) -> Result<Response<ImportFlickrPhotosResponse>, Status> {
        let organization_id = get_organization_from_request(&request);
        let req = request.into_inner();
        let limit = if req.limit > 0 { req.limit } else { 500 };

        tracing::info!(
            "ImportFlickrPhotos called for organization: {}, limit: {}",
            organization_id, limit
        );

        let config = self.config.as_ref().ok_or_else(|| {
            Status::failed_precondition("Flickr OAuth is not configured. Set FLICKR_CONSUMER_KEY and FLICKR_CONSUMER_SECRET.")
        })?;

        let mut conn = self.pool.acquire().await
            .map_err(|e| Status::internal(format!("Failed to acquire connection: {}", e)))?;
        set_current_organization(&mut conn, &organization_id).await
            .map_err(|e| Status::internal(format!("Failed to set organization: {}", e)))?;

        // アクセストークン取得
        let token = sqlx::query_as::<_, FlickrTokenRow>(
            "SELECT access_token, access_token_secret FROM flickr_tokens LIMIT 1"
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to query flickr_tokens: {}", e)))?
        .ok_or_else(|| Status::failed_precondition(
            "No Flickr access token found. Please authorize via GetAuthorizationUrl first."
        ))?;

        // 未検証写真を取得 (cam_files LEFT JOIN flickr_photo)
        let unverified: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT cf.flickr_id
            FROM cam_files cf
            LEFT JOIN flickr_photo fp ON cf.flickr_id = fp.id AND cf.organization_id = fp.organization_id
            WHERE cf.flickr_id IS NOT NULL AND fp.id IS NULL
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to query unverified photos: {}", e)))?;

        if unverified.is_empty() {
            tracing::info!("No unverified Flickr photos found");
            return Ok(Response::new(ImportFlickrPhotosResponse {
                imported_count: 0,
                errors_count: 0,
                remaining_count: 0,
                photos: vec![],
            }));
        }

        tracing::info!("Found {} unverified Flickr photos", unverified.len());

        let mut imported = Vec::new();
        let mut errors_count = 0i32;

        for (flickr_id,) in &unverified {
            match self.call_flickr_get_info(
                flickr_id,
                config,
                &token.access_token,
                &token.access_token_secret,
            ).await {
                Ok(photo) => {
                    // flickr_photoにINSERT
                    match sqlx::query(
                        r#"
                        INSERT INTO flickr_photo (id, organization_id, secret, server)
                        VALUES ($1, $2::uuid, $3, $4)
                        ON CONFLICT (organization_id, id) DO NOTHING
                        "#,
                    )
                    .bind(&photo.id)
                    .bind(&organization_id)
                    .bind(&photo.secret)
                    .bind(&photo.server)
                    .execute(&mut *conn)
                    .await {
                        Ok(_) => {
                            tracing::debug!("Imported flickr_photo: id={}", photo.id);
                            imported.push(FlickrPhoto {
                                id: photo.id,
                                secret: photo.secret,
                                server: photo.server,
                            });
                        }
                        Err(e) => {
                            tracing::warn!("Failed to insert flickr_photo {}: {}", flickr_id, e);
                            errors_count += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch Flickr photo info: {}", e);
                    errors_count += 1;
                }
            }
        }

        // 残りの未検証件数を取得
        let remaining: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM cam_files cf
            LEFT JOIN flickr_photo fp ON cf.flickr_id = fp.id AND cf.organization_id = fp.organization_id
            WHERE cf.flickr_id IS NOT NULL AND fp.id IS NULL
            "#,
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Failed to count remaining: {}", e)))?;

        let imported_count = imported.len() as i32;
        tracing::info!(
            "ImportFlickrPhotos completed: imported={}, errors={}, remaining={}",
            imported_count, errors_count, remaining.0
        );

        Ok(Response::new(ImportFlickrPhotosResponse {
            imported_count,
            errors_count,
            remaining_count: remaining.0 as i32,
            photos: imported,
        }))
    }
}
