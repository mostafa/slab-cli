use anyhow::Context as _;
use clap::Subcommand;

use super::Context;

#[derive(Subcommand)]
pub enum VaultAction {
    /// Initialize a new local vault
    Init {
        /// Slab team subdomain
        #[arg(long)]
        team: Option<String>,
        /// Custom vault path
        #[arg(long)]
        path: Option<std::path::PathBuf>,
    },
    /// Show vault info
    Info,
}

pub fn run(ctx: &Context, action: VaultAction) -> anyhow::Result<()> {
    match action {
        VaultAction::Init { team, path } => init(ctx, team, path),
        VaultAction::Info => info(ctx),
    }
}

fn init(
    ctx: &Context,
    team: Option<String>,
    path: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    let team = team
        .or_else(|| ctx.team.clone())
        .or_else(|| std::env::var("SLAB_TEAM").ok())
        .context("--team is required")?;
    let token = ctx
        .token
        .clone()
        .or_else(|| std::env::var("SLAB_API_TOKEN").ok())
        .context("--token or SLAB_API_TOKEN is required")?;

    let vault_path = path
        .or_else(|| ctx.vault.clone())
        .unwrap_or_else(|| slab_core::config::default_vault_root().join(&team));

    let config = slab_core::Config {
        team,
        token,
        endpoint: std::env::var("SLAB_ENDPOINT")
            .unwrap_or_else(|_| "https://api.slab.com/v1/graphql".into()),
        vault_path: vault_path.clone(),
    };

    slab_core::vault::Vault::init(&config)?;
    println!("vault initialized at {}", vault_path.display());
    Ok(())
}

fn info(ctx: &Context) -> anyhow::Result<()> {
    let cfg = ctx.config()?;
    println!("team:     {}", cfg.team);
    println!("vault:    {}", cfg.vault_path.display());
    println!("endpoint: {}", cfg.endpoint);

    let db_path = cfg.state_db_path();
    if db_path.exists() {
        let vault = slab_core::vault::Vault::open(cfg)?;
        let posts = vault.list_tracked_posts()?;
        println!("posts:    {}", posts.len());
    } else {
        println!("posts:    (no state db — run `slab pull` first)");
    }
    Ok(())
}
