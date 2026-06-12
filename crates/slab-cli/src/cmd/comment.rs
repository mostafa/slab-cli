use super::Context;
use crate::output::{self, Format};
use slab_core::delta::delta_to_markdown;

pub async fn list(ctx: &Context, post_id: &str) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let post_threads = client.get_post_threads(post_id).await?;

    let threads = post_threads.threads.unwrap_or_default();
    let comment_threads: Vec<_> = threads.iter().filter(|t| t.comments.is_some()).collect();

    let out = match ctx.format {
        Format::Json => serde_json::to_string_pretty(&comment_threads)?,
        Format::Tsv => {
            let mut s = String::from("THREAD\tCOMMENT\tAUTHOR\tDATE\tCONTENT\n");
            for thread in &comment_threads {
                for comment in thread.comments.as_deref().unwrap_or_default() {
                    let author = comment
                        .author
                        .as_ref()
                        .and_then(|a| a.name.as_deref())
                        .unwrap_or("unknown");
                    let date = comment.inserted_at.as_deref().unwrap_or("-");
                    let content = format_comment_content(comment.content.as_deref());
                    s.push_str(&format!(
                        "{}\t{}\t{}\t{}\t{}\n",
                        thread.id, comment.id, author, date, content
                    ));
                }
            }
            s
        }
        _ => {
            let mut s = String::new();
            if comment_threads.is_empty() {
                s.push_str("no comments on this post\n");
                output::emit(&s, ctx.copy);
                return Ok(());
            }
            for thread in &comment_threads {
                let resolved = thread.resolved_at.is_some();
                let status = if resolved { " [resolved]" } else { "" };
                s.push_str(&format!("thread {}{}\n", thread.id, status));
                for comment in thread.comments.as_deref().unwrap_or_default() {
                    let author = comment
                        .author
                        .as_ref()
                        .and_then(|a| a.name.as_deref())
                        .unwrap_or("unknown");
                    let date = comment.inserted_at.as_deref().unwrap_or("");
                    let content = format_comment_content(comment.content.as_deref());
                    s.push_str(&format!("  {} ({}, {})\n", content, author, date));
                }
                s.push('\n');
            }
            s
        }
    };
    output::emit(&out, ctx.copy);
    Ok(())
}

pub async fn add(
    ctx: &Context,
    post_id: &str,
    body: &str,
    thread_id: Option<&str>,
) -> anyhow::Result<()> {
    let client = ctx.client()?;

    let delta_content = format!(
        "[{{\"insert\":\"{}\\n\"}}]",
        body.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    );

    // Replying to an existing thread uses a simpler mutation.
    if let Some(tid) = thread_id {
        let comment_id = client.add_comment(tid, &delta_content).await?;
        println!("comment created: {comment_id} (thread: {tid})");
        return Ok(());
    }

    // New threads need the post's content version and OT checksum.
    let post = client.get_post(post_id).await?;
    let version = post.version.unwrap_or(1);
    let checksum = post.checksum();

    // Default mark: anchor the thread at the beginning of the document
    let mark = serde_json::json!({
        "type": "plain",
        "index": 0,
        "length": 1
    });

    let (tid, comment_id) = client
        .create_comment(
            post_id,
            &short_id(),
            &delta_content,
            version,
            checksum,
            &mark,
        )
        .await?;

    println!("comment created: {comment_id} (thread: {tid})");
    Ok(())
}

pub async fn update(ctx: &Context, comment_id: &str, body: &str) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let delta_content = format!(
        "[{{\"insert\":\"{}\\n\"}}]",
        body.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    );
    let result = client.update_comment(comment_id, &delta_content).await?;
    println!("comment {} updated", result.id);
    Ok(())
}

pub async fn delete(ctx: &Context, comment_id: &str) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let id = client.delete_comment(comment_id).await?;
    println!("comment {} deleted", id);
    Ok(())
}

pub async fn react(ctx: &Context, comment_id: &str, emoji: &str) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let id = client.react_to_comment(comment_id, emoji).await?;
    println!("reaction {} added to comment {}", id, comment_id);
    Ok(())
}

pub async fn delete_thread(ctx: &Context, thread_id: &str) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let id = client.delete_thread(thread_id).await?;
    println!("thread {} deleted", id);
    Ok(())
}

pub async fn resolve(ctx: &Context, thread_id: &str) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let result = client.resolve_thread(thread_id).await?;
    println!(
        "thread {} resolved at {}",
        result.id,
        result.resolved_at.as_deref().unwrap_or("now")
    );
    Ok(())
}

pub async fn unresolve(ctx: &Context, thread_id: &str) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let id = client.unresolve_thread(thread_id).await?;
    println!("thread {} unresolved", id);
    Ok(())
}

/// Slab thread IDs are short lowercase base36 strings generated client-side.
fn short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    (0..8)
        .map(|_| {
            let c = CHARS[(n % 36) as usize] as char;
            n /= 36;
            c
        })
        .collect()
}

/// Parse comment content from Quill Delta JSON string to plain text.
fn format_comment_content(content: Option<&str>) -> String {
    let Some(raw) = content else {
        return String::new();
    };

    // Comment content is a JSON string containing Delta ops
    if let Ok(delta) = serde_json::from_str::<serde_json::Value>(raw) {
        let wrapped = serde_json::json!({ "ops": delta });
        let md = delta_to_markdown(&wrapped);
        return md.trim().to_string();
    }

    raw.to_string()
}
