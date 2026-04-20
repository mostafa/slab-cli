use super::Context;
use slab_core::vault::parse_frontmatter_slab_id;

pub fn run(ctx: &Context, id: &str) -> anyhow::Result<()> {
    let cfg = ctx.config()?;

    // Try to resolve a slab URL
    let url = if id.starts_with("http") {
        id.to_string()
    } else {
        // Check if it's a local file path with a slab_id in frontmatter
        let abs_path = cfg.vault_path.join(id);
        if abs_path.exists() {
            let content = std::fs::read_to_string(&abs_path)?;
            if let Some(slab_id) = parse_frontmatter_slab_id(&content) {
                format!("https://{}.slab.com/posts/{slab_id}", cfg.team)
            } else {
                anyhow::bail!("no slab_id in frontmatter for {id}");
            }
        } else {
            format!("https://{}.slab.com/posts/{id}", cfg.team)
        }
    };

    opener::open(&url)?;
    println!("opened: {url}");
    Ok(())
}
