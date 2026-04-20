use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{Value, json};

/// Convert Markdown text into a Quill Delta JSON value.
pub fn markdown_to_delta(md: &str) -> Value {
    // Check for passthrough embeds first
    let processed = preprocess_embeds(md);
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(&processed, options);

    let mut ops: Vec<Value> = Vec::new();
    let mut inline_attrs: Vec<InlineAttr> = Vec::new();
    let mut block_attr: Option<Value> = None;
    let mut text_buf = String::new();
    let mut _in_code_block = false;
    let mut code_lang = String::new();

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let n = heading_level_num(level);
                    block_attr = Some(json!({"header": n}));
                }
                Tag::Strong => inline_attrs.push(InlineAttr::Bold),
                Tag::Emphasis => inline_attrs.push(InlineAttr::Italic),
                Tag::Strikethrough => inline_attrs.push(InlineAttr::Strike),
                Tag::Link { dest_url, .. } => {
                    inline_attrs.push(InlineAttr::Link(dest_url.to_string()));
                }
                Tag::CodeBlock(kind) => {
                    _in_code_block = true;
                    code_lang = match kind {
                        CodeBlockKind::Fenced(lang) => lang.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                }
                Tag::List(_) => {}
                Tag::Item => {}
                Tag::BlockQuote(_) => {
                    block_attr = Some(json!({"blockquote": true}));
                }
                Tag::Paragraph => {}
                Tag::Image { dest_url, .. } => {
                    ops.push(json!({"insert": {"image": dest_url.to_string()}}));
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    flush_text(&mut ops, &mut text_buf, &inline_attrs);
                    if let Some(attr) = block_attr.take() {
                        ops.push(json!({"insert": "\n", "attributes": attr}));
                    } else {
                        ops.push(json!({"insert": "\n"}));
                    }
                }
                TagEnd::Strong => {
                    flush_text(&mut ops, &mut text_buf, &inline_attrs);
                    inline_attrs.retain(|a| !matches!(a, InlineAttr::Bold));
                }
                TagEnd::Emphasis => {
                    flush_text(&mut ops, &mut text_buf, &inline_attrs);
                    inline_attrs.retain(|a| !matches!(a, InlineAttr::Italic));
                }
                TagEnd::Strikethrough => {
                    flush_text(&mut ops, &mut text_buf, &inline_attrs);
                    inline_attrs.retain(|a| !matches!(a, InlineAttr::Strike));
                }
                TagEnd::Link => {
                    flush_text(&mut ops, &mut text_buf, &inline_attrs);
                    inline_attrs.retain(|a| !matches!(a, InlineAttr::Link(_)));
                }
                TagEnd::CodeBlock => {
                    // Each line inside code block becomes its own insert + \n with code-block attr
                    let lang = if code_lang.is_empty() {
                        "plain".to_string()
                    } else {
                        code_lang.clone()
                    };
                    let code_text = std::mem::take(&mut text_buf);
                    for line in code_text.split('\n') {
                        if !line.is_empty() || !code_text.ends_with('\n') {
                            ops.push(json!({"insert": line}));
                        }
                        ops.push(json!({"insert": "\n", "attributes": {"code-block": lang}}));
                    }
                    // Remove trailing extra newline op if the code had a trailing \n
                    if code_text.ends_with('\n') {
                        ops.pop();
                    }
                    _in_code_block = false;
                    code_lang.clear();
                }
                TagEnd::Item => {
                    flush_text(&mut ops, &mut text_buf, &inline_attrs);
                    if let Some(attr) = block_attr.take() {
                        ops.push(json!({"insert": "\n", "attributes": attr}));
                    } else {
                        ops.push(json!({"insert": "\n", "attributes": {"list": "bullet"}}));
                    }
                }
                TagEnd::BlockQuote(_) => {
                    block_attr = None;
                }
                TagEnd::Paragraph => {
                    flush_text(&mut ops, &mut text_buf, &inline_attrs);
                    if block_attr.is_none() {
                        ops.push(json!({"insert": "\n"}));
                    }
                }
                TagEnd::List(_) => {}
                _ => {}
            },
            Event::Text(text) => {
                text_buf.push_str(&text);
            }
            Event::Code(code) => {
                flush_text(&mut ops, &mut text_buf, &inline_attrs);
                ops.push(json!({"insert": code.to_string(), "attributes": {"code": true}}));
            }
            Event::SoftBreak => {
                text_buf.push(' ');
            }
            Event::HardBreak => {
                flush_text(&mut ops, &mut text_buf, &inline_attrs);
                ops.push(json!({"insert": "\n"}));
            }
            Event::Rule => {
                ops.push(json!({"insert": {"divider": true}}));
                ops.push(json!({"insert": "\n"}));
            }
            Event::Html(html) => {
                let html_str = html.trim();
                if let Some(embed_json) = parse_slab_embed_comment(html_str) {
                    ops.push(json!({"insert": embed_json}));
                } else if html_str == "<u>" || html_str == "</u>" {
                    // handled via inline attrs
                } else {
                    text_buf.push_str(&html);
                }
            }
            _ => {}
        }
    }

    flush_text(&mut ops, &mut text_buf, &inline_attrs);

    json!({"ops": ops})
}

#[derive(Debug, Clone)]
enum InlineAttr {
    Bold,
    Italic,
    Strike,
    Link(String),
}

fn flush_text(ops: &mut Vec<Value>, buf: &mut String, attrs: &[InlineAttr]) {
    if buf.is_empty() {
        return;
    }
    let text = std::mem::take(buf);
    if attrs.is_empty() {
        ops.push(json!({"insert": text}));
    } else {
        let mut attr_obj = serde_json::Map::new();
        for attr in attrs {
            match attr {
                InlineAttr::Bold => {
                    attr_obj.insert("bold".into(), json!(true));
                }
                InlineAttr::Italic => {
                    attr_obj.insert("italic".into(), json!(true));
                }
                InlineAttr::Strike => {
                    attr_obj.insert("strike".into(), json!(true));
                }
                InlineAttr::Link(url) => {
                    attr_obj.insert("link".into(), json!(url));
                }
            }
        }
        ops.push(json!({"insert": text, "attributes": attr_obj}));
    }
}

fn heading_level_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn preprocess_embeds(md: &str) -> String {
    md.to_string()
}

fn parse_slab_embed_comment(html: &str) -> Option<Value> {
    let trimmed = html.trim();
    if let Some(rest) = trimmed.strip_prefix("<!-- slab:embed ")
        && let Some(json_str) = rest.strip_suffix(" -->")
        && let Ok(val) = serde_json::from_str::<Value>(json_str)
    {
        return Some(val);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_paragraph() {
        let delta = markdown_to_delta("Hello, world!\n");
        let ops = delta["ops"].as_array().unwrap();
        assert!(ops.iter().any(|op| op["insert"] == "Hello, world!"));
    }

    #[test]
    fn heading_roundtrip() {
        let delta = markdown_to_delta("# Title\n");
        let ops = delta["ops"].as_array().unwrap();
        assert!(ops.iter().any(|op| op["attributes"]["header"] == 1));
    }

    #[test]
    fn bold_text() {
        let delta = markdown_to_delta("**bold**\n");
        let ops = delta["ops"].as_array().unwrap();
        assert!(
            ops.iter()
                .any(|op| op["insert"] == "bold" && op["attributes"]["bold"] == true)
        );
    }

    #[test]
    fn link() {
        let delta = markdown_to_delta("[click](https://example.com)\n");
        let ops = delta["ops"].as_array().unwrap();
        assert!(
            ops.iter()
                .any(|op| op["insert"] == "click"
                    && op["attributes"]["link"] == "https://example.com")
        );
    }

    #[test]
    fn code_block() {
        let delta = markdown_to_delta("```rust\nfn main() {}\n```\n");
        let ops = delta["ops"].as_array().unwrap();
        assert!(
            ops.iter()
                .any(|op| op["attributes"]["code-block"] == "rust")
        );
    }

    #[test]
    fn embed_passthrough() {
        let md = "<!-- slab:embed {\"video\":\"https://example.com/v.mp4\"} -->\n";
        let delta = markdown_to_delta(md);
        let ops = delta["ops"].as_array().unwrap();
        assert!(
            ops.iter()
                .any(|op| op["insert"]["video"] == "https://example.com/v.mp4")
        );
    }
}
