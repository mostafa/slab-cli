use std::collections::HashMap;

use super::Context;
use crate::output::{self, Format};

pub fn links(ctx: &Context, file: Option<&str>) -> anyhow::Result<()> {
    let vault = ctx.vault()?;
    let posts = vault.list_tracked_posts()?;

    let target = file.unwrap_or_else(|| posts.first().map(|p| p.path.as_str()).unwrap_or(""));

    let abs_path = vault.config.vault_path.join(target);
    if !abs_path.exists() {
        anyhow::bail!("file not found: {target}");
    }

    let content = std::fs::read_to_string(&abs_path)?;
    let outgoing = extract_links(&content);

    let out = match ctx.format {
        Format::Json => serde_json::to_string_pretty(&outgoing)?,
        _ => {
            let mut s = format!("outgoing links from {target}:\n");
            for link in &outgoing {
                s.push_str(&format!("  -> {link}\n"));
            }
            if outgoing.is_empty() {
                s.push_str("  (none)\n");
            }
            s
        }
    };
    output::emit(&out, ctx.copy);
    Ok(())
}

pub fn backlinks(ctx: &Context, file: Option<&str>) -> anyhow::Result<()> {
    let vault = ctx.vault()?;
    let posts = vault.list_tracked_posts()?;

    let target = file.unwrap_or("");
    if target.is_empty() {
        anyhow::bail!("file argument is required");
    }

    // Build link index
    let mut link_index: HashMap<String, Vec<String>> = HashMap::new();
    for state in &posts {
        let abs_path = vault.config.vault_path.join(&state.path);
        if !abs_path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&abs_path)?;
        let outgoing = extract_links(&content);
        for link in outgoing {
            link_index.entry(link).or_default().push(state.path.clone());
        }
    }

    // Find backlinks to target
    let target_stem = std::path::Path::new(target)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut incoming = Vec::new();
    for (link, sources) in &link_index {
        if link.contains(&target_stem) || link.contains(target) {
            incoming.extend(sources.clone());
        }
    }
    incoming.sort();
    incoming.dedup();

    let out = match ctx.format {
        Format::Json => serde_json::to_string_pretty(&incoming)?,
        _ => {
            let mut s = format!("backlinks to {target}:\n");
            for src in &incoming {
                s.push_str(&format!("  <- {src}\n"));
            }
            if incoming.is_empty() {
                s.push_str("  (none)\n");
            }
            s
        }
    };
    output::emit(&out, ctx.copy);
    Ok(())
}

/// Extract markdown links from content: both `[text](url)` and `[[wikilink]]`.
fn extract_links(content: &str) -> Vec<String> {
    let mut links = Vec::new();

    // Standard markdown links [text](url)
    let chars = content.char_indices().peekable();
    for (i, c) in chars {
        if c == '[' && content[i..].starts_with('[') {
            // Check for wikilink [[...]]
            if content[i..].starts_with("[[") {
                if let Some(end) = content[i + 2..].find("]]") {
                    let link = &content[i + 2..i + 2 + end];
                    let display = link.split('|').next().unwrap_or(link);
                    links.push(display.to_string());
                }
            } else if let Some(bracket_end) = content[i + 1..].find("](") {
                let paren_start = i + 1 + bracket_end + 2;
                if let Some(paren_end) = content[paren_start..].find(')') {
                    let url = &content[paren_start..paren_start + paren_end];
                    if !url.starts_with("http") {
                        links.push(url.to_string());
                    }
                }
            }
        }
    }
    links
}
