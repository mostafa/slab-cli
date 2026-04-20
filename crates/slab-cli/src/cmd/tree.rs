use super::Context;

pub fn run(ctx: &Context) -> anyhow::Result<()> {
    let cfg = ctx.config()?;
    let vault_path = &cfg.vault_path;

    if !vault_path.exists() {
        anyhow::bail!("vault not found at {}", vault_path.display());
    }

    println!("{}/", vault_path.display());
    print_tree(vault_path, "", true)?;
    Ok(())
}

fn print_tree(dir: &std::path::Path, prefix: &str, is_root: bool) -> anyhow::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            !name_str.starts_with('.')
        })
        .collect();

    entries.sort_by_key(|e| {
        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
        (!is_dir, e.file_name())
    });

    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_root {
            ""
        } else if is_last {
            "└── "
        } else {
            "├── "
        };
        let name = entry.file_name();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if is_dir {
            println!("{prefix}{connector}{}/", name.to_string_lossy());
            let child_prefix = if is_root {
                String::new()
            } else if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}│   ")
            };
            print_tree(&entry.path(), &child_prefix, false)?;
        } else {
            println!("{prefix}{connector}{}", name.to_string_lossy());
        }
    }
    Ok(())
}
