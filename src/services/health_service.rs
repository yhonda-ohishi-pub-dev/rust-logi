use tonic::{Request, Response, Status};

use crate::proto::health::{
    health_server::Health, HealthCheckRequest, HealthCheckResponse,
    health_check_response::ServingStatus,
};

#[derive(Debug, Default)]
pub struct HealthServiceImpl;

impl HealthServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

#[tonic::async_trait]
impl Health for HealthServiceImpl {
    async fn check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Ok(Response::new(HealthCheckResponse {
            status: ServingStatus::Serving.into(),
        }))
    }

    type WatchStream = tokio_stream::wrappers::ReceiverStream<Result<HealthCheckResponse, Status>>;

    async fn watch(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(async move {
            let _ = tx.send(Ok(HealthCheckResponse {
                status: ServingStatus::Serving.into(),
            })).await;
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
}
