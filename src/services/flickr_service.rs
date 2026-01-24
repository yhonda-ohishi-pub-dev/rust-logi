use sqlx::PgPool;
use tonic::{Request, Response, Status};
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::db::{get_organization_from_request, set_current_organization};
use crate::proto::common::Empty;
use crate::proto::flickr::flickr_service_server::FlickrService;
use crate::proto::flickr::{
    AuthorizationUrlResponse, CallbackRequest, TokenResponse,
};

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
    fn generate_signature(
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
    fn percent_encode(s: &str) -> String {
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
    fn generate_nonce() -> String {
        uuid::Uuid::new_v4().to_string().replace("-", "")
    }

    /// タイムスタンプ生成
    fn generate_timestamp() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
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
}
