use anyhow::Context as _;
use clap::Subcommand;

use super::Context;

#[derive(Subcommand)]
pub enum AuthAction {
    /// Store API token and verify connection
    Login {
        /// Slab API token (will prompt if not provided)
        #[arg(long)]
        token: Option<String>,
        /// Slab team subdomain
        #[arg(long)]
        team: Option<String>,
    },
    /// Show current authentication status
    Status,
    /// Remove stored credentials
    Logout,
}

pub fn run(ctx: &Context, action: AuthAction) -> anyhow::Result<()> {
    match action {
        AuthAction::Login { token, team } => login(ctx, token, team),
        AuthAction::Status => status(ctx),
        AuthAction::Logout => logout(ctx),
    }
}

fn login(ctx: &Context, token: Option<String>, team: Option<String>) -> anyhow::Result<()> {
    let team = team
        .or_else(|| ctx.team.clone())
        .or_else(|| std::env::var("SLAB_TEAM").ok())
        .or_else(slab_core::config::default_team)
        .context("--team is required for login")?;

    let token = token
        .or_else(|| ctx.token.clone())
        .or_else(|| std::env::var("SLAB_API_TOKEN").ok())
        .context("--token is required for login")?;

    // Reuse a saved config (preserves vault path / endpoint overrides),
    // falling back to a fresh one for first-time logins.
    let config = slab_core::Config::resolve(Some(team.clone()), Some(token), ctx.vault.clone())?;

    // Verify connection
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let client = slab_core::api::SlabClient::new(&config)?;
    rt.block_on(client.verify_auth())?;
    let org = rt.block_on(client.get_organization())?;
    println!("authenticated to {} ({})", org.host, team);

    // Save config
    config.save()?;
    println!("config saved to {}", config.config_file_path().display());

    Ok(())
}

fn status(ctx: &Context) -> anyhow::Result<()> {
    match ctx.config() {
        Ok(cfg) => {
            println!("team:  {}", cfg.team);
            println!("vault: {}", cfg.vault_path.display());
            println!("endpoint: {}", cfg.endpoint);
            println!("token: {}...", &cfg.token[..8.min(cfg.token.len())]);
        }
        Err(_) => {
            println!("not authenticated — run `slab auth login` first");
        }
    }
    Ok(())
}

fn logout(ctx: &Context) -> anyhow::Result<()> {
    if let Ok(cfg) = ctx.config() {
        let path = cfg.config_file_path();
        if path.exists() {
            std::fs::remove_file(&path)?;
            println!("removed {}", path.display());
        }
    }
    println!("logged out");
    Ok(())
}
