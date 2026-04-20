pub mod auth;
pub mod links;
pub mod open;
pub mod post;
pub mod search;
pub mod sync;
pub mod topic;
pub mod tree;
pub mod vault_cmd;

use std::path::PathBuf;

use crate::output::Format;

pub struct Context {
    pub team: Option<String>,
    pub token: Option<String>,
    pub vault: Option<PathBuf>,
    pub format: Format,
    pub copy: bool,
}

impl Context {
    pub fn config(&self) -> anyhow::Result<slab_core::Config> {
        slab_core::Config::resolve(self.team.clone(), self.token.clone(), self.vault.clone())
    }

    pub fn client(&self) -> anyhow::Result<slab_core::api::SlabClient> {
        let cfg = self.config()?;
        slab_core::api::SlabClient::new(&cfg)
    }

    pub fn vault(&self) -> anyhow::Result<slab_core::vault::Vault> {
        let cfg = self.config()?;
        slab_core::vault::Vault::open(cfg)
    }
}
