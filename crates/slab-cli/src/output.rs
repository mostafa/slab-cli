use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Tsv,
    Md,
}

impl FromStr for Format {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Format::Text),
            "json" => Ok(Format::Json),
            "tsv" => Ok(Format::Tsv),
            "md" | "markdown" => Ok(Format::Md),
            _ => Err(format!("unknown format: {s}")),
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Format::Text => write!(f, "text"),
            Format::Json => write!(f, "json"),
            Format::Tsv => write!(f, "tsv"),
            Format::Md => write!(f, "md"),
        }
    }
}

/// Print output, optionally copying to clipboard.
pub fn emit(text: &str, copy: bool) {
    print!("{text}");
    if copy && let Err(e) = copy_to_clipboard(text) {
        eprintln!("warning: failed to copy to clipboard: {e}");
    }
}

fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;
    Ok(())
}
