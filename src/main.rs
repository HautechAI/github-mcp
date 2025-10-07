mod cli;
pub mod config;
pub mod http;
pub mod mcp;
mod server;
mod tools;
pub mod types;

// clap::Parser is not used directly here; clap is used in cli module.

fn main() -> anyhow::Result<()> {
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
