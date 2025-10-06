mod cli;
pub mod config;
pub mod http;
mod server;
mod tools;
pub mod types;

use clap::Parser;

#[tokio::main(flavor = "current_thread")] // stdio server loops synchronously for now
async fn main() -> anyhow::Result<()> {
    let cmd = cli::build_cli();
    let matches = cmd.get_matches();
    let log_level = matches.get_one::<String>("log-level").cloned();
    let version_flag = matches.get_flag("version");

    cli::init_logging(log_level.as_deref());

    if version_flag {
        println!("github-mcp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    server::run_stdio_server()?;
    Ok(())
}
