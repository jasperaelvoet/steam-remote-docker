mod admin;
mod audio;
mod codec;
mod compositor;
mod config;
mod dbus;
mod environment;
mod filesystem;
mod health;
mod paths;
mod process;
mod session;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::config::RunArgs;
use crate::paths::DEFAULT_RUNTIME_DIR;

#[derive(Debug, Parser)]
#[command(
    name = "steam-remote",
    about = "Always-on Steam Remote Play host runtime",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the virtual display, Gamescope, audio, and Steam Big Picture.
    Run(RunArgs),
    /// Print runtime readiness without changing the exit status.
    Status(ReportArgs),
    /// Check runtime readiness and exit nonzero when it is not ready.
    Health(ReportArgs),
    /// Manage the loopback-only recovery console.
    Admin(AdminArgs),
}

#[derive(Debug, Clone, Args)]
struct ReportArgs {
    #[arg(long, env = "XDG_RUNTIME_DIR", default_value = DEFAULT_RUNTIME_DIR)]
    runtime_dir: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Args)]
struct AdminArgs {
    #[arg(long, env = "XDG_RUNTIME_DIR", default_value = DEFAULT_RUNTIME_DIR)]
    runtime_dir: PathBuf,
    #[command(subcommand)]
    action: admin::AdminAction,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        match cli.command {
            Command::Run(args) => {
                session::RuntimeSession::start(args.validate()?)
                    .await?
                    .run()
                    .await
            }
            Command::Status(args) => {
                health::HealthReport::gather(&args.runtime_dir).print(args.json)
            }
            Command::Health(args) => {
                let report = health::HealthReport::gather(&args.runtime_dir);
                report.print(args.json)?;
                if !report.healthy {
                    std::process::exit(1);
                }
                Ok(())
            }
            Command::Admin(args) => admin::dispatch(&args.runtime_dir, args.action).await,
        }
    })
}
