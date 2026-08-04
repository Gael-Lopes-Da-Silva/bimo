use std::sync::{Arc, OnceLock};

use aisdk::core::tools::Tool;
use aisdk::macros::tool;
use tracing::info;

/// A function that prompts the user with a question and returns their answer.
pub type QuestionHandler = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

static QUESTION_HANDLER: OnceLock<QuestionHandler> = OnceLock::new();

/// Installs the handler used by the [`ask_user`] tool to prompt the user.
///
/// Embeddings (the CLI, and later the TUI) must install a handler that routes
/// the question to their UI; without one [`ask_user`] fails with an error.
/// Installing twice returns an error.
pub fn set_question_handler(
    handler: impl Fn(&str) -> Result<String, String> + Send + Sync + 'static,
) -> Result<(), String> {
    QUESTION_HANDLER
        .set(Arc::new(handler))
        .map_err(|_| "a question handler is already installed".to_string())
}

/// Returns `true` if a custom question handler has been installed.
pub fn has_question_handler() -> bool {
    QUESTION_HANDLER.get().is_some()
}

/// Asks the user a question and returns their answer as text.
///
/// Fails if no question handler is installed (see [`set_question_handler`]).
/// The answer is returned verbatim so the model can use it to continue its
/// task.
#[tool(
    name = "ask_user",
    desc = "Ask the user a question to clarify a point before continuing. Provide the question you need answered. The user's answer is returned as text. Use this only when you genuinely need input that only the user can provide."
)]
pub fn ask_user(question: String) -> Tool {
    info!("Asking user: {}", question);
    match QUESTION_HANDLER.get() {
        Some(handler) => handler(&question),
        None => Err("ask_user is unavailable: no question handler is installed".to_string()),
    }
}
