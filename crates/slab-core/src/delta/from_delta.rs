use serde_json::Value;

/// Convert a Quill Delta JSON (array of ops) into Markdown.
pub fn delta_to_markdown(delta: &Value) -> String {
    let ops = match delta.get("ops").and_then(|v| v.as_array()) {
        Some(ops) => ops,
        None => match delta.as_array() {
            Some(ops) => ops,
            None => return String::new(),
        },
    };

    let mut output = String::new();
    let mut line_buffer = String::new();
    let mut in_code_block = false;
    let mut code_block_lang = String::new();

    for op in ops {
        if let Some(insert) = op.get("insert") {
            let attrs = op.get("attributes");

            match insert {
                Value::String(text) => {
                    let lines: Vec<&str> = text.split('\n').collect();
                    for (i, segment) in lines.iter().enumerate() {
                        if !segment.is_empty() {
                            let formatted = apply_inline_attrs(segment, attrs);
                            line_buffer.push_str(&formatted);
                        }

                        if i < lines.len() - 1 {
                            flush_line(
                                &mut output,
                                &mut line_buffer,
                                attrs,
                                &mut in_code_block,
                                &mut code_block_lang,
                            );
                        }
                    }
                }
                Value::Object(embed) => {
                    handle_embed(&mut line_buffer, embed);
                }
                _ => {}
            }
        }
    }

    if !line_buffer.is_empty() {
        output.push_str(&line_buffer);
        output.push('\n');
    }

    if in_code_block {
        output.push_str("```\n");
    }

    output
}

fn apply_inline_attrs(text: &str, attrs: Option<&Value>) -> String {
    let attrs = match attrs {
        Some(Value::Object(m)) => m,
        _ => return text.to_string(),
    };

    let mut result = text.to_string();

    if attrs.get("code").and_then(|v| v.as_bool()) == Some(true) {
        result = format!("`{result}`");
        return result;
    }

    if let Some(Value::String(url)) = attrs.get("link") {
        result = format!("[{result}]({url})");
    }

    if attrs.get("bold").and_then(|v| v.as_bool()) == Some(true) {
        result = format!("**{result}**");
    }

    if attrs.get("italic").and_then(|v| v.as_bool()) == Some(true) {
        result = format!("*{result}*");
    }

    if attrs.get("strike").and_then(|v| v.as_bool()) == Some(true) {
        result = format!("~~{result}~~");
    }

    if attrs.get("underline").and_then(|v| v.as_bool()) == Some(true) {
        result = format!("<u>{result}</u>");
    }

    result
}

fn flush_line(
    output: &mut String,
    line_buffer: &mut String,
    attrs: Option<&Value>,
    in_code_block: &mut bool,
    code_block_lang: &mut String,
) {
    let attrs = attrs.and_then(|v| v.as_object());

    // Check for code-block
    if let Some(attrs) = attrs
        && (attrs.contains_key("code-block") || attrs.contains_key("code_block"))
    {
        let lang = attrs
            .get("code-block")
            .or_else(|| attrs.get("code_block"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if !*in_code_block {
            *in_code_block = true;
            *code_block_lang = lang.clone();
            if lang.is_empty() || lang == "true" || lang == "plain" {
                output.push_str("```\n");
            } else {
                output.push_str(&format!("```{lang}\n"));
            }
        }

        output.push_str(line_buffer);
        output.push('\n');
        line_buffer.clear();
        return;
    }

    if *in_code_block {
        output.push_str("```\n\n");
        *in_code_block = false;
        code_block_lang.clear();
    }

    if let Some(attrs) = attrs {
        if let Some(header) = attrs.get("header").and_then(|v| v.as_u64()) {
            let prefix = "#".repeat(header as usize);
            output.push_str(&format!("{prefix} {}\n", line_buffer.trim()));
            line_buffer.clear();
            return;
        }

        if let Some(list) = attrs.get("list").and_then(|v| v.as_str()) {
            let indent_level = attrs.get("indent").and_then(|v| v.as_u64()).unwrap_or(0);
            let indent = "  ".repeat(indent_level as usize);

            match list {
                "ordered" => {
                    output.push_str(&format!("{indent}1. {}\n", line_buffer.trim()));
                }
                "bullet" => {
                    output.push_str(&format!("{indent}- {}\n", line_buffer.trim()));
                }
                "checked" => {
                    output.push_str(&format!("{indent}- [x] {}\n", line_buffer.trim()));
                }
                "unchecked" => {
                    output.push_str(&format!("{indent}- [ ] {}\n", line_buffer.trim()));
                }
                _ => {
                    output.push_str(&format!("{indent}- {}\n", line_buffer.trim()));
                }
            }
            line_buffer.clear();
            return;
        }

        if attrs.get("blockquote").and_then(|v| v.as_bool()) == Some(true) {
            output.push_str(&format!("> {}\n", line_buffer.trim()));
            line_buffer.clear();
            return;
        }
    }

    // Plain paragraph
    if line_buffer.is_empty() {
        output.push('\n');
    } else {
        output.push_str(line_buffer);
        output.push('\n');
    }
    line_buffer.clear();
}

fn handle_embed(line_buffer: &mut String, embed: &serde_json::Map<String, Value>) {
    if let Some(Value::String(url)) = embed.get("image") {
        line_buffer.push_str(&format!("![image]({url})"));
        return;
    }

    if embed.contains_key("divider") || embed.contains_key("hr") {
        line_buffer.push_str("---");
        return;
    }

    if let Some(Value::Object(mention)) = embed.get("mention")
        && let Some(Value::String(name)) = mention.get("name")
    {
        line_buffer.push_str(&format!("@{name}"));
        return;
    }

    // Unknown embed: preserve as HTML comment for lossless round-trip
    let json = serde_json::to_string(embed).unwrap_or_default();
    line_buffer.push_str(&format!("<!-- slab:embed {json} -->"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_text() {
        let delta = json!({
            "ops": [
                {"insert": "Hello, world!\n"}
            ]
        });
        let md = delta_to_markdown(&delta);
        assert_eq!(md, "Hello, world!\n");
    }

    #[test]
    fn heading() {
        let delta = json!({
            "ops": [
                {"insert": "Title"},
                {"insert": "\n", "attributes": {"header": 1}}
            ]
        });
        let md = delta_to_markdown(&delta);
        assert_eq!(md, "# Title\n");
    }

    #[test]
    fn bold_italic() {
        let delta = json!({
            "ops": [
                {"insert": "bold", "attributes": {"bold": true}},
                {"insert": " and "},
                {"insert": "italic", "attributes": {"italic": true}},
                {"insert": "\n"}
            ]
        });
        let md = delta_to_markdown(&delta);
        assert_eq!(md, "**bold** and *italic*\n");
    }

    #[test]
    fn code_block() {
        let delta = json!({
            "ops": [
                {"insert": "fn main() {}"},
                {"insert": "\n", "attributes": {"code-block": "rust"}}
            ]
        });
        let md = delta_to_markdown(&delta);
        assert_eq!(md, "```rust\nfn main() {}\n```\n");
    }

    #[test]
    fn bullet_list() {
        let delta = json!({
            "ops": [
                {"insert": "item one"},
                {"insert": "\n", "attributes": {"list": "bullet"}},
                {"insert": "item two"},
                {"insert": "\n", "attributes": {"list": "bullet"}}
            ]
        });
        let md = delta_to_markdown(&delta);
        assert_eq!(md, "- item one\n- item two\n");
    }

    #[test]
    fn link() {
        let delta = json!({
            "ops": [
                {"insert": "click here", "attributes": {"link": "https://example.com"}},
                {"insert": "\n"}
            ]
        });
        let md = delta_to_markdown(&delta);
        assert_eq!(md, "[click here](https://example.com)\n");
    }

    #[test]
    fn image_embed() {
        let delta = json!({
            "ops": [
                {"insert": {"image": "https://example.com/img.png"}},
                {"insert": "\n"}
            ]
        });
        let md = delta_to_markdown(&delta);
        assert_eq!(md, "![image](https://example.com/img.png)\n");
    }

    #[test]
    fn blockquote() {
        let delta = json!({
            "ops": [
                {"insert": "quoted text"},
                {"insert": "\n", "attributes": {"blockquote": true}}
            ]
        });
        let md = delta_to_markdown(&delta);
        assert_eq!(md, "> quoted text\n");
    }

    #[test]
    fn unknown_embed_passthrough() {
        let delta = json!({
            "ops": [
                {"insert": {"video": "https://example.com/v.mp4"}},
                {"insert": "\n"}
            ]
        });
        let md = delta_to_markdown(&delta);
        assert!(md.contains("<!-- slab:embed"));
        assert!(md.contains("video"));
    }
}
