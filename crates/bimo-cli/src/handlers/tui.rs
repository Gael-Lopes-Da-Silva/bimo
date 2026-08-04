use bimo_core::error::CustomError;
use bimo_core::session::Session;

use crate::cli::TuiArgs;

pub async fn run(args: &TuiArgs) -> crate::Result<()> {
    let _session = match &args.session {
        Some(id) => Session::load(id)
            .map_err(|e| CustomError::Session(format!("Cannot load session {id}: {e}")))?,
        None => Session::new(),
    };

    bimo_tui::run().await.expect("Cannot launch TUI");
    Ok(())
}
