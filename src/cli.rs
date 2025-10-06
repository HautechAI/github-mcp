use clap::{Arg, ArgAction, Command};

#[allow(dead_code)]
pub struct CliOptions {
    pub log_level: Option<String>,
    pub version: bool,
}

pub fn build_cli() -> Command {
    Command::new("github-mcp")
        .about("GitHub MCP server (stdio JSON-RPC)")
        .arg(
            Arg::new("log-level")
                .long("log-level")
                .num_args(1)
                .help("Override RUST_LOG level (e.g., info, debug)"),
        )
        .arg(
            Arg::new("version")
                .long("version")
                .help("Print version and exit")
                .action(ArgAction::SetTrue),
        )
}

pub fn init_logging(level: Option<&str>) {
    // Respect explicit level, else default to info, allow env override via RUST_LOG
    if let Some(lvl) = level {
        std::env::set_var("RUST_LOG", lvl);
    } else if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
}
