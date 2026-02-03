use clap::Parser;
use dotenvy::dotenv;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
struct Cli {
    #[clap(long, env = "SKILLREGISTRY_PORT")]
    port: Option<u16>,

    #[clap(long, env = "SKILLREGISTRY_CONFIG_PATH")]
    config: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    #[serde(default = "default_port")]
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

fn default_port() -> u16 {
    3000
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
    #[serde(default = "default_github_api_url")]
    pub api_url: String,
}

fn default_github_api_url() -> String {
    "https://api.github.com".to_string()
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthSettings {
    #[serde(default)]
    pub enabled: bool,
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

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            frontend_origin: None,
            cookie_domain: None,
            jwt: JwtSettings::default(),
            admin_bootstrap: AdminBootstrapSettings::default(),
            oauth: OAuthSettings::default(),
            sso: SsoSettings::default(),
        }
    }
}

impl Settings {
    #[allow(clippy::result_large_err)]
    pub fn new() -> Result<Self, figment::Error> {
        dotenv().ok();
        let cli = Cli::parse();

        let mut figment = Figment::from(Serialized::defaults(Settings::default_settings()));

        // 1. System Config
        figment = figment.merge(Toml::file("/etc/skillregistry/config.toml"));

        // 2. User Config
        if let Some(config_dir) = dirs::config_dir() {
            figment = figment.merge(Toml::file(config_dir.join("skillregistry/config.toml")));
        }

        // 3. Local Config
        figment = figment.merge(Toml::file("skillregistry.toml"));

        // 4. CLI Config File (Overrides previous files)
        if let Some(config_path) = &cli.config {
            figment = figment.merge(Toml::file(config_path));
        }

        // 5. Environment Variables
        // Prefixed with SKILLREGISTRY_ (e.g. SKILLREGISTRY_PORT=8080, SKILLREGISTRY_S3__BUCKET=foo)
        figment = figment.merge(Env::prefixed("SKILLREGISTRY_"));

        // Support standard AWS Env Vars
        figment = figment.merge(
            Env::raw()
                .only(&["AWS_ACCESS_KEY_ID"])
                .map(|_| "s3.access_key_id".into()),
        );
        figment = figment.merge(
            Env::raw()
                .only(&["AWS_SECRET_ACCESS_KEY"])
                .map(|_| "s3.secret_access_key".into()),
        );
        figment = figment.merge(Env::raw().only(&["AWS_REGION"]).map(|_| "s3.region".into()));
        figment = figment.merge(
            Env::raw()
                .only(&["S3_FORCE_PATH_STYLE"])
                .map(|_| "s3.force_path_style".into()),
        );

        // Support standard GITHUB_TOKEN
        figment = figment.merge(
            Env::raw()
                .only(&["GITHUB_TOKEN"])
                .map(|_| "github.token".into()),
        );

        // 6. CLI Arguments (Overrides everything)
        if let Some(port) = cli.port {
            figment = figment.merge(("port", port));
        }

        figment.extract()
    }

    fn default_settings() -> Settings {
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
                api_url: default_github_api_url(),
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
