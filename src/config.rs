use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub gcs_bucket: Option<String>,
    pub dtako_api_url: String,
    pub dvr_notification_enabled: bool,
    pub dvr_lineworks_bot_url: Option<String>,
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
        })
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
