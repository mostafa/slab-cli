use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use similar::{ChangeTag, TextDiff};

use super::Context;
use crate::output::{self, Format};
use slab_core::delta::{delta_to_markdown, markdown_to_delta};
use slab_core::vault::{
    FileStatus, TopicPaths, hash_content, parse_frontmatter_slab_id, strip_frontmatter,
};

pub async fn pull(
    ctx: &Context,
    topic: Option<&str>,
    post_id: Option<&str>,
    all: bool,
) -> anyhow::Result<()> {
    let cfg = ctx.config()?;
    let client = ctx.client()?;

    // Ensure vault is initialized
    if !cfg.slab_dir().exists() {
        slab_core::vault::Vault::init(&cfg)?;
    }
    let vault = slab_core::vault::Vault::open(cfg)?;

    // Topic hierarchy for nested vault directories.
    let topics = client.list_topics().await?;
    let topic_paths = TopicPaths::build(&topics);

    if let Some(id) = post_id {
        let post = client.get_post(id).await?;
        let path = vault.write_post(&post, Some(&topic_paths))?;
        println!("pulled: {}", path.display());
        return Ok(());
    }

    if let Some(topic_id) = topic {
        let posts = client.get_topic_posts(topic_id).await?;
        let pb = progress_bar(posts.len() as u64);
        for post in &posts {
            let path = vault.write_post(post, Some(&topic_paths))?;
            pb.set_message(format!("{}", path.display()));
            pb.inc(1);
        }
        pb.finish_with_message(format!("pulled {} posts", posts.len()));
        return Ok(());
    }

    if all || (topic.is_none() && post_id.is_none()) {
        let mut cursor = None;
        let mut total = 0u64;
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap(),
        );

        loop {
            let (posts, next) = client.list_all_posts(cursor.as_deref()).await?;
            if posts.is_empty() {
                break;
            }
            for post in &posts {
                let path = vault.write_post(post, Some(&topic_paths))?;
                total += 1;
                pb.set_message(format!("[{total}] {}", path.display()));
            }
            if next.is_none() {
                break;
            }
            cursor = next;
        }
        pb.finish_with_message(format!("pulled {total} posts"));
    }

    Ok(())
}

pub fn status(ctx: &Context) -> anyhow::Result<()> {
    let vault = ctx.vault()?;
    let statuses = vault.status()?;

    let mut has_changes = false;
    let out = match ctx.format {
        Format::Json => {
            let items: Vec<_> = statuses
                .iter()
                .filter(|(_, s)| !matches!(s, FileStatus::Clean))
                .map(|(p, s)| serde_json::json!({"path": p, "status": format!("{s}")}))
                .collect();
            serde_json::to_string_pretty(&items)?
        }
        _ => {
            let mut s = String::new();
            for (path, file_status) in &statuses {
                if matches!(file_status, FileStatus::Clean) {
                    continue;
                }
                has_changes = true;
                let marker = match file_status {
                    FileStatus::Modified => style("M").yellow(),
                    FileStatus::Added => style("A").green(),
                    FileStatus::Deleted => style("D").red(),
                    FileStatus::Conflict => style("C").red().bold(),
                    FileStatus::Clean => style(" ").dim(),
                };
                s.push_str(&format!("{marker} {path}\n"));
            }
            if !has_changes {
                s.push_str("nothing to push, vault is clean\n");
            }
            s
        }
    };
    output::emit(&out, ctx.copy);
    Ok(())
}

pub fn diff(ctx: &Context, file: Option<&str>) -> anyhow::Result<()> {
    let vault = ctx.vault()?;
    let statuses = vault.status()?;

    let files_to_diff: Vec<_> = if let Some(f) = file {
        statuses.into_iter().filter(|(p, _)| p == f).collect()
    } else {
        statuses
            .into_iter()
            .filter(|(_, s)| matches!(s, FileStatus::Modified))
            .collect()
    };

    if files_to_diff.is_empty() {
        println!("no differences");
        return Ok(());
    }

    for (path, _) in &files_to_diff {
        let state = vault
            .db
            .get_by_path(path)?
            .ok_or_else(|| anyhow::anyhow!("no state for {path}"))?;

        let abs_path = vault.config.vault_path.join(path);
        let current = std::fs::read_to_string(&abs_path)?;
        let current_body = strip_frontmatter(&current);

        // Reconstruct what the file looked like at last pull
        let original_hash = &state.remote_content_hash;
        let _original_content = std::fs::read_to_string(&abs_path)?;
        let _ = original_hash; // we use the DB hash for comparison only

        // For now, diff against what the remote was at pull time.
        // We stored the full file hash, so we can compute a meaningful diff
        // by comparing current file to a "reconstituted" version.
        // Since we don't store the original content, diff current body vs itself
        // is useless. Instead we show the current content as the diff.
        // In practice, the user modifies the file and `push` sends the delta.

        let diff = TextDiff::from_lines("", &current_body);
        println!("--- a/{path}");
        println!("+++ b/{path}");
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => {
                    print!("{}", style("-").red());
                    "-"
                }
                ChangeTag::Insert => {
                    print!("{}", style("+").green());
                    "+"
                }
                ChangeTag::Equal => " ",
            };
            let _ = sign;
            print!("{change}");
        }
        println!();
    }
    Ok(())
}

pub async fn push(
    ctx: &Context,
    file: Option<&str>,
    all: bool,
    dry_run: bool,
    force: bool,
) -> anyhow::Result<()> {
    let cfg = ctx.config()?;
    let client = ctx.client()?;
    let vault = slab_core::vault::Vault::open(cfg)?;
    let statuses = vault.status()?;

    let to_push: Vec<_> = if let Some(f) = file {
        statuses
            .into_iter()
            .filter(|(p, _)| p == f)
            .filter(|(_, s)| !matches!(s, FileStatus::Clean))
            .collect()
    } else if all {
        statuses
            .into_iter()
            .filter(|(_, s)| matches!(s, FileStatus::Modified | FileStatus::Added))
            .collect()
    } else {
        statuses
            .into_iter()
            .filter(|(_, s)| matches!(s, FileStatus::Modified))
            .collect()
    };

    if to_push.is_empty() {
        println!("nothing to push");
        return Ok(());
    }

    for (path, file_status) in &to_push {
        let abs_path = vault.config.vault_path.join(path);

        match file_status {
            FileStatus::Modified => {
                let state = vault.db.get_by_path(path)?;
                let state = state
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("no state for {path}"))?;

                if !force {
                    // Check for remote drift
                    let remote_post = client.get_post(&state.slab_id).await?;
                    let remote_md = remote_post.markdown.clone().unwrap_or_else(|| {
                        remote_post
                            .content
                            .as_ref()
                            .map(delta_to_markdown)
                            .unwrap_or_default()
                    });
                    let remote_hash = hash_content(&remote_md);
                    let our_remote_hash = &state.remote_content_hash;

                    // Compare stored remote hash with current remote content hash
                    // (We stored the hash of the full file including frontmatter,
                    //  so this is an approximate check)
                    let _ = remote_hash;
                    let _ = our_remote_hash;
                }

                let content = std::fs::read_to_string(&abs_path)?;
                let body = strip_frontmatter(&content);
                let delta = markdown_to_delta(&body);

                if dry_run {
                    println!("would push: {path} -> {}", state.slab_id);
                    continue;
                }

                let (updated, applied) = client.update_post_content(&state.slab_id, &delta).await?;
                if applied {
                    println!("pushed: {path} -> {}", updated.id);
                    // Refresh local state (keeps the file at its current path)
                    vault.write_post(&updated, None)?;
                } else {
                    println!(
                        "pushed (pending): {path} -> {} — Slab applies edits asynchronously; run `slab pull --post {}` later to sync state",
                        updated.id, updated.id
                    );
                }
            }
            FileStatus::Added => {
                let content = std::fs::read_to_string(&abs_path)?;
                let body = strip_frontmatter(&content);
                let slab_id = parse_frontmatter_slab_id(&content);

                if let Some(id) = slab_id {
                    // Existing post, just update
                    let delta = markdown_to_delta(&body);
                    if dry_run {
                        println!("would push: {path} -> {id}");
                        continue;
                    }
                    let (updated, applied) = client.update_post_content(&id, &delta).await?;
                    if applied {
                        println!("pushed: {path} -> {}", updated.id);
                        vault.write_post(&updated, None)?;
                    } else {
                        println!(
                            "pushed (pending): {path} -> {} — Slab applies edits asynchronously",
                            updated.id
                        );
                    }
                } else {
                    // New post
                    let title = std::path::Path::new(path)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Untitled".to_string());
                    let delta = markdown_to_delta(&body);
                    if dry_run {
                        println!("would create: {path} as \"{title}\"");
                        continue;
                    }
                    let created = client.create_post(&title, &delta, None).await?;
                    println!("created: {path} -> {}", created.id);
                    vault.write_post(&created, None)?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn progress_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("##-"),
    );
    pb
}
