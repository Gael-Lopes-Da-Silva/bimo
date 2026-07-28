use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
};
use bimo_api::api::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

type AppState = Arc<Mutex<BimoApi>>;

#[tokio::main]
async fn main() {
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
        .route("/api/provider/list", get(list_providers))
        .route("/api/provider/select", post(select_provider))
        .route("/api/provider/configure", post(configure_provider))
        .route("/api/provider/add", post(add_custom_provider))
        .route("/api/model/list", get(list_models))
        .route("/api/model/select", post(select_model))
        .route("/api/chat", post(chat))
        .route("/api/session", get(get_session))
        .route("/api/session/clear", post(clear_session))
        .route("/api/command", post(execute_command))
        .route("/api/commands", get(list_commands))
        .route("/api/status", get(status))
        .route("/api/help", get(help))
        .layer(TraceLayer::new_for_http())
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

fn api_response_to_http(resp: ApiResponse) -> (StatusCode, Json<ApiResponse>) {
    let status = if resp.success {
        StatusCode::OK
    } else {
        if let Some(ref err) = resp.error {
            tracing::warn!(error_code = %err.code, error_msg = %err.message, "request failed");
        }
        StatusCode::BAD_REQUEST
    };
    (status, Json(resp))
}

async fn list_providers(State(state): State<AppState>) -> Json<ApiResponse> {
    tracing::debug!("GET /api/provider/list");
    let api = state.lock().await;
    let resp = api.list_providers();
    tracing::debug!(success = resp.success, "GET /api/provider/list -> ok");
    Json(resp)
}

async fn select_provider(
    State(state): State<AppState>,
    Json(req): Json<SelectProviderRequest>,
) -> (StatusCode, Json<ApiResponse>) {
    tracing::info!(provider_id = %req.provider_id, "POST /api/provider/select");
    let mut api = state.lock().await;
    let resp = api.select_provider(req).await;
    tracing::info!(success = resp.success, "POST /api/provider/select -> done");
    api_response_to_http(resp)
}

async fn configure_provider(
    State(state): State<AppState>,
    Json(req): Json<ConfigureProviderRequest>,
) -> (StatusCode, Json<ApiResponse>) {
    tracing::info!(provider_id = %req.provider_id, has_base_url = req.base_url.is_some(), has_api_key = req.api_key.is_some(), "POST /api/provider/configure");
    let mut api = state.lock().await;
    let resp = api.configure_provider(req);
    tracing::info!(
        success = resp.success,
        "POST /api/provider/configure -> done"
    );
    api_response_to_http(resp)
}

async fn add_custom_provider(
    State(state): State<AppState>,
    Json(req): Json<AddCustomProviderRequest>,
) -> (StatusCode, Json<ApiResponse>) {
    tracing::info!(id = %req.id, name = %req.name, category = %req.category, "POST /api/provider/add");
    let mut api = state.lock().await;
    let resp = api.add_custom_provider(req);
    tracing::info!(success = resp.success, "POST /api/provider/add -> done");
    api_response_to_http(resp)
}

async fn list_models(State(state): State<AppState>) -> Json<ApiResponse> {
    tracing::debug!("GET /api/model/list");
    let mut api = state.lock().await;
    let resp = api.list_models().await;
    tracing::debug!(success = resp.success, "GET /api/model/list -> ok");
    Json(resp)
}

async fn select_model(
    State(state): State<AppState>,
    Json(req): Json<SelectModelRequest>,
) -> (StatusCode, Json<ApiResponse>) {
    tracing::info!(model_id = %req.model_id, "POST /api/model/select");
    let mut api = state.lock().await;
    let resp = api.select_model(req);
    tracing::info!(success = resp.success, "POST /api/model/select -> done");
    api_response_to_http(resp)
}

async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> (StatusCode, Json<ApiResponse>) {
    tracing::info!(message_len = req.message.len(), "POST /api/chat");
    let mut api = state.lock().await;
    let resp = api.chat(req).await;
    tracing::info!(success = resp.success, "POST /api/chat -> done");
    api_response_to_http(resp)
}

async fn get_session(State(state): State<AppState>) -> Json<ApiResponse> {
    tracing::debug!("GET /api/session");
    let api = state.lock().await;
    let resp = api.get_session();
    tracing::debug!(success = resp.success, "GET /api/session -> ok");
    Json(resp)
}

async fn clear_session(State(state): State<AppState>) -> Json<ApiResponse> {
    tracing::info!("POST /api/session/clear");
    let mut api = state.lock().await;
    let resp = api.clear_session();
    tracing::info!("POST /api/session/clear -> done");
    Json(resp)
}

async fn execute_command(
    State(state): State<AppState>,
    Json(req): Json<CommandRequest>,
) -> (StatusCode, Json<ApiResponse>) {
    tracing::info!(command = %req.command, "POST /api/command");
    let mut api = state.lock().await;
    let resp = api.execute_command(req).await;
    tracing::info!(success = resp.success, "POST /api/command -> done");
    api_response_to_http(resp)
}

async fn status(State(state): State<AppState>) -> Json<ApiResponse> {
    tracing::debug!("GET /api/status");
    let api = state.lock().await;
    let resp = api.status();
    tracing::debug!(success = resp.success, "GET /api/status -> ok");
    Json(resp)
}

async fn help(State(state): State<AppState>) -> Json<ApiResponse> {
    tracing::debug!("GET /api/help");
    let api = state.lock().await;
    let resp = api.help();
    tracing::debug!(success = resp.success, "GET /api/help -> ok");
    Json(resp)
}

async fn list_commands(State(state): State<AppState>) -> Json<ApiResponse> {
    tracing::debug!("GET /api/commands");
    let api = state.lock().await;
    let resp = api.list_commands();
    tracing::debug!(success = resp.success, "GET /api/commands -> ok");
    Json(resp)
}
