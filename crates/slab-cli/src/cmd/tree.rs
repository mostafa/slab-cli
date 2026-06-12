use super::Context;

pub struct TreeOptions {
    /// Subdirectory of the vault to show (relative path).
    pub path: Option<String>,
    /// Show only directories.
    pub dirs: bool,
    /// Limit recursion depth (1 = direct children only).
    pub depth: Option<usize>,
}

pub fn run(ctx: &Context, opts: &TreeOptions) -> anyhow::Result<()> {
    let cfg = ctx.config()?;
    let root = match &opts.path {
        Some(p) => cfg.vault_path.join(p),
        None => cfg.vault_path.clone(),
    };

    if !root.exists() {
        anyhow::bail!("path not found: {}", root.display());
    }

    println!("{}/", root.display());
    print_tree(&root, "", true, opts, 0)?;
    Ok(())
}

fn print_tree(
    dir: &std::path::Path,
    prefix: &str,
    is_root: bool,
    opts: &TreeOptions,
    level: usize,
) -> anyhow::Result<()> {
    if let Some(max) = opts.depth
        && level >= max
    {
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                return false;
            }
            !opts.dirs || e.file_type().map(|t| t.is_dir()).unwrap_or(false)
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
            print_tree(&entry.path(), &child_prefix, false, opts, level + 1)?;
        } else {
            println!("{prefix}{connector}{}", name.to_string_lossy());
        }
    }
    Ok(())
}
