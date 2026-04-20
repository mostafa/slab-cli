use super::Context;
use crate::output::{self, Format};

pub async fn list(ctx: &Context) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let topics = client.list_topics().await?;

    let out = match ctx.format {
        Format::Json => serde_json::to_string_pretty(&topics)?,
        Format::Tsv => {
            let mut s = String::from("ID\tNAME\tPARENT\n");
            for t in &topics {
                s.push_str(&format!(
                    "{}\t{}\t{}\n",
                    t.id,
                    t.name,
                    t.parent_topic_id.as_deref().unwrap_or("-")
                ));
            }
            s
        }
        _ => {
            let mut s = String::new();
            for t in &topics {
                let desc = t.description.as_deref().unwrap_or("");
                if desc.is_empty() {
                    s.push_str(&format!("  {} — {}\n", t.id, t.name));
                } else {
                    s.push_str(&format!("  {} — {} ({})\n", t.id, t.name, desc));
                }
            }
            if s.is_empty() {
                s.push_str("(no topics)\n");
            }
            s
        }
    };
    output::emit(&out, ctx.copy);
    Ok(())
}

pub async fn get(ctx: &Context, id: &str) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let topics = client.list_topics().await?;
    let topic = topics
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| anyhow::anyhow!("topic not found: {id}"))?;

    let out = match ctx.format {
        Format::Json => serde_json::to_string_pretty(topic)?,
        _ => format!(
            "ID:          {}\nName:        {}\nDescription: {}\nParent:      {}\n",
            topic.id,
            topic.name,
            topic.description.as_deref().unwrap_or("-"),
            topic.parent_topic_id.as_deref().unwrap_or("-"),
        ),
    };
    output::emit(&out, ctx.copy);
    Ok(())
}

pub async fn posts(ctx: &Context, id: &str) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let posts = client.get_topic_posts(id).await?;

    let out = match ctx.format {
        Format::Json => serde_json::to_string_pretty(&posts)?,
        Format::Tsv => {
            let mut s = String::from("ID\tTITLE\tUPDATED\n");
            for p in &posts {
                s.push_str(&format!(
                    "{}\t{}\t{}\n",
                    p.id,
                    p.title,
                    p.updated_at.as_deref().unwrap_or("-")
                ));
            }
            s
        }
        _ => {
            let mut s = String::new();
            for p in &posts {
                s.push_str(&format!("  {} — {}\n", p.id, p.title));
            }
            if s.is_empty() {
                s.push_str("(no posts in topic)\n");
            }
            s
        }
    };
    output::emit(&out, ctx.copy);
    Ok(())
}
