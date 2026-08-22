mod charts;
mod cron;
mod daemon;
mod db;
mod demo;
mod editors;
mod engine;
mod exec;
mod herdr;
mod mcp;
mod menus;
mod metrics;
mod panels;
mod reference;
mod settings;
mod splash;
mod status;
mod treebank;
mod ui;
mod workflows;

use clap::Parser;
use std::path::PathBuf;

/// Durable workflows. herdr runs the agents; systemd runs everything else.
#[derive(Parser)]
#[command(name = "powderman")]
struct Cli {
    /// Where the database lives.
    #[arg(long, env = "POWDERMAN_DB")]
    db: Option<PathBuf>,
    #[arg(long, env = "POWDERMAN_PORT", default_value_t = 7777)]
    port: u16,
    /// Print the generated reference and exit. What docs/reference.md holds,
    /// read out of this build's own registries — so it answers "what can this
    /// binary do" without starting a daemon or opening a browser.
    #[arg(long)]
    reference: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.reference {
        print!("{}", reference::markdown(&reference::reference()));
        return Ok(());
    }
    let db = cli.db.unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".local/share/powderman/powderman.db")
    });
    daemon::serve(&db, cli.port).await
}
