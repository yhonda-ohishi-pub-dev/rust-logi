use thiserror::Error;
use tonic::Status;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("File is being restored from Glacier, please try again later")]
    RestoreInProgress,

    #[error("File requires restoration from Glacier")]
    RestoreRequired,

    #[error("AWS SDK error: {0}")]
    AwsSdk(#[from] aws_sdk_s3::error::BuildError),
}

impl From<AppError> for Status {
    fn from(err: AppError) -> Self {
        match err {
            AppError::Database(e) => Status::internal(format!("Database error: {}", e)),
            AppError::NotFound(msg) => Status::not_found(msg),
            AppError::InvalidInput(msg) => Status::invalid_argument(msg),
            AppError::Internal(msg) => Status::internal(msg),
            AppError::Storage(msg) => Status::internal(format!("Storage error: {}", msg)),
            AppError::RestoreInProgress => {
                Status::unavailable("File is being restored from Glacier, please try again later")
            }
            AppError::RestoreRequired => {
                Status::failed_precondition("File requires restoration from Glacier")
            }
            AppError::AwsSdk(e) => Status::internal(format!("AWS SDK error: {}", e)),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
