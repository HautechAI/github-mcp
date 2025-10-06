use std::env;

/// Runtime configuration for GitHub API clients.
/// Values are sourced from environment variables with sensible defaults.
#[derive(Debug, Clone)]
pub struct Config {
    pub token: String,
    pub api_url: String,
    pub graphql_url: String,
    pub api_version: String,
    pub user_agent: String,
    pub timeout_secs: u64,
}

impl Config {
    /// Load configuration from environment.
    ///
    /// Env vars:
    /// - GITHUB_TOKEN (or GH_TOKEN) [required]
    /// - GITHUB_API_URL (default: https://api.github.com)
    /// - GITHUB_GRAPHQL_URL (default: <GITHUB_API_URL>/graphql)
    /// - GITHUB_API_VERSION (default: 2022-11-28)
    /// - GITHUB_HTTP_TIMEOUT_SECS (default: 30)
    /// - GITHUB_USER_AGENT (default: github-mcp/<version>)
    pub fn from_env() -> Result<Self, String> {
        let token = env::var("GITHUB_TOKEN")
            .or_else(|_| env::var("GH_TOKEN"))
            .map_err(|_| "Missing GITHUB_TOKEN or GH_TOKEN".to_string())?;

        let api_url =
            env::var("GITHUB_API_URL").unwrap_or_else(|_| "https://api.github.com".to_string());
        let graphql_url = env::var("GITHUB_GRAPHQL_URL").unwrap_or_else(|_| {
            let mut base = api_url.trim_end_matches('/').to_string();
            base.push_str("/graphql");
            base
        });
        let api_version =
            env::var("GITHUB_API_VERSION").unwrap_or_else(|_| "2022-11-28".to_string());
        let timeout_secs = env::var("GITHUB_HTTP_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        let default_ua = format!(
            "github-mcp/{} (+https://github.com/HautechAI/github-mcp)",
            env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into())
        );
        let user_agent = env::var("GITHUB_USER_AGENT").unwrap_or(default_ua);

        Ok(Self {
            token,
            api_url,
            graphql_url,
            api_version,
            user_agent,
            timeout_secs,
        })
    }
}
