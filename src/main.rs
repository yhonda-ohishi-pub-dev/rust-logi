use std::net::SocketAddr;

use rust_logi::config::Config;
use rust_logi::db::create_pool;
use rust_logi::proto::cam_files::cam_file_exe_stage_service_server::CamFileExeStageServiceServer;
use rust_logi::proto::cam_files::cam_files_service_server::CamFilesServiceServer;
use rust_logi::proto::car_inspection::car_inspection_files_service_server::CarInspectionFilesServiceServer;
use rust_logi::proto::car_inspection::car_inspection_service_server::CarInspectionServiceServer;
use rust_logi::proto::files::files_service_server::FilesServiceServer;
use rust_logi::services::cam_files_service::CamFileExeStageServiceImpl;
use rust_logi::services::{
    CamFilesServiceImpl, CarInspectionFilesServiceImpl, CarInspectionServiceImpl,
    FilesServiceImpl,
};

use tonic::transport::Server;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rust_logi=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::from_env().expect("Failed to load configuration");

    tracing::info!("Starting rust-logi gRPC server...");
    tracing::info!("Connecting to database...");

    // Create database pool
    let pool = create_pool(&config.database_url).await?;
    tracing::info!("Database connection established");

    // Create services
    let files_service = FilesServiceImpl::new(pool.clone());
    let car_inspection_service = CarInspectionServiceImpl::new(pool.clone());
    let car_inspection_files_service = CarInspectionFilesServiceImpl::new(pool.clone());
    let cam_files_service = CamFilesServiceImpl::new(pool.clone());
    let cam_file_exe_stage_service = CamFileExeStageServiceImpl::new(pool.clone());

    // CORS layer for gRPC-Web
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any)
        .expose_headers(Any);

    // Parse server address
    let addr: SocketAddr = config.server_addr().parse()?;
    tracing::info!("Listening on {}", addr);

    // Build and run server with gRPC-Web support
    Server::builder()
        .accept_http1(true) // Required for gRPC-Web
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new()) // Enable gRPC-Web
        .add_service(FilesServiceServer::new(files_service))
        .add_service(CarInspectionServiceServer::new(car_inspection_service))
        .add_service(CarInspectionFilesServiceServer::new(
            car_inspection_files_service,
        ))
        .add_service(CamFilesServiceServer::new(cam_files_service))
        .add_service(CamFileExeStageServiceServer::new(cam_file_exe_stage_service))
        .serve(addr)
        .await?;

    Ok(())
}
