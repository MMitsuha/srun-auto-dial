use super::auth::api_key_middleware;
use super::handlers;
use crate::config::Config;
use crate::error::Result;
use crate::service::SrunService;
use axum::Router;
use axum::middleware;
use axum::routing::{get, post};
use rtnetlink::Handle;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

pub async fn run(config: Arc<Config>, handle: Handle) -> Result<()> {
    let service = Arc::new(SrunService::new(config.clone(), handle));

    let api_key = config.server.api_key.clone();

    let app = Router::new()
        .route("/api/health", get(handlers::health))
        .route("/api/status", get(handlers::status))
        .route("/api/interfaces", get(handlers::list_interfaces))
        .route("/api/login/local", post(handlers::login_local))
        .route("/api/logout/local", post(handlers::logout_local))
        .route("/api/login/macvlan", post(handlers::login_macvlan))
        .route("/api/logout/macvlan", post(handlers::logout_macvlan))
        .route("/api/login/random", post(handlers::login_random))
        .layer(middleware::from_fn(move |req, next| {
            api_key_middleware(req, next, api_key.clone())
        }))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(service);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&addr).await?;
    info!(addr = %addr, "API server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
