use super::Context;
use crate::output::{self, Format};

pub async fn remote(ctx: &Context, query: &str, limit: usize) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let mut all_results = Vec::new();
    let mut cursor = None;

    loop {
        let (results, next) = client.search_posts(query, cursor.as_deref()).await?;
        all_results.extend(results);
        if all_results.len() >= limit || next.is_none() {
            break;
        }
        cursor = next;
    }

    all_results.truncate(limit);

    let out = match ctx.format {
        Format::Json => serde_json::to_string_pretty(&all_results)?,
        Format::Tsv => {
            let mut s = String::from("ID\tTITLE\tHIGHLIGHT\n");
            for r in &all_results {
                s.push_str(&format!(
                    "{}\t{}\t{}\n",
                    r.post.id,
                    r.post.title,
                    r.highlight.as_deref().unwrap_or("")
                ));
            }
            s
        }
        _ => {
            let mut s = format!("{} result(s) for \"{query}\"\n\n", all_results.len());
            for r in &all_results {
                s.push_str(&format!("  {} — {}\n", r.post.id, r.post.title));
                if let Some(h) = &r.highlight {
                    s.push_str(&format!("    {h}\n"));
                }
            }
            s
        }
    };
    output::emit(&out, ctx.copy);
    Ok(())
}

pub fn local(ctx: &Context, query: &str, limit: usize) -> anyhow::Result<()> {
    let vault = ctx.vault()?;
    let posts = vault.list_tracked_posts()?;

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for state in &posts {
        let abs_path = vault.config.vault_path.join(&state.path);
        if !abs_path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&abs_path)?;
        let mut file_matches: Vec<(usize, String)> = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                file_matches.push((line_num + 1, line.to_string()));
            }
        }

        if !file_matches.is_empty() {
            matches.push((state.path.clone(), file_matches));
        }

        if matches.len() >= limit {
            break;
        }
    }

    let out = match ctx.format {
        Format::Json => {
            let json_matches: Vec<_> = matches
                .iter()
                .map(|(path, lines)| {
                    serde_json::json!({
                        "path": path,
                        "matches": lines.iter().map(|(n, l)| {
                            serde_json::json!({"line": n, "text": l})
                        }).collect::<Vec<_>>()
                    })
                })
                .collect();
            serde_json::to_string_pretty(&json_matches)?
        }
        _ => {
            let mut s = String::new();
            for (path, lines) in &matches {
                for (line_num, text) in lines {
                    s.push_str(&format!("{path}:{line_num}: {text}\n"));
                }
            }
            if s.is_empty() {
                s.push_str(&format!("no matches for \"{query}\"\n"));
            }
            s
        }
    };
    output::emit(&out, ctx.copy);
    Ok(())
}
