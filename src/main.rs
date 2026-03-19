mod api;
mod config;
mod error;
mod net;
mod service;
mod srun;
mod tui;

use clap::{Parser, Subcommand};
use config::Config;
use error::{Result, SrunError};
use service::SrunService;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "srun-auto-dial", version, about = "Srun campus network auto-dialer")]
struct Cli {
    /// Path to config file (default: srun.toml)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Increase log verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Interactive TUI mode
    Tui,
    /// Start REST API server
    Server {
        /// Override server port
        #[arg(short, long)]
        port: Option<u16>,
        /// Override server host
        #[arg(long)]
        host: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = match cli.verbose {
        0 => "srun_auto_dial=warn",
        1 => "srun_auto_dial=info",
        2 => "srun_auto_dial=debug",
        _ => "srun_auto_dial=trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    let mut config = Config::load(cli.config.as_deref())?;

    // Set up rtnetlink connection
    let (connection, handle, _) =
        rtnetlink::new_connection().map_err(|e| SrunError::Io(e))?;
    tokio::spawn(connection);

    match cli.command {
        Command::Tui => {
            let config = Arc::new(config);
            let service = Arc::new(SrunService::new(config, handle));
            tui::run(service).await
        }
        Command::Server { port, host } => {
            // CLI overrides config file
            if let Some(p) = port {
                config.server.port = p;
            }
            if let Some(h) = host {
                config.server.host = h;
            }
            let config = Arc::new(config);
            api::run(config, handle).await
        }
    }
}
