use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::Response,
    routing::{get, post},
};
use bimo_api::api::*;
use bimo_api::provider;
use bimo_api::tool;
use futures_util::StreamExt;
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
        .route("/api/chat/stream", post(chat_stream))
        .route("/api/session", get(get_session))
        .route("/api/session/clear", post(clear_session))
        .route("/api/command", post(execute_command))
        .route("/api/commands", get(list_commands))
        .route("/api/status", get(status))
        .route("/api/help", get(help))
        .route("/api/session/context", get(get_context))
        .route("/api/thinking", get(get_thinking))
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

async fn chat_stream(State(state): State<AppState>, Json(req): Json<ChatRequest>) -> Response {
    tracing::info!(message_len = req.message.len(), "POST /api/chat/stream");

    let (tx, mut rx) =
        tokio::sync::mpsc::channel::<Result<ChatStreamEvent, bimo_api::BimoError>>(64);

    tokio::spawn(async move {
        let result = run_chat_stream(state, &req.message, &tx).await;
        if let Err(e) = result {
            let _ = tx
                .send(Ok(ChatStreamEvent::Error {
                    message: e.to_string(),
                }))
                .await;
        }
    });

    let stream = async_stream::stream! {
        while let Some(msg) = rx.recv().await {
            match msg {
                Ok(event) => {
                    let data = match serde_json::to_string(&event) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    yield Ok::<_, std::convert::Infallible>(format!("data: {data}\n\n"));
                }
                Err(_) => break,
            }
        }
    };

    let pinned = Box::pin(stream);

    Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(axum::body::Body::from_stream(pinned))
        .unwrap()
}

/// Runs the agent tool-calling loop, sending streaming events through `tx`.
async fn run_chat_stream(
    state: AppState,
    user_message: &str,
    tx: &tokio::sync::mpsc::Sender<Result<ChatStreamEvent, bimo_api::BimoError>>,
) -> bimo_api::Result<()> {
    const MAX_TOOL_ITERATIONS: usize = 20;

    let mut first = true;

    for iteration in 0..=MAX_TOOL_ITERATIONS {
        {
            let mut api = state.lock().await;
            let runtime = api
                .agent
                .runtime
                .as_ref()
                .ok_or_else(|| bimo_api::BimoError::Provider("no provider selected".into()))?
                .clone();
            let model_id = api
                .agent
                .config
                .selected_model
                .clone()
                .ok_or_else(|| bimo_api::BimoError::Model("no model selected".into()))?;
            let thinking = api.agent.config.thinking.clone();

            if first {
                api.agent.session.add_user_message(user_message);
                first = false;
            }

            let messages = api.agent.session.to_chat_messages();
            let mut stream = Box::pin(
                provider::chat_completion_streaming(&runtime, &messages, &model_id, &thinking)
                    .await?,
            );

            let mut content = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(delta) =
                            provider::extract_stream_delta(&chunk, &runtime.request_body_format)
                        {
                            content.push_str(&delta);
                            let _ = tx.try_send(Ok(ChatStreamEvent::Content {
                                delta: delta.clone(),
                            }));
                        }
                    }
                    Err(e) => {
                        let _ = tx.try_send(Err(e));
                        return Ok(());
                    }
                }
            }

            let tool_calls = tool::call::parse_tool_calls(&content);

            if tool_calls.is_empty() || iteration == MAX_TOOL_ITERATIONS {
                let sid = api.agent.session.id.clone();
                api.agent.session.add_assistant_message(&content);
                let _ = tx.try_send(Ok(ChatStreamEvent::Done {
                    model: Some(model_id),
                    usage: None,
                    session_id: sid,
                }));
                return Ok(());
            }

            api.agent.session.add_assistant_message(&content);

            for call in &tool_calls {
                let _ = tx.try_send(Ok(ChatStreamEvent::ToolStart {
                    tool: call.name.clone(),
                    args: serde_json::to_value(&call.arguments).ok(),
                }));

                let result = tool::call::execute_tool_call(call, &api.agent.tool_registry).await;

                let _ = tx.try_send(Ok(ChatStreamEvent::ToolResult {
                    tool: result.name.clone(),
                    is_error: result.is_error,
                }));

                let result_msg = tool::call::format_tool_result_message(&result);
                api.agent.session.add_tool_message(&result_msg);
            }
        }
    }

    Ok(())
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

async fn get_context(State(state): State<AppState>) -> Json<ApiResponse> {
    tracing::debug!("GET /api/session/context");
    let api = state.lock().await;
    let resp = api.get_context();
    tracing::debug!(success = resp.success, "GET /api/session/context -> ok");
    Json(resp)
}

async fn get_thinking(State(state): State<AppState>) -> Json<ApiResponse> {
    tracing::debug!("GET /api/thinking");
    let api = state.lock().await;
    let resp = api.get_thinking();
    tracing::debug!(success = resp.success, "GET /api/thinking -> ok");
    Json(resp)
}
