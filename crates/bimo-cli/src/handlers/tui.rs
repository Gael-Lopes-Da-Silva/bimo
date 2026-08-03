//! TUI command handler — lists themes or launches the interactive TUI.

use bimo_core::error::CustomError;
use bimo_core::session::Session;

use crate::cli::TuiArgs;

pub async fn run(args: &TuiArgs) -> crate::Result<()> {
    if args.list_themes {
        let themes = bimo_tui::list_available_themes()
            .map_err(|e| CustomError::Other(format!("Cannot list themes: {e}")))?;
        for theme in themes {
            println!("{theme}");
        }
        return Ok(());
    }

    let _session = match &args.session {
        Some(id) => Session::load(id)
            .map_err(|e| CustomError::Session(format!("Cannot load session {id}: {e}")))?,
        None => Session::new(),
    };

    // bimo_tui::run_tui(session, args.theme.as_deref()).map_err(|e| CustomError::Other(e.to_string()))
    Ok(())
}
