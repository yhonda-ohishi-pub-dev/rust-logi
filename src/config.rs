use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub s3_bucket: Option<String>,
    pub s3_region: String,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        dotenvy::dotenv().ok();

        Ok(Config {
            database_url: env::var("DATABASE_URL")?,
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "50051".to_string())
                .parse()
                .unwrap_or(50051),
            s3_bucket: env::var("S3_BUCKET").ok(),
            s3_region: env::var("AWS_REGION").unwrap_or_else(|_| "ap-northeast-1".to_string()),
        })
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
