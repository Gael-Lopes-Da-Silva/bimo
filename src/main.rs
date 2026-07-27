use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use bimo::api::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

/// Shared application state behind the Axum handlers.
type AppState = Arc<Mutex<BimoApi>>;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let api = BimoApi::new();
    let state: AppState = Arc::new(Mutex::new(api));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Provider routes
        .route("/api/provider/list", get(list_providers))
        .route("/api/provider/select", post(select_provider))
        .route("/api/provider/configure", post(configure_provider))
        .route("/api/provider/add", post(add_custom_provider))
        // Model routes
        .route("/api/model/list", get(list_models))
        .route("/api/model/select", post(select_model))
        // Chat
        .route("/api/chat", post(chat))
        // Session
        .route("/api/session", get(get_session))
        .route("/api/session/clear", post(clear_session))
        // Commands
        .route("/api/command", post(execute_command))
        // Status & help
        .route("/api/status", get(status))
        .route("/api/help", get(help))
        .layer(cors)
        .with_state(state);

    let host = std::env::var("BIMO_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("BIMO_PORT").unwrap_or_else(|_| "3847".into());
    let addr = format!("{host}:{port}");

    tracing::info!("Bimo API server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("server failed");
}

// ---------------------------------------------------------------------------
// Handler functions
// ---------------------------------------------------------------------------

async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    let api = state.lock().await;
    Json(api.list_providers())
}

async fn select_provider(
    State(state): State<AppState>,
    Json(req): Json<SelectProviderRequest>,
) -> impl IntoResponse {
    let mut api = state.lock().await;
    Json(api.select_provider(req).await)
}

async fn configure_provider(
    State(state): State<AppState>,
    Json(req): Json<ConfigureProviderRequest>,
) -> impl IntoResponse {
    let mut api = state.lock().await;
    Json(api.configure_provider(req))
}

async fn add_custom_provider(
    State(state): State<AppState>,
    Json(req): Json<AddCustomProviderRequest>,
) -> impl IntoResponse {
    let mut api = state.lock().await;
    Json(api.add_custom_provider(req))
}

async fn list_models(State(state): State<AppState>) -> impl IntoResponse {
    let mut api = state.lock().await;
    Json(api.list_models().await)
}

async fn select_model(
    State(state): State<AppState>,
    Json(req): Json<SelectModelRequest>,
) -> impl IntoResponse {
    let mut api = state.lock().await;
    Json(api.select_model(req))
}

async fn chat(State(state): State<AppState>, Json(req): Json<ChatRequest>) -> impl IntoResponse {
    let mut api = state.lock().await;
    Json(api.chat(req).await)
}

async fn get_session(State(state): State<AppState>) -> impl IntoResponse {
    let api = state.lock().await;
    Json(api.get_session())
}

async fn clear_session(State(state): State<AppState>) -> impl IntoResponse {
    let mut api = state.lock().await;
    Json(api.clear_session())
}

async fn execute_command(
    State(state): State<AppState>,
    Json(req): Json<CommandRequest>,
) -> impl IntoResponse {
    let mut api = state.lock().await;
    let response = api.execute_command(req);
    let status = if response.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(response))
}

async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let api = state.lock().await;
    Json(api.status())
}

async fn help(State(state): State<AppState>) -> impl IntoResponse {
    let api = state.lock().await;
    Json(api.help())
}
