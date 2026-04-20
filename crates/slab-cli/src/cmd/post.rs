use std::io::Read;

use super::Context;
use crate::output::{self, Format};

pub async fn get(ctx: &Context, id: &str, raw: bool) -> anyhow::Result<()> {
    let client = ctx.client()?;

    let clean_id = extract_post_id(id);
    let post = client.get_post(&clean_id).await?;

    if raw {
        let json = serde_json::to_string_pretty(&post.content)?;
        output::emit(&json, ctx.copy);
        return Ok(());
    }

    let out = match ctx.format {
        Format::Json => serde_json::to_string_pretty(&post)?,
        _ => {
            let md = match &post.content {
                Some(content) => slab_core::delta::delta_to_markdown(content),
                None => String::new(),
            };
            format!(
                "# {}\n\nID: {} | Updated: {}\n\n{}",
                post.title,
                post.id,
                post.updated_at.as_deref().unwrap_or("unknown"),
                md
            )
        }
    };
    output::emit(&out, ctx.copy);
    Ok(())
}

pub async fn list(ctx: &Context, topic: Option<&str>) -> anyhow::Result<()> {
    let client = ctx.client()?;

    let posts = if let Some(topic_id) = topic {
        client.get_topic_posts(topic_id).await?
    } else {
        let (posts, _) = client.list_all_posts(None).await?;
        posts
    };

    let out = format_post_list(&posts, ctx.format);
    output::emit(&out, ctx.copy);
    Ok(())
}

pub async fn create(
    ctx: &Context,
    title: &str,
    topic: Option<&str>,
    content_file: Option<&str>,
) -> anyhow::Result<()> {
    let client = ctx.client()?;

    let md_content = read_content_source(content_file)?;
    let delta = slab_core::delta::markdown_to_delta(&md_content);

    let post = client.create_post(title, &delta, topic).await?;
    println!("created post: {} (id: {})", post.title, post.id);
    Ok(())
}

pub async fn update(
    ctx: &Context,
    id: &str,
    content_file: Option<&str>,
    _title: Option<&str>,
) -> anyhow::Result<()> {
    let client = ctx.client()?;

    let clean_id = extract_post_id(id);

    let md_content = read_content_source(content_file)?;
    let delta = slab_core::delta::markdown_to_delta(&md_content);

    let post = client.update_post_content(&clean_id, &delta).await?;
    println!("updated post: {} (id: {})", post.title, post.id);
    Ok(())
}

fn format_post_list(posts: &[slab_core::api::types::Post], format: Format) -> String {
    match format {
        Format::Json => serde_json::to_string_pretty(posts).unwrap_or_default(),
        Format::Tsv => {
            let mut out = String::from("ID\tTITLE\tUPDATED\n");
            for p in posts {
                out.push_str(&format!(
                    "{}\t{}\t{}\n",
                    p.id,
                    p.title,
                    p.updated_at.as_deref().unwrap_or("-")
                ));
            }
            out
        }
        _ => {
            let mut out = String::new();
            for p in posts {
                let topics: String = p
                    .topics
                    .as_ref()
                    .map(|ts| {
                        ts.iter()
                            .map(|t| t.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                out.push_str(&format!("  {} — {} [{}]\n", p.id, p.title, topics));
            }
            if out.is_empty() {
                out.push_str("(no posts)\n");
            }
            out
        }
    }
}

fn read_content_source(source: Option<&str>) -> anyhow::Result<String> {
    match source {
        Some("-") => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        Some(path) => Ok(std::fs::read_to_string(path)?),
        None => Ok(String::new()),
    }
}

fn extract_post_id(input: &str) -> String {
    // Handle full URLs like https://team.slab.com/posts/slug-POSTID
    if input.contains("slab.com/posts/")
        && let Some(slug) = input.rsplit('/').next()
    {
        if let Some(id) = slug.rsplit('-').next()
            && !id.is_empty()
        {
            return id.to_string();
        }
        return slug.to_string();
    }
    input.to_string()
}
