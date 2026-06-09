use comrak::{Arena, Options, parse_document};
use comrak::nodes::{AstNode, NodeValue, ListType, TableAlignment};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;
use crate::ast::*;

type FootnoteMap = HashMap<String, Vec<Inline>>;

pub fn parse_markdown(input: &str) -> Document {
    let (frontmatter, body_raw) = split_frontmatter(input);
    let body_no_comments = strip_obsidian_comments(body_raw);
    // Convert Obsidian wikilink images to standard Markdown before comrak sees them:
    //   ![[path|width]]  →  ![|width](path)
    //   ![[path]]        →  ![](path)
    let body_owned = preprocess_obsidian_images(&body_no_comments);
    let body = body_owned.as_str();

    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.math_dollars = true;
    options.extension.footnotes = true;

    let root = parse_document(&arena, body, &options);

    // Pre-collect footnote definitions so references can be resolved inline.
    let footnotes = collect_footnotes(root);

    let blocks = parse_blocks(root.children(), &footnotes);

    Document { frontmatter, blocks }
}

/// Pre-pass: collect all FootnoteDefinition blocks into a name → inlines map.
fn collect_footnotes<'a>(root: &'a AstNode<'a>) -> FootnoteMap {
    let empty: FootnoteMap = HashMap::new();
    let mut map = HashMap::new();
    for node in root.children() {
        let val = node.data.borrow();
        if let NodeValue::FootnoteDefinition(fd) = &val.value {
            let name = fd.name.clone();
            drop(val);
            // Collect inlines from the first paragraph of the definition.
            let inlines = node.children()
                .find_map(|child| {
                    let v = child.data.borrow();
                    if matches!(v.value, NodeValue::Paragraph) {
                        drop(v);
                        Some(parse_inlines(child.children(), &empty))
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            map.insert(name, inlines);
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Frontmatter
// ---------------------------------------------------------------------------

/// Splits a `---` YAML frontmatter block from the rest of the document.
/// Returns (parsed key/value pairs, remaining markdown).
fn split_frontmatter(input: &str) -> (Vec<(String, FrontmatterValue)>, &str) {
    let input = input.trim_start();
    if !input.starts_with("---") {
        return (vec![], input);
    }
    let after_open = &input[3..];
    if let Some(close) = find_frontmatter_close(after_open) {
        let yaml = &after_open[..close].trim_start_matches('\n');
        let rest = &after_open[close + 4..]; // skip \n---
        let rest = rest.trim_start_matches(['\n', '\r']);
        return (parse_yaml_frontmatter(yaml), rest);
    }
    (vec![], input)
}

/// Finds the position of a closing `---` frontmatter delimiter.
/// The `---` must appear after a newline and be followed by a newline, `\r`, or end-of-string.
fn find_frontmatter_close(s: &str) -> Option<usize> {
    let mut search_from = 0;
    while let Some(rel) = s[search_from..].find("\n---") {
        let abs = search_from + rel;
        let after = abs + 4;
        if after >= s.len() || s.as_bytes()[after] == b'\n' || s.as_bytes()[after] == b'\r' {
            return Some(abs);
        }
        search_from = abs + 1;
    }
    None
}

/// Parses simple YAML key/value pairs into (key, FrontmatterValue) pairs.
/// Handles strings, numbers, booleans, inline arrays, and block arrays.
fn parse_yaml_frontmatter(yaml: &str) -> Vec<(String, FrontmatterValue)> {
    let mut result = Vec::new();
    let mut lines = yaml.lines().peekable();

    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(colon) = line.find(':') else { continue };

        let key = sanitize_key(&line[..colon]);
        let rest = line[colon + 1..].trim();

        if rest.is_empty() {
            // Block sequence: following lines starting with `- `
            let mut items: Vec<String> = Vec::new();
            while let Some(next) = lines.peek() {
                let next = next.trim();
                if next.starts_with("- ") || next.starts_with('-') {
                    let item = next.trim_start_matches('-').trim().to_string();
                    items.push(typst_string(&item));
                    lines.next();
                } else {
                    break;
                }
            }
            if !items.is_empty() {
                let raw = if items.len() == 1 {
                    format!("({},)", items[0])
                } else {
                    format!("({})", items.join(", "))
                };
                result.push((key, FrontmatterValue::Raw(raw)));
            }
        } else if matches!(rest, "|" | "|-" | "|+") {
            // YAML literal block scalar — collect indented body lines.
            // Indent depth is determined from the first non-empty body line.
            let mut indent: Option<usize> = None;
            let mut body: Vec<String> = Vec::new();

            loop {
                match lines.peek() {
                    None => break,
                    Some(raw) => {
                        let raw: &str = raw;
                        if raw.trim().is_empty() {
                            body.push(String::new());
                            lines.next();
                        } else {
                            let line_indent = raw.len() - raw.trim_start().len();
                            // If indent not yet set and this line is at column 0, it is
                            // the next YAML key — not part of the block body. Stop.
                            if indent.is_none() && line_indent == 0 {
                                break;
                            }
                            let ind = *indent.get_or_insert(line_indent);
                            if line_indent >= ind {
                                body.push(raw[ind..].trim_end().to_string());
                                lines.next();
                            } else {
                                break;
                            }
                        }
                    }
                }
            }

            // YAML clip chomping: strip trailing empty lines.
            while body.last().is_some_and(|s| s.is_empty()) {
                body.pop();
            }

            if !body.is_empty() {
                // Parse each line independently so rich inline formatting
                // (bold, italic) works per-line. Lines are stored separately;
                // the renderer joins them with Typst \ line breaks.
                let lines_parsed: Vec<Vec<Inline>> = body
                    .iter()
                    .map(|line| parse_inline_markdown(line))
                    .collect();
                result.push((key, FrontmatterValue::Multiline(lines_parsed)));
            }
        } else {
            result.push((key, yaml_scalar_to_fm_value(rest)));
        }
    }
    result
}

fn sanitize_key(k: &str) -> String {
    const TYPST_KEYWORDS: &[&str] = &[
        "let", "if", "else", "for", "while", "in", "not", "and", "or",
        "none", "true", "false", "import", "include", "show", "set",
    ];
    let mut result: String = k.trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if result.starts_with(|c: char| c.is_ascii_digit()) {
        result.insert(0, '_');
    }
    if TYPST_KEYWORDS.contains(&result.as_str()) {
        result.insert(0, '_');
    }
    result
}

fn yaml_scalar_to_fm_value(s: &str) -> FrontmatterValue {
    // Inline array  [item1, item2, ...] → Raw Typst tuple of strings
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len() - 1];
        let items: Vec<String> = inner
            .split(',')
            .map(|i| typst_string(i.trim().trim_matches(|c: char| c == '"' || c == '\'')))
            .collect();
        let raw = if items.len() == 1 {
            format!("({},)", items[0])
        } else {
            format!("({})", items.join(", "))
        };
        return FrontmatterValue::Raw(raw);
    }
    // Quoted string — strip the quotes before inline-parsing
    let text = if (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('\'') && s.ends_with('\''))
    {
        &s[1..s.len() - 1]
    } else {
        s
    };
    FrontmatterValue::Inlines(parse_inline_markdown(text))
}

/// Parse a short string as inline Markdown using a fresh comrak arena.
/// Used for frontmatter values so they can carry bold, italic, code, etc.
fn parse_inline_markdown(s: &str) -> Vec<Inline> {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.math_dollars = true;
    let root = parse_document(&arena, s, &options);
    let empty: FootnoteMap = HashMap::new();
    root.children()
        .find_map(|node| {
            let val = node.data.borrow();
            if matches!(val.value, NodeValue::Paragraph) {
                drop(val);
                Some(parse_inlines(node.children(), &empty))
            } else {
                None
            }
        })
        .unwrap_or_else(|| vec![Inline::Text(s.to_string())])
}

fn typst_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

// ---------------------------------------------------------------------------
// Block parsing
// ---------------------------------------------------------------------------

fn parse_blocks<'a>(nodes: impl Iterator<Item = &'a AstNode<'a>>, footnotes: &FootnoteMap) -> Vec<Block> {
    nodes.filter_map(|n| parse_block(n, footnotes)).collect()
}

fn parse_block<'a>(node: &'a AstNode<'a>, footnotes: &FootnoteMap) -> Option<Block> {
    let val = node.data.borrow();
    match &val.value {
        NodeValue::Heading(h) => {
            let level = h.level;
            drop(val);
            Some(Block::Heading { level, inlines: parse_inlines(node.children(), footnotes) })
        }
        NodeValue::Paragraph => {
            drop(val);
            Some(Block::Paragraph(parse_inlines(node.children(), footnotes)))
        }
        NodeValue::CodeBlock(cb) => {
            let lang = cb.info.trim().to_string();
            let code = cb.literal.trim_end_matches('\n').to_string();
            Some(Block::CodeBlock { lang, code })
        }
        NodeValue::BlockQuote => {
            drop(val);
            let inner = parse_blocks(node.children(), footnotes);
            Some(try_parse_callout(&inner).unwrap_or(Block::BlockQuote(inner)))
        }
        NodeValue::List(nl) => {
            let ordered = nl.list_type == ListType::Ordered;
            drop(val);
            let items: Vec<ListItem> = node.children()
                .map(|item| make_list_item(parse_blocks(item.children(), footnotes)))
                .collect();
            if ordered { Some(Block::OrderedList(items)) } else { Some(Block::BulletList(items)) }
        }
        NodeValue::Table(_) => {
            drop(val);
            Some(parse_table(node, footnotes))
        }
        NodeValue::ThematicBreak => Some(Block::ThematicBreak),
        // Skip footnote definitions — resolved at reference sites
        NodeValue::FootnoteDefinition(_) => None,
        _ => None,
    }
}

static CALLOUT_RE: OnceLock<Regex> = OnceLock::new();

fn try_parse_callout(blocks: &[Block]) -> Option<Block> {
    let re = CALLOUT_RE.get_or_init(|| Regex::new(r"(?i)^\[!(\w+)\][+-]?(?:\s+(.+))?$").unwrap());

    if let Some(Block::Paragraph(inlines)) = blocks.first() {
        if let Some(Inline::Text(first_text)) = inlines.first() {
            if let Some(caps) = re.captures(first_text.trim()) {
                let kind = caps[1].to_lowercase();
                let title = caps.get(2)
                    .map(|m| m.as_str().trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| capitalize(&kind));

                let remaining: Vec<Inline> = inlines[1..].iter()
                    .skip_while(|i| matches!(i, Inline::SoftBreak))
                    .cloned()
                    .collect();

                let mut body: Vec<Block> = Vec::new();
                if !remaining.is_empty() {
                    body.push(Block::Paragraph(remaining));
                }
                body.extend_from_slice(&blocks[1..]);
                return Some(Block::Callout { kind, title, body });
            }
        }
    }
    None
}

fn parse_table<'a>(node: &'a AstNode<'a>, footnotes: &FootnoteMap) -> Block {
    let mut header = TableRow { cells: vec![] };
    let mut rows: Vec<TableRow> = Vec::new();

    // Extract column alignments from the table node itself
    let alignments = {
        let val = node.data.borrow();
        if let NodeValue::Table(ref t) = val.value {
            t.alignments.iter().map(|a| match a {
                TableAlignment::Center => ColAlign::Center,
                TableAlignment::Right  => ColAlign::Right,
                _                      => ColAlign::Left,
            }).collect()
        } else {
            vec![]
        }
    };

    for row_node in node.children() {
        let val = row_node.data.borrow();
        if let NodeValue::TableRow(is_header) = val.value {
            let header_row = is_header;
            drop(val);
            let cells: Vec<Vec<Inline>> = row_node.children()
                .map(|cell| parse_inlines(cell.children(), footnotes))
                .collect();
            let row = TableRow { cells };
            if header_row { header = row; } else { rows.push(row); }
        }
    }
    Block::Table { header, rows, alignments }
}

// ---------------------------------------------------------------------------
// Inline parsing
// ---------------------------------------------------------------------------

fn parse_inlines<'a>(nodes: impl Iterator<Item = &'a AstNode<'a>>, footnotes: &FootnoteMap) -> Vec<Inline> {
    let nodes: Vec<_> = nodes.collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < nodes.len() {
        let node = nodes[i];
        // Peek at HtmlInline tags for <sup> and <sub>
        let html_tag = {
            let val = node.data.borrow();
            if let NodeValue::HtmlInline(html) = &val.value {
                Some(html.trim().to_lowercase())
            } else {
                None
            }
        };
        if let Some(tag) = html_tag {
            let (is_sup, closing) = if tag == "<sup>" {
                (true, "</sup>")
            } else if tag == "<sub>" {
                (false, "</sub>")
            } else {
                // Unknown/unhandled HTML tag — skip silently
                i += 1;
                continue;
            };
            i += 1;
            let mut inner = Vec::new();
            while i < nodes.len() {
                let is_closing = {
                    let v = nodes[i].data.borrow();
                    matches!(&v.value, NodeValue::HtmlInline(h) if h.trim().to_lowercase() == closing)
                };
                if is_closing { i += 1; break; }
                parse_inline_node(nodes[i], &mut inner, footnotes);
                i += 1;
            }
            if is_sup {
                out.push(Inline::Superscript(inner));
            } else {
                out.push(Inline::Subscript(inner));
            }
        } else {
            parse_inline_node(node, &mut out, footnotes);
            i += 1;
        }
    }
    out
}

fn parse_inline_node<'a>(node: &'a AstNode<'a>, out: &mut Vec<Inline>, footnotes: &FootnoteMap) {
    let val = node.data.borrow();
    match &val.value {
        NodeValue::Text(t) => parse_marked_text(t, out),
        NodeValue::Code(c) => out.push(Inline::Code(c.literal.clone())),
        NodeValue::Strong => {
            drop(val);
            out.push(Inline::Bold(parse_inlines(node.children(), footnotes)));
        }
        NodeValue::Emph => {
            drop(val);
            out.push(Inline::Italic(parse_inlines(node.children(), footnotes)));
        }
        NodeValue::Strikethrough => {
            drop(val);
            out.push(Inline::Strikethrough(parse_inlines(node.children(), footnotes)));
        }
        NodeValue::Math(m) => {
            let value = m.literal.clone();
            if m.display_math {
                out.push(Inline::DisplayMath(value));
            } else {
                out.push(Inline::InlineMath(value));
            }
        }
        NodeValue::FootnoteReference(fr) => {
            let inlines = footnotes.get(&fr.name).cloned().unwrap_or_default();
            out.push(Inline::Footnote(inlines));
        }
        NodeValue::Link(link) => {
            let url = link.url.clone();
            drop(val);
            out.push(Inline::Link { text: parse_inlines(node.children(), footnotes), url });
        }
        NodeValue::Image(link) => {
            let src = link.url.clone();
            drop(val);
            let raw_alt = collect_inline_text(&parse_inlines(node.children(), footnotes));
            let (alt, width) = if let Some(pos) = raw_alt.rfind('|') {
                let maybe_w = raw_alt[pos + 1..].trim();
                if let Ok(w) = maybe_w.parse::<u32>() {
                    (raw_alt[..pos].to_string(), Some(w))
                } else {
                    (raw_alt, None)
                }
            } else {
                (raw_alt, None)
            };
            out.push(Inline::Image { alt, src, width });
        }
        NodeValue::SoftBreak => out.push(Inline::SoftBreak),
        NodeValue::LineBreak => out.push(Inline::HardBreak),
        _ => {
            drop(val);
            for child in node.children() {
                parse_inline_node(child, out, footnotes);
            }
        }
    }
}

pub fn collect_inline_text(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(t) => s.push_str(t),
            Inline::Bold(inner) | Inline::Italic(inner) => s.push_str(&collect_inline_text(inner)),
            Inline::Code(t) => s.push_str(t),
            Inline::SoftBreak => s.push(' '),
            Inline::HardBreak => s.push('\n'),
            _ => {}
        }
    }
    s
}

/// Convert Obsidian wikilink image syntax to standard Markdown.
///   ![[path|150]]  →  ![|150](path)
///   ![[path]]      →  ![](path)
static OBSIDIAN_IMAGE_RE: OnceLock<Regex> = OnceLock::new();
static FENCE_RE: OnceLock<Regex> = OnceLock::new();

fn strip_obsidian_comments(input: &str) -> String {
    let fence = FENCE_RE.get_or_init(|| Regex::new(r"^(```|~~~)").unwrap());
    let mut out = String::with_capacity(input.len());
    let mut in_fence = false;
    let mut in_comment = false;

    for line in input.split_inclusive('\n') {
        if fence.is_match(line.trim_start()) {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if in_fence {
            out.push_str(line);
            continue;
        }
        // Strip %% ... %% spans, tracking state across lines for block comments.
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '%' && chars.peek() == Some(&'%') {
                chars.next();
                in_comment = !in_comment;
            } else if !in_comment {
                out.push(ch);
            }
        }
    }
    out
}

fn preprocess_obsidian_images(input: &str) -> String {
    // Matches:
    //   ![[path]]      → no width, no alt
    //   ![[path|150]]  → width only
    // Fenced code blocks (``` or ~~~) are passed through unchanged.
    let re = OBSIDIAN_IMAGE_RE.get_or_init(|| Regex::new(r"!\[\[([^\]\|]+?)(?:\|(\d+))?\]\]").unwrap());
    let fence = FENCE_RE.get_or_init(|| Regex::new(r"^(```|~~~)").unwrap());

    let mut out = String::with_capacity(input.len());
    let mut in_fence = false;

    for line in input.split_inclusive('\n') {
        if fence.is_match(line.trim_start()) {
            in_fence = !in_fence;
            out.push_str(line);
        } else if in_fence {
            // Inside a code block — emit verbatim
            out.push_str(line);
        } else {
            // Normal content — apply the substitution
            let replaced = re.replace_all(line, |caps: &regex::Captures| {
                let path  = &caps[1];
                let width = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                if width.is_empty() {
                    format!("![]({})", path)
                } else {
                    format!("![|{}]({})", width, path)
                }
            });
            out.push_str(&replaced);
        }
    }
    out
}

/// Detect Obsidian task-list syntax at the start of a list item.
/// `- [ ] text`, `- [x] text`, `- [/] text`, etc. →
///   checkbox = Some(char_between_brackets), first text stripped of `[c] `
fn make_list_item(mut blocks: Vec<Block>) -> ListItem {
    let checkbox = if let Some(Block::Paragraph(inlines)) = blocks.first_mut() {
        if let Some(Inline::Text(text)) = inlines.first_mut() {
            let mut chars = text.chars();
            let first = chars.next();
            let ch = chars.next();
            let third = chars.next();
            let fourth = chars.next();
            if first == Some('[') && third == Some(']')
                && (fourth == Some(' ') || fourth == Some('\t'))
            {
                if let Some(c) = ch {
                    let skip = 1 + c.len_utf8() + 2; // '[' + c + '] '
                    *text = text[skip..].to_string();
                    if text.is_empty() {
                        inlines.remove(0);
                    }
                    Some(c)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    ListItem { checkbox, blocks }
}

static MARKED_TEXT_RE: OnceLock<Regex> = OnceLock::new();

/// Splits a raw text node on `==highlight==` markers, pushing `Inline::Highlight`
/// variants into `out`. Subscript is handled via HTML `<sub>` tags in `parse_inlines`.
fn parse_marked_text(text: &str, out: &mut Vec<Inline>) {
    let re = MARKED_TEXT_RE.get_or_init(|| Regex::new(r"==([^=\n]+)==").unwrap());
    let mut last = 0;
    for cap in re.captures_iter(text) {
        let m = cap.get(0).unwrap();
        if m.start() > last {
            out.push(Inline::Text(text[last..m.start()].to_string()));
        }
        if let Some(h) = cap.get(1) {
            out.push(Inline::Highlight(vec![Inline::Text(h.as_str().to_string())]));
        }
        last = m.end();
    }
    if last < text.len() {
        out.push(Inline::Text(text[last..].to_string()));
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_inline_comment() {
        let out = strip_obsidian_comments("hello %%secret%% world\n");
        assert_eq!(out, "hello  world\n");
    }

    #[test]
    fn strip_block_comment() {
        let out = strip_obsidian_comments("before\n%%\nhidden\n%%\nafter\n");
        assert!(!out.contains("hidden"), "comment content must be stripped");
        assert!(out.contains("before"), "content before comment must be kept");
        assert!(out.contains("after"), "content after comment must be kept");
    }

    #[test]
    fn comment_preserved_in_code_block() {
        let input = "```\n%%not a comment%%\n```\n";
        let out = strip_obsidian_comments(input);
        assert_eq!(out, input);
    }

    #[test]
    fn no_comments_unchanged() {
        let input = "just normal text\n";
        assert_eq!(strip_obsidian_comments(input), input);
    }

    #[test]
    fn yaml_block_scalar_produces_multiline_variant() {
        let yaml = "summary: |\n  First line\n  Second line\n";
        let result = parse_yaml_frontmatter(yaml);
        assert_eq!(result.len(), 1);
        let (key, val) = &result[0];
        assert_eq!(key, "summary");
        assert!(
            matches!(val, FrontmatterValue::Multiline(_)),
            "Expected Multiline variant, got: {val:?}"
        );
    }

    #[test]
    fn yaml_block_scalar_line_count() {
        let yaml = "summary: |\n  First line\n  Second line\n  Third line\n";
        let result = parse_yaml_frontmatter(yaml);
        let (_, val) = &result[0];
        if let FrontmatterValue::Multiline(lines) = val {
            assert_eq!(lines.len(), 3, "Expected 3 lines, got {}", lines.len());
        } else {
            panic!("Expected Multiline, got: {val:?}");
        }
    }

    #[test]
    fn yaml_block_scalar_strips_trailing_blank_lines() {
        let yaml = "summary: |\n  Only line\n\n\nother: value\n";
        let result = parse_yaml_frontmatter(yaml);
        let summary = result.iter().find(|(k, _)| k == "summary")
            .expect("summary key missing");
        if let FrontmatterValue::Multiline(lines) = &summary.1 {
            assert_eq!(lines.len(), 1, "Trailing blank lines must be stripped");
        } else {
            panic!("Expected Multiline, got: {:?}", summary.1);
        }
    }

    #[test]
    fn yaml_block_scalar_stops_at_next_key() {
        let yaml = "summary: |\n  Body line\ntitle: My Title\n";
        let result = parse_yaml_frontmatter(yaml);
        assert_eq!(result.len(), 2, "Both keys should be parsed");
        assert_eq!(result[0].0, "summary");
        assert_eq!(result[1].0, "title");
    }

    #[test]
    fn yaml_block_scalar_strip_chomping_indicator() {
        // Obsidian's YAML editor writes |- (strip chomping) instead of |.
        // The parser must treat |- identically to |.
        let yaml = "summary: |-\n  First line\n  Second line\n";
        let result = parse_yaml_frontmatter(yaml);
        assert_eq!(result.len(), 1);
        let (key, val) = &result[0];
        assert_eq!(key, "summary");
        if let FrontmatterValue::Multiline(lines) = val {
            assert_eq!(lines.len(), 2, "Expected 2 lines from |- block scalar");
        } else {
            panic!("Expected Multiline variant for |- block scalar, got: {val:?}");
        }
    }

    #[test]
    fn yaml_block_scalar_empty_body_does_not_eat_next_key() {
        // A | scalar with no indented body must not consume the next key.
        let yaml = "summary: |\ntitle: My Title\n";
        let result = parse_yaml_frontmatter(yaml);
        // summary produces nothing (empty body → not pushed), title is parsed
        let title = result.iter().find(|(k, _)| k == "title")
            .expect("title key must be parsed");
        assert_eq!(
            matches!(&title.1, FrontmatterValue::Inlines(_)),
            true,
            "title must be an Inlines value, not consumed by summary"
        );
    }
}
