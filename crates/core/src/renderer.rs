use crate::ast::*;
use crate::RenderOptions;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn render_typst(doc: &Document, template: Option<&str>, _options: &RenderOptions) -> String {
    let mut out = String::new();

    // Level offset: if the document has a title (from frontmatter or a level-1
    // heading), shift ## → =, ### → ==, etc.
    let has_fm_title = doc.frontmatter.iter().any(|(k, _)| k == "title");
    let title_idx = doc.blocks.iter()
        .position(|b| matches!(b, Block::Heading { level: 1, .. }));
    let level_offset: u8 = if has_fm_title || title_idx.is_some() { 1 } else { 0 };

    // When the frontmatter has no title key, fall back to the text of the
    // first level-1 heading so the template cover page is never blank.
    let synth_title: Option<String> = if !has_fm_title {
        title_idx.and_then(|idx| match &doc.blocks[idx] {
            Block::Heading { inlines, .. } => Some(inlines_to_plain_text(inlines)),
            _ => None,
        })
    } else {
        None
    };

    // Revision / approval section hoisting.
    // The frontmatter values name an H2 section; we extract its content blocks,
    // render them into a Typst content variable, inject that into `fm`, and
    // skip the section entirely during normal body rendering.
    let revision_range = doc.frontmatter.iter()
        .find(|(k, _)| k == "revision-table")
        .and_then(|(_, v)| extract_named_section(&doc.blocks, &fm_plain_text(v)));
    let approval_range = doc.frontmatter.iter()
        .find(|(k, _)| k == "approval-table")
        .and_then(|(_, v)| extract_named_section(&doc.blocks, &fm_plain_text(v)));

    // Emit content variables before the fm dict so fm can reference them.
    if let Some((start, end)) = revision_range {
        let rendered = render_blocks_to_string(&doc.blocks[start + 1..end], level_offset);
        out.push_str(&format!("#let _revision_blocks = [\n{}]\n", rendered));
    }
    if let Some((start, end)) = approval_range {
        let rendered = render_blocks_to_string(&doc.blocks[start + 1..end], level_offset);
        out.push_str(&format!("#let _approval_blocks = [\n{}]\n", rendered));
    }

    // Frontmatter: individual #let variables, available everywhere in the file.
    // Always emit the `fm` dict so that `#show: template.with(fm: fm)` never
    // produces a compile error even when the document has no frontmatter block.
    if !doc.frontmatter.is_empty() || synth_title.is_some() {
        out.push_str("// --- frontmatter ---\n");
        for (key, value) in &doc.frontmatter {
            out.push_str(&format!("#let {} = {}\n", key, render_fm_value(value)));
        }
        if let Some(ref t) = synth_title {
            out.push_str(&format!("#let title = {}\n", typst_string_val(t)));
        }
    }
    // Always emit fm dict so templates can use fm.at("key", default: "").
    out.push_str("#let fm = (\n");
    for (key, _) in &doc.frontmatter {
        out.push_str(&format!("  \"{}\": {},\n", key, key));
    }
    if let Some(ref t) = synth_title {
        out.push_str(&format!("  \"title\": {},\n", typst_string_val(t)));
    }
    if revision_range.is_some() {
        out.push_str("  \"revision-blocks\": _revision_blocks,\n");
    }
    if approval_range.is_some() {
        out.push_str("  \"approval-blocks\": _approval_blocks,\n");
    }
    out.push_str(")\n\n");

    // Emit Font Awesome preamble only when the document contains checkbox items.
    if doc_has_checkboxes(&doc.blocks) {
        out.push_str(FA_PREAMBLE);
    }

    match template {
        Some(path) => {
            // Template owns cover page, TOC and all page structure.
            // Pass the fm dict so the template can use document metadata.
            out.push_str(&format!("#import \"{}\": template, callout\n", path));
            out.push_str("#show: template.with(fm: fm)\n\n");
            // Emit content blocks; skip the H1 title and any hoisted sections.
            for (i, block) in doc.blocks.iter().enumerate() {
                if Some(i) == title_idx { continue; }
                if in_range(i, revision_range) || in_range(i, approval_range) { continue; }
                render_block(&mut out, block, level_offset);
            }
        }
        None => {
            out.push_str(BUILTIN_PREAMBLE);
            // Inline title block from the first level-1 heading.
            if let Some(idx) = title_idx {
                if let Block::Heading { inlines, .. } = &doc.blocks[idx] {
                    out.push_str("\n#align(center)[\n");
                    out.push_str("  #text(size: 26pt, weight: \"bold\")[");
                    render_inlines(&mut out, inlines);
                    out.push_str("]\n]\n");
                }
            }
            out.push_str("\n#outline(indent: 1em)\n\n#pagebreak()\n\n");
            for (i, block) in doc.blocks.iter().enumerate() {
                if Some(i) == title_idx { continue; }
                if in_range(i, revision_range) || in_range(i, approval_range) { continue; }
                render_block(&mut out, block, level_offset);
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Built-in preamble (used when no --template is given)
// ---------------------------------------------------------------------------

const BUILTIN_PREAMBLE: &str = r##"#set page(paper: "a4", margin: (x: 2.5cm, y: 3cm))
#set text(font: ("Verdana", "Arial", "Liberation Sans"), size: 11pt)
#set heading(numbering: "1.1.")
#set table(stroke: 0.5pt)

#let _co-colors = (
  "note":      (fill: rgb("#dbeafe"), accent: rgb("#1d4ed8")),
  "info":      (fill: rgb("#dbeafe"), accent: rgb("#1d4ed8")),
  "tip":       (fill: rgb("#dcfce7"), accent: rgb("#15803d")),
  "hint":      (fill: rgb("#dcfce7"), accent: rgb("#15803d")),
  "important": (fill: rgb("#dcfce7"), accent: rgb("#15803d")),
  "warning":   (fill: rgb("#fef9c3"), accent: rgb("#a16207")),
  "caution":   (fill: rgb("#fef9c3"), accent: rgb("#a16207")),
  "attention": (fill: rgb("#fef9c3"), accent: rgb("#a16207")),
  "danger":    (fill: rgb("#fee2e2"), accent: rgb("#b91c1c")),
  "error":     (fill: rgb("#fee2e2"), accent: rgb("#b91c1c")),
  "bug":       (fill: rgb("#fee2e2"), accent: rgb("#b91c1c")),
  "quote":     (fill: rgb("#f1f5f9"), accent: rgb("#475569")),
  "cite":      (fill: rgb("#f1f5f9"), accent: rgb("#475569")),
)
#let _co-icon-paths = (
  "note":      "<circle cx='12' cy='12' r='10'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/>",
  "info":      "<circle cx='12' cy='12' r='10'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/>",
  "tip":       "<path d='M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5'/><path d='M9 18h6'/><path d='M10 22h4'/>",
  "hint":      "<path d='M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5'/><path d='M9 18h6'/><path d='M10 22h4'/>",
  "important": "<circle cx='12' cy='12' r='10'/><line x1='12' y1='8' x2='12' y2='12'/><line x1='12' y1='16' x2='12.01' y2='16'/>",
  "warning":   "<path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/>",
  "caution":   "<path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/>",
  "attention": "<path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/>",
  "danger":    "<path d='M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z'/>",
  "error":     "<circle cx='12' cy='12' r='10'/><path d='m15 9-6 6'/><path d='m9 9 6 6'/>",
  "bug":       "<path d='m8 2 1.88 1.88'/><path d='M14.12 3.88 16 2'/><path d='M9 7.13v-1a3.003 3.003 0 1 1 6 0v1'/><path d='M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6z'/><path d='M12 20v-9'/><path d='M6.53 9C4.6 8.8 3 7.1 3 5'/><path d='M6 13H2'/><path d='M3 21c0-2.1 1.7-3.9 4-4'/><path d='M20.97 5c0 2.1-1.6 3.8-3.5 4'/><path d='M22 13h-4'/><path d='M17 17c2.3.1 4 1.9 4 4'/>",
  "quote":     "<path d='M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z'/><path d='M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z'/>",
  "cite":      "<path d='M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z'/><path d='M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z'/>",
)
#let callout(kind, title, body) = {
  let c = _co-colors.at(kind, default: (fill: rgb("#f3f4f6"), accent: rgb("#374151")))
  let paths = _co-icon-paths.at(kind, default: none)
  let hdr = context {
    if paths != none {
      let cap_h = measure(text(fill: c.accent, weight: "bold")[H]).height
      let svg = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='" + c.accent.to-hex() + "' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'>" + paths + "</svg>"
      box(height: cap_h, baseline: 0pt, image(bytes(svg), format: "svg")) + h(4pt) + title
    } else { title }
  }
  block(fill: c.fill, inset: (x: 12pt, y: 10pt), radius: 4pt, width: 100%)[
    #text(fill: c.accent, weight: "bold")[#hdr] \
    #body
  ]
}
"##;

// ---------------------------------------------------------------------------
// Font Awesome preamble (emitted only when the document contains checkboxes)
// ---------------------------------------------------------------------------

const FA_PREAMBLE: &str = r##"#let _co-icon(c) = {
  // emoji — rendered via system color emoji font, no installation needed
  let _icons = (
    " ": "🔲",
    "x": "✅",
    "X": "✅",
    "/": "🔄",
    "-": "🚫",
    ">": "➡️",
    "!": "⚠️",
    "?": "❓",
    "i": "💡",
    "I": "📖",
    "*": "⭐",
  )
  let glyph = _icons.at(c, default: "🔲")
  move(dy: -2pt, text(glyph))
}
"##;

// ---------------------------------------------------------------------------
// Built-in template — written to disk by --export-template.
// A user can customise this and pass it back via --template.
// ---------------------------------------------------------------------------

pub const BUILTIN_TEMPLATE: &str = r##"// omd2typst template
// Customise this file and use it with:  --template <this-file>
//
// Required exports consumed by omd2typst:
//   template   — show-rule function wrapping the whole document
//   callout    — renders Obsidian callout blocks

// ---------------------------------------------------------------------------
// Callout colours
// ---------------------------------------------------------------------------
#let _co-colors = (
  "note":      (fill: rgb("#dbeafe"), accent: rgb("#1d4ed8")),
  "info":      (fill: rgb("#dbeafe"), accent: rgb("#1d4ed8")),
  "tip":       (fill: rgb("#dcfce7"), accent: rgb("#15803d")),
  "hint":      (fill: rgb("#dcfce7"), accent: rgb("#15803d")),
  "important": (fill: rgb("#dcfce7"), accent: rgb("#15803d")),
  "warning":   (fill: rgb("#fef9c3"), accent: rgb("#a16207")),
  "caution":   (fill: rgb("#fef9c3"), accent: rgb("#a16207")),
  "attention": (fill: rgb("#fef9c3"), accent: rgb("#a16207")),
  "danger":    (fill: rgb("#fee2e2"), accent: rgb("#b91c1c")),
  "error":     (fill: rgb("#fee2e2"), accent: rgb("#b91c1c")),
  "bug":       (fill: rgb("#fee2e2"), accent: rgb("#b91c1c")),
  "quote":     (fill: rgb("#f1f5f9"), accent: rgb("#475569")),
  "cite":      (fill: rgb("#f1f5f9"), accent: rgb("#475569")),
)

// callout(kind, title)[body]
//   kind  — lowercase type: "note", "warning", …
//   title — display title string (no emoji prefix — icon rendered by this function)
#let _co-icon-paths = (
  "note":      "<circle cx='12' cy='12' r='10'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/>",
  "info":      "<circle cx='12' cy='12' r='10'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/>",
  "tip":       "<path d='M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5'/><path d='M9 18h6'/><path d='M10 22h4'/>",
  "hint":      "<path d='M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5'/><path d='M9 18h6'/><path d='M10 22h4'/>",
  "important": "<circle cx='12' cy='12' r='10'/><line x1='12' y1='8' x2='12' y2='12'/><line x1='12' y1='16' x2='12.01' y2='16'/>",
  "warning":   "<path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/>",
  "caution":   "<path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/>",
  "attention": "<path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/>",
  "danger":    "<path d='M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z'/>",
  "error":     "<circle cx='12' cy='12' r='10'/><path d='m15 9-6 6'/><path d='m9 9 6 6'/>",
  "bug":       "<path d='m8 2 1.88 1.88'/><path d='M14.12 3.88 16 2'/><path d='M9 7.13v-1a3.003 3.003 0 1 1 6 0v1'/><path d='M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6z'/><path d='M12 20v-9'/><path d='M6.53 9C4.6 8.8 3 7.1 3 5'/><path d='M6 13H2'/><path d='M3 21c0-2.1 1.7-3.9 4-4'/><path d='M20.97 5c0 2.1-1.6 3.8-3.5 4'/><path d='M22 13h-4'/><path d='M17 17c2.3.1 4 1.9 4 4'/>",
  "quote":     "<path d='M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z'/><path d='M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z'/>",
  "cite":      "<path d='M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z'/><path d='M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z'/>",
)
#let callout(kind, title, body) = {
  let c = _co-colors.at(kind, default: (fill: rgb("#f3f4f6"), accent: rgb("#374151")))
  let paths = _co-icon-paths.at(kind, default: none)
  let hdr = context {
    if paths != none {
      let cap_h = measure(text(fill: c.accent, weight: "bold")[H]).height
      let svg = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='" + c.accent.to-hex() + "' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'>" + paths + "</svg>"
      box(height: cap_h, baseline: 0pt, image(bytes(svg), format: "svg")) + h(4pt) + title
    } else { title }
  }
  block(fill: c.fill, inset: (x: 12pt, y: 10pt), radius: 4pt, width: 100%)[
    #text(fill: c.accent, weight: "bold")[#hdr] \
    #body
  ]
}

// ---------------------------------------------------------------------------
// template(doc) — applied via  #show: template
// ---------------------------------------------------------------------------
#let template(doc) = {
  set page(
    paper: "a4",
    margin: (x: 2.5cm, y: 3cm),
    header: context {
      if counter(page).get().first() > 2 {
        set text(size: 9pt, fill: luma(120))
        line(length: 100%, stroke: 0.4pt + luma(180))
        v(-8pt)
        // Use a frontmatter variable if available, otherwise a static string.
        // Example: align(right)[#title]
        align(right)[_omd2typst_]
      }
    },
    footer: context {
      if counter(page).get().first() > 2 {
        set text(size: 9pt, fill: luma(120))
        align(center)[#counter(page).display("1 / 1", both: true)]
      }
    },
  )

  set text(font: ("Verdana", "Arial", "Liberation Sans"), size: 11pt)
  set par(justify: true, leading: 0.65em)
  set heading(numbering: "1.1.")

  show heading.where(level: 1): it => {
    v(1.2em)
    text(size: 16pt, weight: "bold", fill: rgb("#1e3a5f"), it)
    v(0.4em)
  }
  show heading.where(level: 2): it => {
    v(0.8em)
    text(size: 13pt, weight: "bold", fill: rgb("#2c5282"), it)
    v(0.2em)
  }

  set table(stroke: 0.5pt, inset: 6pt)
  show table.cell.where(y: 0): set text(weight: "bold")

  show raw.where(block: true): it => block(
    fill: luma(245), inset: (x: 10pt, y: 8pt), radius: 4pt, width: 100%, it,
  )

  doc
}
"##;

// ---------------------------------------------------------------------------
// Checkbox detection helper
// ---------------------------------------------------------------------------

fn doc_has_checkboxes(blocks: &[Block]) -> bool {
    blocks.iter().any(block_has_checkboxes)
}

fn block_has_checkboxes(block: &Block) -> bool {
    match block {
        Block::BulletList(items) | Block::OrderedList(items) => {
            items.iter().any(|item| {
                item.checkbox.is_some()
                    || item.blocks.iter().any(block_has_checkboxes)
            })
        }
        Block::Callout { body, .. } | Block::BlockQuote(body) => {
            body.iter().any(block_has_checkboxes)
        }
        _ => false,
    }
}

// Block rendering
// ---------------------------------------------------------------------------

fn render_block(out: &mut String, block: &Block, level_offset: u8) {
    match block {
        Block::Heading { level, inlines } => {
            let effective = level.saturating_sub(level_offset).max(1);
            out.push_str(&"=".repeat(effective as usize));
            out.push(' ');
            render_inlines(out, inlines);
            out.push_str("\n\n");
        }
        Block::Paragraph(inlines) => {
            render_inlines(out, inlines);
            out.push_str("\n\n");
        }
        Block::CodeBlock { lang, code } => {
            out.push_str("```");
            out.push_str(lang);
            out.push('\n');
            out.push_str(code);
            out.push_str("\n```\n\n");
        }
        Block::Table { header, rows, alignments } => {
            let col_count = header.cells.len().max(1);
            out.push_str(&format!("#table(\n  columns: {},\n", col_count));
            // Emit column alignments when at least one column is non-left
            if alignments.iter().any(|a| !matches!(a, ColAlign::Left)) {
                let align_strs: Vec<&str> = (0..col_count).map(|i| {
                    match alignments.get(i) {
                        Some(ColAlign::Center) => "center",
                        Some(ColAlign::Right)  => "right",
                        _                      => "left",
                    }
                }).collect();
                out.push_str(&format!("  align: ({}),\n", align_strs.join(", ")));
            }
            for cell in &header.cells {
                out.push_str("  [*");
                render_inlines(out, cell);
                out.push_str("*],\n");
            }
            for row in rows {
                for cell in &row.cells {
                    out.push_str("  [");
                    render_inlines(out, cell);
                    out.push_str("],\n");
                }
            }
            out.push_str(")\n\n");
        }
        Block::Callout { kind, title, body } => {
            out.push_str(&format!("#callout({}, {})[\n",
                typst_string_val(kind),
                typst_string_val(title)));
            for b in body {
                render_block(out, b, level_offset);
            }
            out.push_str("]\n\n");
        }
        Block::BulletList(items) => {
            let has_cb = items.iter().any(|it| it.checkbox.is_some());
            if has_cb {
                render_checkbox_list(out, items, level_offset);
            } else {
                for item in items {
                    render_list_item(out, &item.blocks, false, 0, level_offset, 0);
                }
                out.push('\n');
            }
        }
        Block::OrderedList(items) => {
            let has_cb = items.iter().any(|it| it.checkbox.is_some());
            if has_cb {
                render_checkbox_list(out, items, level_offset);
            } else {
                for (i, item) in items.iter().enumerate() {
                    render_list_item(out, &item.blocks, true, i + 1, level_offset, 0);
                }
                out.push('\n');
            }
        }
        Block::BlockQuote(blocks) => {
            out.push_str("#quote(block: true)[\n");
            for b in blocks {
                render_block(out, b, level_offset);
            }
            out.push_str("]\n\n");
        }
        Block::ThematicBreak => {
            out.push_str("#line(length: 100%)\n\n");
        }
    }
}

fn render_list_item(out: &mut String, blocks: &[Block], ordered: bool, num: usize, level_offset: u8, depth: usize) {
    let pad = "  ".repeat(depth);
    let prefix = if ordered { format!("{}{}. ", pad, num) } else { format!("{}- ", pad) };
    out.push_str(&prefix);
    for (i, block) in blocks.iter().enumerate() {
        if i == 0 {
            if let Block::Paragraph(inlines) = block {
                render_inlines(out, inlines);
                out.push('\n');
                continue;
            }
        }
        match block {
            Block::BulletList(items) => {
                for item in items {
                    render_list_item(out, &item.blocks, false, 0, level_offset, depth + 1);
                }
            }
            Block::OrderedList(items) => {
                for (j, item) in items.iter().enumerate() {
                    render_list_item(out, &item.blocks, true, j + 1, level_offset, depth + 1);
                }
            }
            _ => {
                out.push_str(&format!("{}  ", pad));
                render_block(out, block, level_offset);
            }
        }
    }
}

fn render_checkbox_list(out: &mut String, items: &[ListItem], level_offset: u8) {
    out.push_str("#grid(\n  columns: (auto, 1fr),\n  column-gutter: 0.5em,\n  row-gutter: 0.55em,\n  align: (top, top),\n");
    for item in items {
        match item.checkbox {
            Some(c) => {
                let escaped = match c {
                    '"'  => "\\\"".to_string(),
                    '\\' => "\\\\".to_string(),
                    _    => c.to_string(),
                };
                out.push_str(&format!("  [#_co-icon(\"{}\")],\n", escaped));
            }
            None => out.push_str("  [•],\n"),
        }
        out.push_str("  [");
        for (i, block) in item.blocks.iter().enumerate() {
            if i == 0 {
                if let Block::Paragraph(inlines) = block {
                    render_inlines(out, inlines);
                    continue;
                }
            }
            render_block(out, block, level_offset);
        }
        out.push_str("],\n");
    }
    out.push_str(")\n\n");
}

fn render_inlines(out: &mut String, inlines: &[Inline]) {
    for inline in inlines {
        render_inline(out, inline);
    }
}

fn render_inline(out: &mut String, inline: &Inline) {
    match inline {
        Inline::Text(t) => out.push_str(&escape_typst(t)),
        Inline::Bold(inner) => { out.push('*'); render_inlines(out, inner); out.push('*'); }
        Inline::Italic(inner) => { out.push('_'); render_inlines(out, inner); out.push('_'); }
        Inline::Strikethrough(inner) => { out.push_str("#strike["); render_inlines(out, inner); out.push(']'); }
        Inline::Highlight(inner)     => { out.push_str("#highlight["); render_inlines(out, inner); out.push(']'); }
        Inline::Superscript(inner)   => { out.push_str("#super["); render_inlines(out, inner); out.push(']'); }
        Inline::Subscript(inner)     => { out.push_str("#sub["); render_inlines(out, inner); out.push(']'); }
        Inline::InlineMath(s)        => out.push_str(&format!("${}$", s)),
        Inline::DisplayMath(s)       => out.push_str(&format!("\n$ {} $\n", s)),
        Inline::Footnote(inner)      => { out.push_str("#footnote["); render_inlines(out, inner); out.push(']'); }
        Inline::Code(c) => {
            let max_run = c.chars()
                .fold((0usize, 0usize), |(max, cur), ch| {
                    if ch == '`' { (max.max(cur + 1), cur + 1) } else { (max, 0) }
                })
                .0;
            let ticks = "`".repeat(max_run + 1);
            if c.starts_with('`') || c.ends_with('`') {
                out.push_str(&format!("{} {} {}", ticks, c, ticks));
            } else {
                out.push_str(&format!("{}{}{}", ticks, c, ticks));
            }
        }
        Inline::Link { text, url } => {
            out.push_str(&format!("#link({})[", typst_string_val(url)));
            render_inlines(out, text);
            out.push(']');
        }
        Inline::Image { src, alt, width } => {
            let img = match width {
                Some(w) => format!("image({}, width: {}pt)", typst_string_val(src), w),
                None    => format!("image({})", typst_string_val(src)),
            };
            out.push_str(&format!("#figure({}, caption: [{}])", img, escape_typst(alt)));
        }
        Inline::SoftBreak => out.push(' '),
        Inline::HardBreak => out.push_str("\\\n"),
    }
}

fn escape_typst(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('#', "\\#")
        .replace('@', "\\@")
        .replace('$', "\\$")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

/// Recursively extract plain text from an inline sequence (used for title synthesis).
fn inlines_to_plain_text(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(t)                       => s.push_str(t),
            Inline::Code(t)
            | Inline::InlineMath(t)
            | Inline::DisplayMath(t)              => s.push_str(t),
            Inline::Bold(i)
            | Inline::Italic(i)
            | Inline::Strikethrough(i)
            | Inline::Highlight(i)
            | Inline::Superscript(i)
            | Inline::Subscript(i)
            | Inline::Footnote(i)
            | Inline::Link { text: i, .. }        => s.push_str(&inlines_to_plain_text(i)),
            Inline::Image { alt, .. }             => s.push_str(alt),
            Inline::SoftBreak | Inline::HardBreak => s.push(' '),
        }
    }
    s
}

/// Wrap a plain string in a Typst string literal with proper escaping.
fn typst_string_val(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Render a frontmatter value to a Typst expression.
/// Plain-text inlines → `"string"` (preserves string type for template comparisons).
/// Rich inlines (bold, italic, …) → `[content block]`.
/// Raw literals (arrays) → emitted as-is.
fn render_fm_value(value: &FrontmatterValue) -> String {
    match value {
        FrontmatterValue::Raw(s) => s.clone(),
        FrontmatterValue::Inlines(inlines) => {
            if is_plain_inlines(inlines) {
                typst_string_val(&inlines_to_plain_text(inlines))
            } else {
                let mut content = String::new();
                render_inlines(&mut content, inlines);
                format!("[{}]", content)
            }
        }
    }
}

/// Returns true when `inlines` contains only unformatted text and whitespace —
/// no bold, italic, code, links, etc.
fn is_plain_inlines(inlines: &[Inline]) -> bool {
    inlines.iter().all(|i| matches!(i, Inline::Text(_) | Inline::SoftBreak | Inline::HardBreak))
}

/// Extract the plain-text content of a frontmatter value (used for section-name matching).
fn fm_plain_text(value: &FrontmatterValue) -> String {
    match value {
        FrontmatterValue::Raw(s) => {
            if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
                s[1..s.len() - 1].to_string()
            } else {
                s.clone()
            }
        }
        FrontmatterValue::Inlines(inlines) => inlines_to_plain_text(inlines),
    }
}

/// Return the `(start, end)` block index range for the H2 section whose
/// heading text matches `name`. `start` is the heading block itself; `end`
/// is exclusive and stops at the next heading of level ≤ 2 or end of list.
fn extract_named_section(blocks: &[Block], name: &str) -> Option<(usize, usize)> {
    let name = name.trim();
    for (i, block) in blocks.iter().enumerate() {
        if let Block::Heading { level: 2, inlines } = block {
            if inlines_to_plain_text(inlines).trim() == name {
                let end = blocks[i + 1..]
                    .iter()
                    .position(|b| matches!(b, Block::Heading { level, .. } if *level <= 2))
                    .map(|j| i + 1 + j)
                    .unwrap_or(blocks.len());
                return Some((i, end));
            }
        }
    }
    None
}

/// Render a slice of blocks to a Typst string (used for hoisted sections).
fn render_blocks_to_string(blocks: &[Block], level_offset: u8) -> String {
    let mut out = String::new();
    for block in blocks {
        render_block(&mut out, block, level_offset);
    }
    out
}

/// Returns true if `idx` falls within the optional half-open range `[start, end)`.
fn in_range(idx: usize, range: Option<(usize, usize)>) -> bool {
    range.is_some_and(|(start, end)| idx >= start && idx < end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_markdown, RenderOptions};

    fn render_callout(md: &str) -> String {
        let doc = parse_markdown(md);
        render_typst(&doc, None, &RenderOptions::default())
    }

    #[test]
    fn callout_title_has_no_emoji() {
        // The #callout() call in the output must pass a bare title — no emoji prefix.
        let out = render_callout("> [!note] My Note\n> body text\n");
        assert!(
            out.contains("#callout(\"note\", \"My Note\")["),
            "Expected bare title in callout call:\n{out}"
        );
    }

    #[test]
    fn callout_emits_svg_icon_in_preamble() {
        // The preamble must define an image(bytes(...), format: "svg") for icons.
        let out = render_callout("> [!note] My Note\n> body text\n");
        assert!(
            out.contains("image(bytes("),
            "Expected image(bytes( in preamble:\n{out}"
        );
        assert!(
            out.contains("format: \"svg\""),
            "Expected format: \"svg\" in preamble:\n{out}"
        );
    }

    #[test]
    fn callout_unknown_kind_bare_title_no_panic() {
        // Unknown kinds must still render without emoji and without panicking.
        let out = render_callout("> [!foobar] Unknown\n> body\n");
        assert!(
            out.contains("#callout(\"foobar\", \"Unknown\")["),
            "Expected bare title for unknown kind:\n{out}"
        );
    }
}
