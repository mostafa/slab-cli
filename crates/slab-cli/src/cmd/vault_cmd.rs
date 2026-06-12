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
    let path = path.or_else(|| ctx.vault.clone());

    // Resolve from saved config (auth login), env vars, or explicit flags.
    let mut config = slab_core::Config::resolve(
        team.or_else(|| ctx.team.clone()),
        ctx.token.clone(),
        path.clone(),
    )
    .context("no saved config found — run `slab auth login` or pass --team with SLAB_API_TOKEN")?;

    if let Some(p) = path {
        config.vault_path = p;
    }

    slab_core::vault::Vault::init(&config)?;
    println!("vault initialized at {}", config.vault_path.display());
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
