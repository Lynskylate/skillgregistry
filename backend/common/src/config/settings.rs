use dotenvy::dotenv;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
struct Cli {
    port: Option<u16>,
    config: Option<String>,
}

fn parse_cli_from_args<I, S>(args: I) -> Cli
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut cli = Cli::default();
    let mut iter = args.into_iter().map(Into::into);

    // Skip binary name
    let _ = iter.next();

    while let Some(arg) = iter.next() {
        if let Some(raw_port) = arg.strip_prefix("--port=") {
            if let Ok(port) = raw_port.parse::<u16>() {
                cli.port = Some(port);
            }
            continue;
        }

        if arg == "--port" {
            if let Some(raw_port) = iter.next() {
                if let Ok(port) = raw_port.parse::<u16>() {
                    cli.port = Some(port);
                }
            }
            continue;
        }

        if let Some(raw_config) = arg.strip_prefix("--config=") {
            if !raw_config.is_empty() {
                cli.config = Some(raw_config.to_string());
            }
            continue;
        }

        if arg == "--config" {
            if let Some(config) = iter.next() {
                if !config.is_empty() {
                    cli.config = Some(config);
                }
            }
        }
    }

    cli
}

fn parse_cli() -> Cli {
    parse_cli_from_args(std::env::args())
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub port: u16,
    pub database: DatabaseSettings,
    pub s3: S3Settings,
    pub github: GithubSettings,
    pub worker: WorkerSettings,
    pub temporal: TemporalSettings,
    #[serde(default)]
    pub auth: AuthSettings,
    #[serde(default)]
    pub debug: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseSettings {
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct S3Settings {
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    #[serde(default)]
    pub force_path_style: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GithubSettings {
    pub search_keywords: String,
    pub token: Option<String>,
    pub api_url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkerSettings {
    pub scan_interval_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TemporalSettings {
    pub server_url: String,
    pub task_queue: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AuthSettings {
    pub frontend_origin: Option<String>,
    pub cookie_domain: Option<String>,
    #[serde(default)]
    pub jwt: JwtSettings,
    #[serde(default)]
    pub admin_bootstrap: AdminBootstrapSettings,
    #[serde(default)]
    pub oauth: OAuthSettings,
    #[serde(default)]
    pub sso: SsoSettings,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JwtSettings {
    pub issuer: String,
    pub audience: String,
    pub signing_key: Option<String>,
    #[serde(default = "default_access_ttl_seconds")]
    pub access_ttl_seconds: i64,
    #[serde(default = "default_refresh_ttl_seconds")]
    pub refresh_ttl_seconds: i64,
}

fn default_access_ttl_seconds() -> i64 {
    15 * 60
}

fn default_refresh_ttl_seconds() -> i64 {
    30 * 24 * 60 * 60
}

impl Default for JwtSettings {
    fn default() -> Self {
        Self {
            issuer: "skillregistry".to_string(),
            audience: "skillregistry".to_string(),
            signing_key: None,
            access_ttl_seconds: default_access_ttl_seconds(),
            refresh_ttl_seconds: default_refresh_ttl_seconds(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AdminBootstrapSettings {
    #[serde(default = "default_admin_username")]
    pub username: String,
    pub password: Option<String>,
}

fn default_admin_username() -> String {
    "admin".to_string()
}

impl Default for AdminBootstrapSettings {
    fn default() -> Self {
        Self {
            username: default_admin_username(),
            password: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct OAuthSettings {
    pub github: Option<OAuthClientSettings>,
    pub google: Option<OAuthClientSettings>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OAuthClientSettings {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct SsoSettings {
    pub base_url: Option<String>,
}

impl Settings {
    #[allow(clippy::result_large_err)]
    pub fn new() -> Result<Self, figment::Error> {
        dotenv().ok();
        let cli = parse_cli();

        let mut figment = Figment::from(Serialized::defaults(Settings::default()));

        figment = figment.merge(Toml::file("/etc/skillregistry/config.toml"));

        if let Some(config_dir) = dirs::config_dir() {
            figment = figment.merge(Toml::file(config_dir.join("skillregistry/config.toml")));
        }

        figment = figment.merge(Toml::file("skillregistry.toml"));

        let config_path = cli
            .config
            .or_else(|| std::env::var("SKILLREGISTRY_CONFIG_PATH").ok());
        if let Some(config_path) = config_path {
            figment = figment.merge(Toml::file(config_path));
        }

        figment = figment.merge(Env::prefixed("SKILLREGISTRY_").split("__"));

        if let Some(port) = cli.port {
            figment = figment.merge(("port", port));
        }

        figment.extract()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            port: 3000,
            debug: false,
            database: DatabaseSettings {
                url: "sqlite://skillregistry.db?mode=rwc".to_string(),
            },
            s3: S3Settings {
                bucket: "skill-registry-bucket".to_string(),
                region: "us-east-1".to_string(),
                endpoint: None,
                access_key_id: None,
                secret_access_key: None,
                force_path_style: false,
            },
            github: GithubSettings {
                search_keywords: "topic:agent-skill".to_string(),
                token: None,
                api_url: "https://api.github.com".to_string(),
            },
            worker: WorkerSettings {
                scan_interval_seconds: 3600,
            },
            temporal: TemporalSettings {
                server_url: "http://localhost:7233".to_string(),
                task_queue: "skill-registry-queue".to_string(),
            },
            auth: AuthSettings::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_cli_from_args;

    #[test]
    fn parse_cli_ignores_unknown_flags() {
        let cli = parse_cli_from_args(["api-bin", "--quiet", "--nocapture", "--port", "4010"]);

        assert_eq!(cli.port, Some(4010));
        assert_eq!(cli.config, None);
    }

    #[test]
    fn parse_cli_supports_equals_syntax() {
        let cli = parse_cli_from_args(["api-bin", "--config=local.toml", "--port=3111"]);

        assert_eq!(cli.port, Some(3111));
        assert_eq!(cli.config.as_deref(), Some("local.toml"));
    }

    #[test]
    fn parse_cli_ignores_invalid_port_values() {
        let cli = parse_cli_from_args(["api-bin", "--port", "invalid"]);

        assert_eq!(cli.port, None);
        assert_eq!(cli.config, None);
    }
}
