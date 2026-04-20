use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub team: String,
    pub token: String,
    pub endpoint: String,
    pub vault_path: PathBuf,
}

impl Config {
    pub fn slab_dir(&self) -> PathBuf {
        self.vault_path.join(".slab")
    }

    pub fn state_db_path(&self) -> PathBuf {
        self.slab_dir().join("state.db")
    }

    pub fn config_file_path(&self) -> PathBuf {
        self.slab_dir().join("config.toml")
    }

    /// Load config from a vault directory (looks for `.slab/config.toml`).
    pub fn load_from_vault(vault_path: &Path) -> anyhow::Result<Self> {
        let config_path = vault_path.join(".slab").join("config.toml");
        if !config_path.exists() {
            bail!(
                "No vault config found at {}. Run `slab vault init` first.",
                config_path.display()
            );
        }
        let text = std::fs::read_to_string(&config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;
        let cfg: ConfigFile = toml::de::from_str(&text)
            .with_context(|| format!("parsing {}", config_path.display()))?;
        Ok(Self {
            team: cfg.team,
            token: cfg.token,
            endpoint: cfg
                .endpoint
                .unwrap_or_else(|| "https://api.slab.com/v1/graphql".into()),
            vault_path: vault_path.to_path_buf(),
        })
    }

    /// Build a config from env vars + explicit overrides (for non-vault commands).
    pub fn from_env(
        team: Option<String>,
        token: Option<String>,
        vault: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        let team = team
            .or_else(|| std::env::var("SLAB_TEAM").ok())
            .context("--team or SLAB_TEAM is required")?;
        let token = token
            .or_else(|| std::env::var("SLAB_API_TOKEN").ok())
            .context("--token or SLAB_API_TOKEN is required")?;
        let vault_path = vault
            .or_else(|| std::env::var("SLAB_VAULT").ok().map(PathBuf::from))
            .unwrap_or_else(|| default_vault_root().join(&team));
        Ok(Self {
            team,
            token,
            endpoint: std::env::var("SLAB_ENDPOINT")
                .unwrap_or_else(|_| "https://api.slab.com/v1/graphql".into()),
            vault_path,
        })
    }

    /// Try loading from vault first, then fall back to env vars.
    pub fn resolve(
        team: Option<String>,
        token: Option<String>,
        vault: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        if let Some(ref v) = vault
            && v.join(".slab").join("config.toml").exists()
        {
            let mut cfg = Self::load_from_vault(v)?;
            if let Some(t) = team {
                cfg.team = t;
            }
            if let Some(tk) = token {
                cfg.token = tk;
            }
            return Ok(cfg);
        }

        // Try default vault location if team is known
        let effective_team = team.clone().or_else(|| std::env::var("SLAB_TEAM").ok());
        if let Some(ref t) = effective_team {
            let default_path = default_vault_root().join(t);
            if default_path.join(".slab").join("config.toml").exists() {
                let mut cfg = Self::load_from_vault(&default_path)?;
                if let Some(tk) = token {
                    cfg.token = tk;
                }
                return Ok(cfg);
            }
        }

        Self::from_env(team, token, vault)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let slab_dir = self.slab_dir();
        std::fs::create_dir_all(&slab_dir)?;
        let cfg = ConfigFile {
            team: self.team.clone(),
            token: self.token.clone(),
            endpoint: Some(self.endpoint.clone()),
        };
        let text = toml::ser::to_string_pretty(&cfg)?;
        std::fs::write(self.config_file_path(), text)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    team: String,
    token: String,
    endpoint: Option<String>,
}

pub fn default_vault_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".slab")
}
