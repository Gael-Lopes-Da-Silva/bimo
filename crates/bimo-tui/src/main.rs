mod app;

use std::process::ExitCode;

fn main() -> std::io::Result<ExitCode> {
    tuie::config::update(|cfg| cfg.always_selected = true);

    let model = app::App::new();
    tuie::start_tui(model.into_root())
}
