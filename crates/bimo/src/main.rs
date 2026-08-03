use bimo_core::Session;
use bimo_tui::run_tui;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "bimo", version, about = "Bimo - AI Coding Agent")]
struct Args {
    /// Start the TUI interface
    #[arg(short, long, default_value_t = true)]
    tui: bool,

    /// Session ID to load (optional)
    #[arg(short, long)]
    session: Option<String>,

    /// Theme name to use
    #[arg(long)]
    theme: Option<String>,

    /// List available themes
    #[arg(long)]
    list_themes: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let args = Args::parse();

    if args.list_themes {
        let themes = bimo_tui::list_available_themes()?;
        for theme in themes {
            println!("{theme}");
        }
        return Ok(());
    }

    let session = if let Some(id) = args.session {
        Session::load(&id)?
    } else {
        Session::new()
    };

    if args.tui {
        run_tui(session)?;
    } else {
        println!("CLI mode not yet implemented. Use --tui for the TUI interface.");
    }

    Ok(())
}
