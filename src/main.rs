use std::net::SocketAddr;
use std::sync::Arc;

use rust_logi::config::Config;
use rust_logi::db::create_pool;
use rust_logi::http_client::HttpClient;
use rust_logi::middleware::auth::AuthLayer;
use rust_logi::middleware::grpc_web_fix::GrpcWebTrailerFixLayer;
use rust_logi::proto::cam_files::cam_file_exe_stage_service_server::CamFileExeStageServiceServer;
use rust_logi::proto::cam_files::cam_files_service_server::CamFilesServiceServer;
use rust_logi::proto::car_inspection::car_inspection_files_service_server::CarInspectionFilesServiceServer;
use rust_logi::proto::car_inspection::car_inspection_service_server::CarInspectionServiceServer;
use rust_logi::proto::files::files_service_server::FilesServiceServer;
use rust_logi::proto::health::health_server::HealthServer;
use rust_logi::proto::dtakologs::dtakologs_service_server::DtakologsServiceServer;
use rust_logi::proto::flickr::flickr_service_server::FlickrServiceServer;
use rust_logi::proto::dvr_notifications::dvr_notifications_service_server::DvrNotificationsServiceServer;
use rust_logi::proto::auth::auth_service_server::AuthServiceServer;
use rust_logi::proto::organization::organization_service_server::OrganizationServiceServer;
use rust_logi::proto::member::member_service_server::MemberServiceServer;
use rust_logi::proto::sso_settings::sso_settings_service_server::SsoSettingsServiceServer;
use rust_logi::proto::bot_config::bot_config_service_server::BotConfigServiceServer;
use rust_logi::proto::access_request::access_request_service_server::AccessRequestServiceServer;
use rust_logi::proto::items::items_service_server::ItemsServiceServer;
use rust_logi::services::cam_files_service::CamFileExeStageServiceImpl;
use rust_logi::services::flickr_service::FlickrConfig;
use rust_logi::services::{
    CamFilesServiceImpl, CarInspectionFilesServiceImpl, CarInspectionServiceImpl,
    FileAutoParser, FilesServiceImpl, HealthServiceImpl, DtakologsServiceImpl, FlickrServiceImpl,
    DvrNotificationsServiceImpl,
    AuthServiceImpl, OrganizationServiceImpl, MemberServiceImpl,
    SsoSettingsServiceImpl,
    BotConfigServiceImpl,
    AccessRequestServiceImpl,
    ItemsServiceImpl,
};
use rust_logi::storage::{StorageBackend, GcsBackend, R2Backend};

use tonic::transport::Server;
use tonic_reflection::server::Builder as ReflectionBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Include file descriptor for gRPC reflection
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("logi_descriptor");

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

    // Create storage backend based on STORAGE_BACKEND env var
    let storage: Option<Arc<dyn StorageBackend>> = match config.storage_backend.as_deref() {
        Some("r2") => {
            let bucket = config.r2_bucket.as_ref()
                .expect("R2_BUCKET required when STORAGE_BACKEND=r2");
            let account_id = config.r2_account_id.as_ref()
                .expect("R2_ACCOUNT_ID required when STORAGE_BACKEND=r2");
            let access_key = config.r2_access_key.as_ref()
                .expect("R2_ACCESS_KEY required when STORAGE_BACKEND=r2");
            let secret_key = config.r2_secret_key.as_ref()
                .expect("R2_SECRET_KEY required when STORAGE_BACKEND=r2");

            tracing::info!("R2 storage enabled: bucket={}", bucket);
            match R2Backend::new(
                bucket.clone(),
                account_id.clone(),
                access_key.clone(),
                secret_key.clone(),
            ) {
                Ok(backend) => Some(Arc::new(backend)),
                Err(e) => {
                    tracing::error!("Failed to create R2 backend: {}", e);
                    None
                }
            }
        }
        Some("gcs") | None => {
            if let Some(bucket) = &config.gcs_bucket {
                tracing::info!("GCS storage enabled: bucket={}", bucket);
                match GcsBackend::new(bucket.clone()).await {
                    Ok(backend) => Some(Arc::new(backend)),
                    Err(e) => {
                        tracing::error!("Failed to create GCS backend: {}", e);
                        None
                    }
                }
            } else {
                tracing::info!("No storage backend configured, using database blob storage");
                None
            }
        }
        Some(other) => {
            panic!("Unknown STORAGE_BACKEND: '{}'. Expected 'gcs' or 'r2'", other);
        }
    };

    // Create HTTP client for external API calls
    let http_client = Arc::new(HttpClient::new());

    // Create services
    let file_auto_parser = Arc::new(FileAutoParser::new(pool.clone()));
    let files_service = FilesServiceImpl::new(pool.clone(), storage.clone(), file_auto_parser);
    let car_inspection_service = CarInspectionServiceImpl::new(
        pool.clone(),
        http_client.clone(),
        config.dtako_api_url.clone(),
    );
    let car_inspection_files_service = CarInspectionFilesServiceImpl::new(pool.clone());
    let cam_files_service = CamFilesServiceImpl::new(
        pool.clone(),
        config.cam_config.clone(),
        FlickrConfig::from_env(),
    );
    let cam_file_exe_stage_service = CamFileExeStageServiceImpl::new(pool.clone());
    let health_service = HealthServiceImpl::new();
    let dtakologs_service = DtakologsServiceImpl::new(pool.clone());
    let flickr_service = FlickrServiceImpl::new(pool.clone());
    let dvr_notifications_service = DvrNotificationsServiceImpl::new(
        pool.clone(),
        config.clone(),
        http_client.clone(),
        storage.clone(),
    );
    let auth_service = AuthServiceImpl::new(
        pool.clone(),
        config.jwt_secret.clone(),
        config.google_client_id.clone(),
    );
    let organization_service = OrganizationServiceImpl::new(pool.clone());
    let member_service = MemberServiceImpl::new(pool.clone(), config.jwt_secret.clone());
    let sso_settings_service =
        SsoSettingsServiceImpl::new(pool.clone(), config.jwt_secret.clone());
    let bot_config_service =
        BotConfigServiceImpl::new(pool.clone(), config.jwt_secret.clone());
    let access_request_service = AccessRequestServiceImpl::new(
        pool.clone(),
        config.clone(),
        http_client.clone(),
    );

    let items_service = ItemsServiceImpl::new(pool.clone());

    // Auth middleware layer
    let auth_layer = AuthLayer::new(pool.clone(), config.jwt_secret.clone());

    // CORS layer for gRPC-Web
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any)
        .expose_headers(Any);

    // Build reflection service
    let reflection_service = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    // Parse server address
    let addr: SocketAddr = config.server_addr().parse()?;
    tracing::info!("Listening on {}", addr);

    // Build and run server with gRPC-Web support
    Server::builder()
        .accept_http1(true) // Required for gRPC-Web
        .layer(GrpcWebTrailerFixLayer::new()) // Fix trailers-only for CF Containers
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new()) // Enable gRPC-Web
        .layer(auth_layer) // JWT authentication
        .add_service(reflection_service)
        .add_service(FilesServiceServer::new(files_service))
        .add_service(CarInspectionServiceServer::new(car_inspection_service))
        .add_service(CarInspectionFilesServiceServer::new(
            car_inspection_files_service,
        ))
        .add_service(CamFilesServiceServer::new(cam_files_service))
        .add_service(CamFileExeStageServiceServer::new(cam_file_exe_stage_service))
        .add_service(HealthServer::new(health_service))
        .add_service(DtakologsServiceServer::new(dtakologs_service))
        .add_service(FlickrServiceServer::new(flickr_service))
        .add_service(DvrNotificationsServiceServer::new(dvr_notifications_service))
        .add_service(AuthServiceServer::new(auth_service))
        .add_service(OrganizationServiceServer::new(organization_service))
        .add_service(MemberServiceServer::new(member_service))
        .add_service(SsoSettingsServiceServer::new(sso_settings_service))
        .add_service(BotConfigServiceServer::new(bot_config_service))
        .add_service(AccessRequestServiceServer::new(access_request_service))
        .add_service(ItemsServiceServer::new(items_service))
        .serve(addr)
        .await?;

    Ok(())
}
