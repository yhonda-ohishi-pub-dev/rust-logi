use std::env;

#[derive(Clone, Debug)]
pub struct CamConfig {
    pub digest_user: String,
    pub digest_pass: String,
    pub machine_name: String,
    pub sdcard_cgi: String,
    pub mp4_cgi: String,
    pub jpg_cgi: String,
    pub cf_access_client_id: Option<String>,
    pub cf_access_client_secret: Option<String>,
}

impl CamConfig {
    pub fn from_env() -> Option<Self> {
        let digest_user = env::var("CAM_DIGEST_USER").ok()?;
        let digest_pass = env::var("CAM_DIGEST_PASS").ok()?;
        let machine_name = env::var("CAM_MACHINE_NAME").ok()?;
        let sdcard_cgi = env::var("CAM_SDCARD_CGI").ok()?;
        let mp4_cgi = env::var("CAM_MP4_CGI").ok()?;
        let jpg_cgi = env::var("CAM_JPG_CGI").ok()?;
        let cf_access_client_id = env::var("CAM_CF_ACCESS_CLIENT_ID").ok();
        let cf_access_client_secret = env::var("CAM_CF_ACCESS_CLIENT_SECRET").ok();
        Some(Self { digest_user, digest_pass, machine_name, sdcard_cgi, mp4_cgi, jpg_cgi, cf_access_client_id, cf_access_client_secret })
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub gcs_bucket: Option<String>,
    pub dtako_api_url: String,
    pub dvr_notification_enabled: bool,
    pub dvr_lineworks_bot_url: Option<String>,
    pub cam_config: Option<CamConfig>,
    pub jwt_secret: String,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        dotenvy::dotenv().ok();

        Ok(Config {
            database_url: env::var("DATABASE_URL")?,
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("PORT")  // Cloud Run sets PORT
                .or_else(|_| env::var("SERVER_PORT"))
                .unwrap_or_else(|_| "50051".to_string())
                .parse()
                .unwrap_or(50051),
            gcs_bucket: env::var("GCS_BUCKET").ok(),
            dtako_api_url: env::var("DTAKO_API_URL").unwrap_or_else(|_| {
                "https://hono-api.mtamaramu.com/api/dtakologs/currentListAllHome".to_string()
            }),
            dvr_notification_enabled: env::var("DVR_NOTIFICATION_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            dvr_lineworks_bot_url: env::var("DVR_LINEWORKS_BOT_URL").ok(),
            cam_config: CamConfig::from_env(),
            jwt_secret: env::var("JWT_SECRET")?,
        })
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
