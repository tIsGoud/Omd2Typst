use crate::ast::*;
use crate::{MathMode, PdfEmbedMode, RenderOptions, WikilinkMode, DEFAULT_IMAGE_REFERENCE_WIDTH};
use std::cell::RefCell;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn render_typst(doc: &Document, template: Option<&str>, options: &RenderOptions) -> String {
    // Wikilink rendering needs document-wide state (mode, vault, heading labels)
    // inside the free `render_*` functions. It is carried in a thread-local that
    // lives only for the duration of this call.
    MATH_CTX.with(|c| {
        let mut c = c.borrow_mut();
        c.mode = options.math;
        c.failures.clear();
    });
    FILENAME_CAPTIONS.with(|c| c.set(options.filename_captions));
    PDF_CTX.with(|c| *c.borrow_mut() = PdfCtx { mode: options.pdf_embeds, pages: options.pdf_pages.clone() });
    IMAGE_REFERENCE_WIDTH.with(|c| c.set(options.image_reference_width.unwrap_or(DEFAULT_IMAGE_REFERENCE_WIDTH)));
    WIKI_CTX.with(|c| {
        *c.borrow_mut() = WikiCtx {
            mode: options.wikilinks,
            vault: options.obsidian_vault.clone().filter(|v| !v.trim().is_empty()),
            headings: HashMap::new(),
        };
    });
    let out = render_typst_inner(doc, template, options);
    WIKI_CTX.with(|c| *c.borrow_mut() = WikiCtx::default());
    out
}

fn render_typst_inner(doc: &Document, template: Option<&str>, options: &RenderOptions) -> String {
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

    // Appendix numbering: locate the first top-level chapter named by the
    // `appendix-from` frontmatter key. A state-gated numbering switch is emitted
    // just before it (see APPENDIX_SWITCH); when the template does not register a
    // localized label the switch is a no-op, so this degrades gracefully.
    let appendix_from = appendix_from_value(doc);
    let appendix_idx = appendix_from.as_deref()
        .and_then(|af| appendix_boundary_index(doc, level_offset, title_idx, af));

    // Numbering start: chapters before the one named by `numbering-from` stay
    // unnumbered; numbering begins at 1 on the named chapter. Absent, empty or
    // unmatched values leave numbering untouched (the CLI warns when unmatched).
    let numbering_idx = numbering_from_value(doc)
        .and_then(|nf| appendix_boundary_index(doc, level_offset, title_idx, &nf));

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

    // Reading guide: same hoisting as the revision/approval sections. Templates
    // that render `fm.at("reading-guide-blocks")` show it as its own unnumbered
    // page; the section is not part of the numbered body.
    let reading_range = doc.frontmatter.iter()
        .find(|(k, _)| k == "reading-guide")
        .and_then(|(_, v)| extract_named_section(&doc.blocks, &fm_plain_text(v)));

    // Wikilink-to-heading links (`[[#Heading]]`): in link mode, give each body
    // heading a Typst label so such links can point at it. Only headings that
    // end up in the body qualify (not the title, not hoisted sections), and only
    // the first heading with a given text — the same one Obsidian jumps to.
    let mut heading_labels: HashMap<usize, String> = HashMap::new();
    if options.wikilinks == WikilinkMode::Link {
        let mut by_text: HashMap<String, String> = HashMap::new();
        for (i, block) in doc.blocks.iter().enumerate() {
            if Some(i) == title_idx { continue; }
            if in_range(i, revision_range) || in_range(i, approval_range) || in_range(i, reading_range) { continue; }
            if let Block::Heading { inlines, .. } = block {
                let key = normalize_heading(&inlines_to_plain_text(inlines));
                if key.is_empty() || by_text.contains_key(&key) { continue; }
                let label = format!("omd-h-{}", i);
                by_text.insert(key, label.clone());
                heading_labels.insert(i, label);
            }
        }
        WIKI_CTX.with(|c| c.borrow_mut().headings = by_text);
    }

    // Emit content variables before the fm dict so fm can reference them.
    if let Some((start, end)) = revision_range {
        let rendered = render_blocks_to_string(&doc.blocks[start + 1..end], level_offset);
        out.push_str(&format!("#let _revision_blocks = [\n{}]\n", rendered));
    }
    if let Some((start, end)) = approval_range {
        let rendered = render_blocks_to_string(&doc.blocks[start + 1..end], level_offset);
        out.push_str(&format!("#let _approval_blocks = [\n{}]\n", rendered));
    }
    if let Some((start, end)) = reading_range {
        let rendered = render_blocks_to_string(&doc.blocks[start + 1..end], level_offset);
        out.push_str(&format!("#let _reading_guide_blocks = [\n{}]\n", rendered));
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
    // Use (:) (empty Typst dict) when there are no entries; plain () is an
    // empty array whose .at() requires an integer index, not a string key.
    let fm_has_entries = !doc.frontmatter.is_empty()
        || synth_title.is_some()
        || revision_range.is_some()
        || approval_range.is_some()
        || reading_range.is_some();
    if fm_has_entries {
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
        if reading_range.is_some() {
            out.push_str("  \"reading-guide-blocks\": _reading_guide_blocks,\n");
        }
        out.push_str(")\n\n");
    } else {
        out.push_str("#let fm = (:)\n\n");
    }

    // Emit checkbox icon preamble only when the document contains checkbox items.
    if doc_has_checkboxes(&doc.blocks) {
        out.push_str(CB_PREAMBLE);
    }

    match template {
        Some(path) => {
            // Template owns cover page, TOC and all page structure.
            // Pass the fm dict so the template can use document metadata.
            out.push_str(&format!("#import \"{}\": template, callout\n", path));
            out.push_str("#show: template.with(fm: fm)\n\n");
            // Emit content blocks; skip the H1 title and any hoisted sections.
            let mut numbering_off_emitted = false;
            for (i, block) in doc.blocks.iter().enumerate() {
                if Some(i) == title_idx { continue; }
                if in_range(i, revision_range) || in_range(i, approval_range) || in_range(i, reading_range) { continue; }
                if numbering_idx.is_some() && !numbering_off_emitted {
                    out.push_str(NUMBERING_OFF);
                    numbering_off_emitted = true;
                }
                if Some(i) == numbering_idx { out.push_str(NUMBERING_START); }
                if Some(i) == appendix_idx { out.push_str(APPENDIX_SWITCH); }
                render_body_block(&mut out, block, level_offset, heading_labels.get(&i));
            }
        }
        None => {
            out.push_str(BUILTIN_PREAMBLE);
            out.push_str(BUILTIN_COVER);
            out.push_str("\n#outline(indent: 1em)\n\n#pagebreak()\n\n");
            let mut numbering_off_emitted = false;
            for (i, block) in doc.blocks.iter().enumerate() {
                if Some(i) == title_idx { continue; }
                if in_range(i, revision_range) || in_range(i, approval_range) || in_range(i, reading_range) { continue; }
                if numbering_idx.is_some() && !numbering_off_emitted {
                    out.push_str(NUMBERING_OFF);
                    numbering_off_emitted = true;
                }
                if Some(i) == numbering_idx { out.push_str(NUMBERING_START); }
                if Some(i) == appendix_idx { out.push_str(APPENDIX_SWITCH); }
                render_body_block(&mut out, block, level_offset, heading_labels.get(&i));
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Built-in preamble (used when no --template is given)
// ---------------------------------------------------------------------------

const BUILTIN_PREAMBLE: &str = r##"#set page(
  paper: "a4",
  margin: (x: 2.5cm, y: 3cm),
  header: context {
    if counter(page).get().first() > 2 {
      set text(size: 9pt, fill: luma(120))
      line(length: 100%, stroke: 0.4pt + luma(180))
      v(-8pt)
      align(right)[#fm.at("title", default: "")]
    }
  },
  footer: context {
    if counter(page).get().first() > 2 {
      set text(size: 9pt, fill: luma(120))
      align(center)[#counter(page).display("1 / 1", both: true)]
    }
  },
)
#set text(font: ("Verdana", "Arial", "Liberation Sans"), size: 11pt)
#set heading(numbering: "1.1.")
#set table(stroke: 0.5pt)
#show figure: set block(below: 2em)
#show figure.caption: set text(size: 0.85em)
#show link: set text(fill: rgb("#1e3a5f"))

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
      let svg = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='" + c.accent.to-hex() + "' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'>" + paths + "</svg>"
      box(height: 1.5 * cap_h, baseline: 1.5 * cap_h / 12, image(bytes(svg), format: "svg")) + h(4pt) + title
    } else { title }
  }
  block(fill: c.fill, inset: (x: 12pt, y: 10pt), radius: 4pt, width: 100%)[
    #text(fill: c.accent, weight: "bold")[#hdr] \
    #body
  ]
}
"##;

// ---------------------------------------------------------------------------
// Built-in cover page (used when no --template is given)
// ---------------------------------------------------------------------------

const BUILTIN_COVER: &str = r##"
#page(margin: (x: 2.5cm, y: 3cm), header: none, footer: none)[
  #let _t  = fm.at("title",    default: "")
  #let _st = fm.at("subtitle", default: "")
  #let _au = fm.at("author",   default: "")
  #let _dt = fm.at("date",     default: "")
  #let _vs = fm.at("version",  default: "")
  #let _ss = fm.at("status",   default: "")
  #let _su = fm.at("summary",  default: "")
  #v(1fr)
  #if _t != "" [
    #text(size: 28pt, weight: "bold", fill: rgb("#1e3a5f"))[#_t]
    #if _st != "" [
      #v(0.3cm)
      #text(size: 16pt, fill: rgb("#2c5282"))[#_st]
    ]
    #v(0.6cm)
    #line(length: 100%, stroke: 1.5pt + rgb("#1e3a5f"))
    #if _su != "" [
      #v(0.4cm)
      #block(stroke: (left: 3pt + rgb("#1e3a5f")), inset: (left: 12pt, y: 6pt))[#_su]
    ]
    #v(1fr)
    #grid(
      columns: (1fr, 1fr),
      row-gutter: 0.4em,
      [#if _vs != "" [*Version:* #_vs]], [#if _au != "" [*Author:* #_au]],
      [#if _dt != "" [*Date:* #_dt]],    [#if _ss != "" [*Status:* #_ss]],
    )
  ] else [
    #text(size: 16pt, fill: luma(120))[_No title found._]
    #v(0.4cm)
    #text(size: 10pt, fill: luma(120))[
      Add frontmatter to your document to populate this cover page. \
      Supported fields: `title`, `subtitle`, `author`, `date`, `version`, `status`, `summary`. \
      See the README for details.
    ]
    #v(1fr)
  ]
  #v(0.5cm)
]
"##;

// ---------------------------------------------------------------------------
// Checkbox icon preamble (emitted only when the document contains checkboxes)
// ---------------------------------------------------------------------------

const CB_PREAMBLE: &str = r##"#let _cb-icons = (
  " ": (color: rgb("#9ca3af"), paths: "<rect x='3' y='3' width='18' height='18' rx='2' ry='2'/>"),
  "x": (color: rgb("#16a34a"), paths: "<polyline points='9 11 12 14 22 4'/><path d='M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11'/>"),
  "X": (color: rgb("#16a34a"), paths: "<polyline points='9 11 12 14 22 4'/><path d='M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11'/>"),
  "/": (color: rgb("#2563eb"), paths: "<circle cx='12' cy='12' r='10'/><polyline points='12 6 12 12 16 14'/>"),
  "-": (color: rgb("#6b7280"), paths: "<circle cx='12' cy='12' r='10'/><line x1='15' y1='9' x2='9' y2='15'/><line x1='9' y1='9' x2='15' y2='15'/>"),
  ">": (color: rgb("#2563eb"), paths: "<line x1='5' y1='12' x2='19' y2='12'/><polyline points='12 5 19 12 12 19'/>"),
  "!": (color: rgb("#b91c1c"), paths: "<circle cx='12' cy='12' r='10'/><line x1='12' y1='8' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/>"),
  "?": (color: rgb("#7c3aed"), paths: "<circle cx='12' cy='12' r='10'/><path d='M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3'/><line x1='12' y1='17' x2='12.01' y2='17'/>"),
  "i": (color: rgb("#2563eb"), paths: "<circle cx='12' cy='12' r='10'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/>"),
  "I": (color: rgb("#d97706"), paths: "<line x1='9' y1='18' x2='15' y2='18'/><line x1='10' y1='22' x2='14' y2='22'/><path d='M15.09 14c.18-.98.65-1.74 1.41-2.5A4.65 4.65 0 0 0 18 8 6 6 0 0 0 6 8c0 1 .23 2.23 1.5 3.5A4.61 4.61 0 0 1 8.91 14'/>"),
  "*": (color: rgb("#d97706"), paths: "<polygon points='12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2'/>"),
)
#let _co-icon(c) = context {
  let entry = _cb-icons.at(c, default: _cb-icons.at(" "))
  let cap_h = measure(text(weight: "bold")[H]).height
  let svg = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='" + entry.color.to-hex() + "' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'>" + entry.paths + "</svg>"
  box(height: cap_h, baseline: cap_h / 12, image(bytes(svg), format: "svg"))
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
      let svg = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='" + c.accent.to-hex() + "' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'>" + paths + "</svg>"
      box(height: 1.5 * cap_h, baseline: 1.5 * cap_h / 12, image(bytes(svg), format: "svg")) + h(4pt) + title
    } else { title }
  }
  block(fill: c.fill, inset: (x: 12pt, y: 10pt), radius: 4pt, width: 100%)[
    #text(fill: c.accent, weight: "bold")[#hdr] \
    #body
  ]
}

// ---------------------------------------------------------------------------
// template(fm, doc) — applied via  #show: template.with(fm: fm)
// ---------------------------------------------------------------------------
#let template(fm: (:), doc) = {
  set page(
    paper: "a4",
    margin: (x: 2.5cm, y: 3cm),
    header: context {
      if counter(page).get().first() > 2 {
        set text(size: 9pt, fill: luma(120))
        line(length: 100%, stroke: 0.4pt + luma(180))
        v(-8pt)
        align(right)[#fm.at("title", default: "")]
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
  show link: set text(fill: rgb("#1e3a5f"))

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

  // Cover page
  {
    let _t  = fm.at("title",    default: "")
    let _st = fm.at("subtitle", default: "")
    let _au = fm.at("author",   default: "")
    let _dt = fm.at("date",     default: "")
    let _vs = fm.at("version",  default: "")
    let _ss = fm.at("status",   default: "")
    let _su = fm.at("summary",  default: "")
    page(margin: (x: 2.5cm, y: 3cm), header: none, footer: none)[
      #v(1fr)
      #if _t != "" [
        #text(size: 28pt, weight: "bold", fill: rgb("#1e3a5f"))[#_t]
        #if _st != "" [
          #v(0.3cm)
          #text(size: 16pt, fill: rgb("#2c5282"))[#_st]
        ]
        #v(0.6cm)
        #line(length: 100%, stroke: 1.5pt + rgb("#1e3a5f"))
        #if _su != "" [
          #v(0.4cm)
          #block(stroke: (left: 3pt + rgb("#1e3a5f")), inset: (left: 12pt, y: 6pt))[#_su]
        ]
        #v(1fr)
        #grid(
          columns: (1fr, 1fr),
          row-gutter: 0.4em,
          [#if _vs != "" [*Version:* #_vs]], [#if _au != "" [*Author:* #_au]],
          [#if _dt != "" [*Date:* #_dt]],    [#if _ss != "" [*Status:* #_ss]],
        )
      ] else [
        #text(size: 16pt, fill: luma(120))[_No title found._]
        #v(0.4cm)
        #text(size: 10pt, fill: luma(120))[
          Add frontmatter to your document to populate this cover page. \
          Supported fields: `title`, `subtitle`, `author`, `date`, `version`, `status`, `summary`. \
          See the README for details.
        ]
        #v(1fr)
      ]
      #v(0.5cm)
    ]
  }

  // Table of contents
  outline(indent: 1em)
  pagebreak()

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
            // Wrap the table in a scope that turns OFF paragraph justification
            // (so wrapped cells are ragged, not stretched across the column) but
            // turns ON hyphenation. Typst's default `hyphenate: auto` only breaks
            // words when justifying, so without this a long unbreakable word (e.g.
            // "Netwerkconfiguratie") would overflow into the neighbouring column
            // instead of wrapping. `lang` from the outer `set text` still applies.
            out.push_str("#{\n  set par(justify: false)\n  set text(hyphenate: true)\n");
            out.push_str(&format!("  table(\n    columns: {},\n", col_count));
            // Emit column alignments when at least one column is non-left
            if alignments.iter().any(|a| !matches!(a, ColAlign::Left)) {
                let align_strs: Vec<&str> = (0..col_count).map(|i| {
                    match alignments.get(i) {
                        Some(ColAlign::Center) => "center",
                        Some(ColAlign::Right)  => "right",
                        _                      => "left",
                    }
                }).collect();
                out.push_str(&format!("    align: ({}),\n", align_strs.join(", ")));
            }
            for cell in &header.cells {
                out.push_str("    [*");
                render_inlines(out, cell);
                out.push_str("*],\n");
            }
            for row in rows {
                for cell in &row.cells {
                    out.push_str("    [");
                    render_inlines(out, cell);
                    out.push_str("],\n");
                }
            }
            out.push_str("  )\n}\n\n");
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

// ---------------------------------------------------------------------------
// Image captions
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// PDF embeds
// ---------------------------------------------------------------------------

#[derive(Default)]
struct PdfCtx {
    mode: PdfEmbedMode,
    pages: HashMap<String, u32>,
}

thread_local! {
    static PDF_CTX: RefCell<PdfCtx> = RefCell::new(PdfCtx::default());
}

/// The most pages one embed may include (a guard against `page=1-99999999`).
const MAX_PDF_PAGES: u32 = 2000;

/// The pages selected by an Obsidian fragment, in order. In Obsidian `#page=3`
/// is where the embedded viewer starts (the whole document stays scrollable),
/// so it selects page 3 through the last page. A closed range (`page=2-4`)
/// selects those pages, and a comma-separated list (`page=3,5,7` or
/// `page=2-3,7`) selects exactly the pages named. Other keys (`height=…`) are
/// ignored; without a `page` key all pages are selected. Only "to the last
/// page" selections need the page count; pages beyond a known count are dropped.
/// Anything else (`page=3-`, `page=-5`, `page=x`) gives None.
fn pdf_pages_selected(fragment: &str, count: Option<u32>) -> Option<Vec<u32>> {
    let number = |t: &str| t.trim().parse::<u32>().ok().filter(|n| *n >= 1);
    let range = |a: &str, b: &str| -> Option<Vec<u32>> {
        let (first, last) = (number(a)?, number(b)?);
        (first <= last && last - first < MAX_PDF_PAGES).then(|| (first..=last).collect())
    };
    let to_end = |first: u32| -> Option<Vec<u32>> {
        let last = count?;
        (first <= last).then(|| (first..=last).collect())
    };

    let mut pages: Vec<u32> = match fragment.split('&').find_map(|kv| kv.strip_prefix("page=")) {
        None => to_end(1)?,
        Some(value) if value.contains(',') => {
            let mut list = Vec::new();
            for item in value.split(',') {
                match item.split_once('-') {
                    Some((a, b)) => list.extend(range(a, b)?),
                    None => list.push(number(item)?),
                }
            }
            list
        }
        Some(value) => match value.split_once('-') {
            Some((a, b)) => range(a, b)?,
            None => to_end(number(value)?)?,
        },
    };
    if let Some(n) = count {
        pages.retain(|p| *p <= n);
    }
    (!pages.is_empty() && pages.len() <= MAX_PDF_PAGES as usize).then_some(pages)
}

/// Include a PDF embed as one Typst page per PDF page. Returns false (the
/// caller shows the placeholder) when the embed is not a PDF, PDF embeds are
/// off, or the pages are unknown (no `#page=` and no page count supplied).
fn render_pdf_embed(out: &mut String, src: &str) -> bool {
    if embed_extension(src).as_deref() != Some("pdf") {
        return false;
    }
    let (path, fragment) = src.split_once('#').unwrap_or((src, ""));
    let path = path.trim_end();
    let (mode, count) = PDF_CTX.with(|c| {
        let c = c.borrow();
        (c.mode, c.pages.get(path).copied())
    });
    if mode == PdfEmbedMode::Off {
        return false;
    }
    if mode == PdfEmbedMode::Skip {
        return true; // left out on purpose: nothing is shown
    }
    let Some(pages) = pdf_pages_selected(fragment, count) else {
        return false;
    };
    let src = typst_string_val(path);
    out.push('\n');
    for page in pages {
        let image = format!("image({src}, page: {page}, width: 100%, height: 100%, fit: \"contain\")");
        match mode {
            PdfEmbedMode::Full => out.push_str(&format!(
                "#page(margin: 0pt, header: none, footer: none, background: none, foreground: none)[#{image}]\n"
            )),
            _ => out.push_str(&format!("#pagebreak(weak: true)\n#{image}\n")),
        }
    }
    if mode == PdfEmbedMode::Fit {
        out.push_str("#pagebreak(weak: true)\n");
    }
    true
}

thread_local! {
    /// `RenderOptions::filename_captions` for the current render call.
    static FILENAME_CAPTIONS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

thread_local! {
    /// `RenderOptions::image_reference_width` for the current render call.
    static IMAGE_REFERENCE_WIDTH: std::cell::Cell<u32> = const { std::cell::Cell::new(DEFAULT_IMAGE_REFERENCE_WIDTH) };
}

/// Typst width for an Obsidian image width in pixels: the share of the
/// reference width (the text width in Obsidian), capped at 100%. With a
/// reference width of 0 the number is used as points.
fn image_width(px: u32) -> String {
    let reference = IMAGE_REFERENCE_WIDTH.with(|c| c.get());
    if reference == 0 {
        return format!("{px}pt");
    }
    let percent = (f64::from(px) / f64::from(reference) * 100.0).min(100.0);
    let text = format!("{percent:.1}");
    format!("{}%", text.strip_suffix(".0").unwrap_or(&text))
}

/// The caption of an image: its alt text, or — with `filename_captions` and no
/// alt text — the file name without folder and extension. Images the plugin
/// generates itself (`…-mermaid-0.svg`, `…-excalidraw-0.png`) never get a
/// file-name caption.
fn image_caption(alt: &str, src: &str) -> String {
    if !alt.trim().is_empty() || !FILENAME_CAPTIONS.with(|c| c.get()) {
        return alt.to_string();
    }
    let file = src.rsplit(['/', '\\']).next().unwrap_or(src);
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    let generated = ["-mermaid-", "-excalidraw-"].iter().any(|m| {
        stem.rsplit_once(m).is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    });
    if generated { String::new() } else { stem.to_string() }
}

// ---------------------------------------------------------------------------
// Math
// ---------------------------------------------------------------------------

#[derive(Default)]
struct MathCtx {
    mode: MathMode,
    /// Formulas the LaTeX converter could not handle (passed through as typed).
    failures: Vec<String>,
}

thread_local! {
    static MATH_CTX: RefCell<MathCtx> = RefCell::new(MathCtx::default());
}

/// Formulas that could not be converted from LaTeX during the last
/// [`render_typst`] call on this thread, as `formula: reason`. Such formulas
/// are passed through to Typst as typed. The CLI uses this to warn on stderr.
pub fn take_math_failures() -> Vec<String> {
    MATH_CTX.with(|c| std::mem::take(&mut c.borrow_mut().failures))
}

/// The formula as Typst math: converted from LaTeX unless the mode is `typst`.
fn math_source(s: &str) -> String {
    MATH_CTX.with(|c| {
        let mut c = c.borrow_mut();
        if c.mode == MathMode::Typst {
            return s.to_string();
        }
        match crate::math::latex_to_typst(s) {
            Ok(t) => t,
            Err(e) => {
                c.failures.push(format!("{}: {}", s.trim(), e));
                s.to_string()
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Wikilinks
// ---------------------------------------------------------------------------

#[derive(Default)]
struct WikiCtx {
    mode: WikilinkMode,
    vault: Option<String>,
    /// Normalised heading text -> Typst label of the first such body heading.
    headings: HashMap<String, String>,
}

thread_local! {
    static WIKI_CTX: RefCell<WikiCtx> = RefCell::new(WikiCtx::default());
}

/// Heading text as compared by wikilinks: trimmed, lowercased, whitespace
/// collapsed (Obsidian matches headings case-insensitively).
fn normalize_heading(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Percent-encode everything except RFC 3986 unreserved characters.
fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn render_wikilink(out: &mut String, target: &str, fragment: Option<&str>, text: &[Inline], aliased: bool) {
    let (mode, vault, label) = WIKI_CTX.with(|c| {
        let c = c.borrow();
        let label = if target.is_empty() {
            fragment
                .map(|f| normalize_heading(f.rsplit('#').next().unwrap_or(f)))
                .and_then(|k| c.headings.get(&k).cloned())
        } else {
            None
        };
        (c.mode, c.vault.clone(), label)
    });
    match mode {
        WikilinkMode::Keep => {
            let mut raw = String::from("[[");
            raw.push_str(target);
            if let Some(f) = fragment {
                raw.push('#');
                raw.push_str(f);
            }
            if aliased {
                raw.push('|');
                raw.push_str(&inlines_to_plain_text(text));
            }
            raw.push_str("]]");
            out.push_str(&escape_typst(&raw));
        }
        WikilinkMode::Text => render_inlines(out, text),
        WikilinkMode::Bold => {
            out.push_str("#strong[");
            render_inlines(out, text);
            out.push(']');
        }
        WikilinkMode::Link => {
            if let Some(label) = label {
                out.push_str(&format!("#link(<{}>)[", label));
                render_inlines(out, text);
                out.push(']');
            } else if let (false, Some(vault)) = (target.is_empty(), vault) {
                let mut file = target.to_string();
                if let Some(f) = fragment {
                    file.push('#');
                    file.push_str(f);
                }
                let url = format!(
                    "obsidian://open?vault={}&file={}",
                    percent_encode(&vault),
                    percent_encode(&file)
                );
                out.push_str(&format!("#link({})[", typst_string_val(&url)));
                render_inlines(out, text);
                out.push(']');
            } else {
                // Unresolvable here (heading not in the document, or no vault
                // name for a link to another note): plain display text.
                render_inlines(out, text);
            }
        }
    }
}

/// Render a top-level body block; a heading that is a wikilink target gets its
/// Typst label appended (`= Title <omd-h-3>`).
fn render_body_block(out: &mut String, block: &Block, level_offset: u8, label: Option<&String>) {
    match label {
        Some(label) => {
            let mut tmp = String::new();
            render_block(&mut tmp, block, level_offset);
            out.push_str(tmp.trim_end());
            out.push_str(&format!(" <{}>\n\n", label));
        }
        None => render_block(out, block, level_offset),
    }
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
        Inline::Highlight { color, children } => {
            out.push_str(&format!("#highlight(fill: rgb(\"{}\"))[", color.fill_hex()));
            render_inlines(out, children);
            out.push(']');
        }
        // Explicit size: with `typographic` on (the default), some fonts such as
        // Helvetica Neue take the script size from their own metrics and come out tiny.
        Inline::Superscript(inner)   => { out.push_str("#super(typographic: false, size: 0.7em)["); render_inlines(out, inner); out.push(']'); }
        Inline::Subscript(inner)     => { out.push_str("#sub(typographic: false, size: 0.7em)["); render_inlines(out, inner); out.push(']'); }
        Inline::InlineMath(s)        => out.push_str(&format!("${}$", math_source(s))),
        Inline::DisplayMath(s)       => out.push_str(&format!("\n$ {} $\n", math_source(s))),
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
        Inline::WikiLink { target, fragment, text, aliased } => {
            render_wikilink(out, target, fragment.as_deref(), text, *aliased);
        }
        Inline::Image { src, alt, width } => {
            if is_unsupported_embed(src) && render_pdf_embed(out, src) {
                // A PDF annex, included page by page (Typst 0.14+).
            } else if is_unsupported_embed(src) {
                // Non-image embeds — PDFs, note transclusions, audio/video,
                // Obsidian "Bases" (`![[x.base]]`, which the plugin renders to a
                // table before this engine runs, so only the CLI gets here), etc.
                // — cannot become an `image()` call: Typst would reject the file
                // with "unknown image format". Emit a visible placeholder instead.
                out.push_str(&format!(
                    "#block(fill: rgb(\"#fff5ea\"), inset: 8pt, radius: 4pt, width: 100%)[\
                     #text(fill: rgb(\"#b45309\"), weight: \"bold\")[Embed not supported:] \
                     #raw({})]",
                    typst_string_val(src),
                ));
            } else {
                let img = match width {
                    Some(w) => format!("image({}, width: {})", typst_string_val(src), image_width(*w)),
                    None    => format!("image({})", typst_string_val(src)),
                };
                out.push_str(&format!("#figure({}, caption: [{}])", img, escape_typst(&image_caption(alt, src))));
            }
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
            | Inline::Superscript(i)
            | Inline::Subscript(i)
            | Inline::Footnote(i)
            | Inline::Link { text: i, .. }
            | Inline::WikiLink { text: i, .. }    => s.push_str(&inlines_to_plain_text(i)),
            Inline::Highlight { children, .. }    => s.push_str(&inlines_to_plain_text(children)),
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

/// Emitted just before the first appendix chapter. Resets the heading counter
/// and switches heading numbering to the appendix scheme — but only when a
/// template has registered a localized label in `state("omd-appendix-label")`.
/// Otherwise the wrapper reproduces normal `1.1.` numbering and skips the reset,
/// so a non-appendix-aware template (or the built-in one) degrades gracefully.
/// The over-long `"A.1.1.1.1"` pattern is intentional: Typst uses only as many
/// counting symbols as there are numbers, giving `A.1`, `A.1.1`, ….
const APPENDIX_SWITCH: &str = r##"#context { if state("omd-appendix-label", none).get() != none { counter(heading).update(0) } }
#set heading(numbering: (..n) => context {
  let label = state("omd-appendix-label", none).get()
  let nums = n.pos()
  if label == none { numbering("1.1.", ..nums) }
  else if nums.len() == 1 { [#label #numbering("A", nums.first()) -] }
  else { numbering("A.1.1.1.1", ..nums) }
})
"##;

/// Emitted before the first body block when `numbering-from` is set and
/// matched: headings are unnumbered (but still listed in the outline) until
/// `NUMBERING_START`. Standard `1.1.` numbering is restored there, which is the
/// scheme every shipped template and the built-in preamble use.
const NUMBERING_OFF: &str = "#set heading(numbering: none)\n";
const NUMBERING_START: &str = "#set heading(numbering: \"1.1.\")\n#counter(heading).update(0)\n";

/// Plain-text value of a chapter-naming frontmatter key, trimmed.
/// `None` when the key is absent or empty.
fn chapter_key_value(doc: &Document, key: &str) -> Option<String> {
    doc.frontmatter.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| fm_plain_text(v).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Returns the `reading-guide` value when it is set but names no H2 section,
/// so the CLI can warn. `None` when unset or successfully matched.
pub fn reading_guide_unmatched(doc: &Document) -> Option<String> {
    let name = chapter_key_value(doc, "reading-guide")?;
    match extract_named_section(&doc.blocks, &name) {
        Some(_) => None,
        None => Some(name),
    }
}

fn appendix_from_value(doc: &Document) -> Option<String> {
    chapter_key_value(doc, "appendix-from")
}

fn numbering_from_value(doc: &Document) -> Option<String> {
    chapter_key_value(doc, "numbering-from")
}

/// Block index of the first effective-level-1 heading whose trimmed plain text
/// equals `appendix_from` — the appendix boundary. The document title is skipped.
fn appendix_boundary_index(
    doc: &Document,
    level_offset: u8,
    title_idx: Option<usize>,
    appendix_from: &str,
) -> Option<usize> {
    doc.blocks.iter().enumerate().find_map(|(i, b)| {
        if Some(i) == title_idx {
            return None;
        }
        if let Block::Heading { level, inlines } = b {
            let effective = level.saturating_sub(level_offset).max(1);
            if effective == 1 && inlines_to_plain_text(inlines).trim() == appendix_from {
                return Some(i);
            }
        }
        None
    })
}

/// Returns the `appendix-from` value when it is set but names no top-level
/// chapter, so the CLI can warn. `None` when unset or successfully matched.
pub fn appendix_unmatched(doc: &Document) -> Option<String> {
    chapter_unmatched(doc, appendix_from_value(doc)?)
}

/// Returns the `numbering-from` value when it is set but names no top-level
/// chapter, so the CLI can warn. `None` when unset or successfully matched.
pub fn numbering_unmatched(doc: &Document) -> Option<String> {
    chapter_unmatched(doc, numbering_from_value(doc)?)
}

fn chapter_unmatched(doc: &Document, appendix_from: String) -> Option<String> {
    let title_idx = doc.blocks.iter()
        .position(|b| matches!(b, Block::Heading { level: 1, .. }));
    let has_fm_title = doc.frontmatter.iter().any(|(k, _)| k == "title");
    let level_offset = if has_fm_title || title_idx.is_some() { 1 } else { 0 };
    if appendix_boundary_index(doc, level_offset, title_idx, &appendix_from).is_some() {
        None
    } else {
        Some(appendix_from)
    }
}

/// File extensions Typst can embed via `image()`.
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp"];

/// Lowercased file extension of an embed target, ignoring any `#…`/`?…`
/// fragment. `None` when the target has no extension (e.g. a bare note
/// transclusion `![[note]]`), which is left to render as an image as before.
fn embed_extension(src: &str) -> Option<String> {
    let path = src.split(['#', '?']).next().unwrap_or(src).trim_end();
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let dot = name.rfind('.')?;
    if dot == 0 {
        return None; // leading-dot name like ".gitkeep" — not an extension
    }
    Some(name[dot + 1..].to_ascii_lowercase())
}

/// True when an embed target is *not* a Typst-renderable image (e.g. `.base`,
/// `.pdf`, `.md`, `.mp3`, `.mp4`, `.canvas`). Such embeds are handled by the
/// Obsidian plugin but not by this engine, so the renderer emits a warning
/// rather than an `image()` call. Extension-less targets are treated as images.
fn is_unsupported_embed(src: &str) -> bool {
    match embed_extension(src) {
        Some(ext) => !IMAGE_EXTS.contains(&ext.as_str()),
        None => false,
    }
}

/// Collect the `src` of every embed this engine cannot render as an image
/// (`.base`, `.pdf`, `.md`, audio/video, …). First-seen order, de-duplicated.
/// The CLI uses this to warn on stderr; the rendered document also shows an
/// inline notice for each such embed.
pub fn collect_unsupported_embeds(doc: &Document) -> Vec<String> {
    let mut found = Vec::new();
    for block in &doc.blocks {
        collect_block_embeds(block, &mut found);
    }
    let mut seen = std::collections::HashSet::new();
    found.into_iter().filter(|s| seen.insert(s.clone())).collect()
}

fn collect_block_embeds(block: &Block, out: &mut Vec<String>) {
    match block {
        Block::Heading { inlines, .. } => collect_inline_embeds(inlines, out),
        Block::Paragraph(inlines) => collect_inline_embeds(inlines, out),
        Block::Table { header, rows, .. } => {
            for cell in &header.cells {
                collect_inline_embeds(cell, out);
            }
            for row in rows {
                for cell in &row.cells {
                    collect_inline_embeds(cell, out);
                }
            }
        }
        Block::Callout { body, .. } => {
            for b in body {
                collect_block_embeds(b, out);
            }
        }
        Block::BlockQuote(body) => {
            for b in body {
                collect_block_embeds(b, out);
            }
        }
        Block::BulletList(items) | Block::OrderedList(items) => {
            for item in items {
                for b in &item.blocks {
                    collect_block_embeds(b, out);
                }
            }
        }
        Block::CodeBlock { .. } | Block::ThematicBreak => {}
    }
}

fn collect_inline_embeds(inlines: &[Inline], out: &mut Vec<String>) {
    for inline in inlines {
        match inline {
            Inline::Image { src, .. } if is_unsupported_embed(src) => out.push(src.clone()),
            Inline::Bold(children)
            | Inline::Italic(children)
            | Inline::Strikethrough(children)
            | Inline::Footnote(children)
            | Inline::Superscript(children)
            | Inline::Subscript(children) => collect_inline_embeds(children, out),
            Inline::Highlight { children, .. } => collect_inline_embeds(children, out),
            Inline::Link { text, .. } | Inline::WikiLink { text, .. } => collect_inline_embeds(text, out),
            _ => {}
        }
    }
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
        FrontmatterValue::Multiline(lines) => {
            let mut content = String::new();
            for (i, line_inlines) in lines.iter().enumerate() {
                if i > 0 {
                    content.push_str("\\\n");
                }
                render_inlines(&mut content, line_inlines);
            }
            format!("[{}]", content)
        }
    }
}

/// Returns true when `inlines` contains only unformatted text and whitespace —
/// no bold, italic, code, links, etc.
fn is_plain_inlines(inlines: &[Inline]) -> bool {
    // In text mode a wikilink is just its display text, so it stays plain.
    let text_mode = WIKI_CTX.with(|c| c.borrow().mode == WikilinkMode::Text);
    inlines.iter().all(|i| match i {
        Inline::Text(_) | Inline::SoftBreak | Inline::HardBreak => true,
        Inline::WikiLink { text, .. } => text_mode && is_plain_inlines(text),
        _ => false,
    })
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
        FrontmatterValue::Multiline(lines) => lines
            .iter()
            .map(|line| inlines_to_plain_text(line))
            .collect::<Vec<_>>()
            .join(" "),
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
    use crate::{parse_markdown, PdfEmbedMode, RenderOptions};

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
    fn checkbox_uses_svg_icon_no_emoji() {
        let doc = parse_markdown("- [x] done\n- [ ] todo\n- [/] in progress\n");
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(out.contains("image(bytes("), "Expected image(bytes( for checkbox icons");
        assert!(out.contains("format: \"svg\""), "Expected SVG format for checkbox icons");
        // No emoji characters in output
        assert!(!out.contains('✅'), "Expected no emoji in checkbox output");
        assert!(!out.contains('🔲'), "Expected no emoji in checkbox output");
    }

    #[test]
    fn highlight_colours_from_leading_dot() {
        let cases = [
            ("==plain==", "#fdf1a5", "plain"),
            ("==🔴text==", "#f8bcb9", "text"),
            ("==🟠text==", "#ffdfbf", "text"),
            ("==🟡text==", "#f4e5b8", "text"),
            ("==🟢text==", "#c8f0c9", "text"),
            ("==🔵text==", "#b3ccf5", "text"),
            ("==🟣text==", "#e8cbf6", "text"),
        ];
        for (md, fill, body) in cases {
            let out = render_typst(&parse_markdown(&format!("{md}\n")), None, &RenderOptions::default());
            let want = format!("#highlight(fill: rgb(\"{fill}\"))[{body}]");
            assert!(out.contains(&want), "Expected {want} for {md}:\n{out}");
        }
    }

    #[test]
    fn super_and_subscript_have_explicit_size() {
        let out = render_typst(&parse_markdown("E = mc<sup>2</sup> and H<sub>2</sub>O\n"), None, &RenderOptions::default());
        assert!(out.contains("#super(typographic: false, size: 0.7em)[2]"), "{out}");
        assert!(out.contains("#sub(typographic: false, size: 0.7em)[2]"), "{out}");
    }

    #[test]
    fn latex_math_is_converted_by_default() {
        let out = render_typst(&parse_markdown("Log: $\\log$ and $$\\frac{a}{b}$$\n"), None, &RenderOptions::default());
        assert!(out.contains("$log$"), "{out}");
        assert!(out.contains("$ a/b $"), "{out}");
    }

    #[test]
    fn typst_math_mode_passes_formulas_through() {
        let opts = RenderOptions { math: MathMode::Typst, ..Default::default() };
        let out = render_typst(&parse_markdown("$sum_(k=1)^n k$ and $log(x)$\n"), None, &opts);
        assert!(out.contains("$sum_(k=1)^n k$"), "{out}");
        assert!(out.contains("$log(x)$"), "{out}");
    }

    #[test]
    fn failed_latex_conversion_falls_back_and_is_reported() {
        let out = render_typst(&parse_markdown("$\\frac{a$\n"), None, &RenderOptions::default());
        assert!(out.contains("$\\frac{a$"), "{out}");
        let failures = take_math_failures();
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(take_math_failures().is_empty());
    }

    fn image_out(md: &str, filename_captions: bool) -> String {
        let opts = RenderOptions { filename_captions, ..Default::default() };
        render_typst(&parse_markdown(&format!("{md}\n")), None, &opts)
    }

    #[test]
    fn obsidian_embed_options_give_width_and_alt() {
        for (md, caption, width) in [
            ("![[_assets/laptop.png|200|alt text]]", "alt text", Some("28.6%")),
            ("![[_assets/laptop.png|alt text|200]]", "alt text", Some("28.6%")),
            ("![[_assets/laptop.png|100x145|alt text]]", "alt text", Some("14.3%")),
            ("![[_assets/laptop.png|alt text]]", "alt text", None),
            ("![[_assets/laptop.png|200]]", "", Some("28.6%")),
            ("![[_assets/laptop.png]]", "", None),
            ("![alt text|200](_assets/laptop.png)", "alt text", Some("28.6%")),
        ] {
            let out = image_out(md, false);
            assert!(out.contains(&format!("caption: [{caption}]")), "{md}:\n{out}");
            match width {
                Some(w) => assert!(out.contains(&format!("width: {w}")), "{md}:\n{out}"),
                None => assert!(!out.contains("width:") || !out.contains("laptop.png\", width"), "{md}:\n{out}"),
            }
        }
    }

    #[test]
    fn image_width_is_a_share_of_the_reference_width() {
        let width = |px: u32, reference: Option<u32>| {
            let opts = RenderOptions { image_reference_width: reference, ..Default::default() };
            let out = render_typst(&parse_markdown(&format!("![[a.png|{px}]]\n")), None, &opts);
            out.split("image(\"a.png\", width: ").nth(1).unwrap().split(')').next().unwrap().to_string()
        };
        assert_eq!(width(400, None), "57.1%");
        assert_eq!(width(350, None), "50%");
        assert_eq!(width(700, None), "100%");
        assert_eq!(width(900, None), "100%");
        assert_eq!(width(400, Some(800)), "50%");
        assert_eq!(width(400, Some(0)), "400pt");
    }

    fn pdf_out(md: &str, mode: PdfEmbedMode, pages: &[(&str, u32)]) -> String {
        let opts = RenderOptions {
            pdf_embeds: mode,
            pdf_pages: pages.iter().map(|(p, n)| (p.to_string(), *n)).collect(),
            ..Default::default()
        };
        render_typst(&parse_markdown(&format!("{md}\n")), None, &opts)
    }

    #[test]
    fn pdf_embed_fit_gives_one_page_per_pdf_page_inside_the_margins() {
        let out = pdf_out("![[annex.pdf]]", PdfEmbedMode::Fit, &[("annex.pdf", 3)]);
        for p in 1..=3 {
            assert!(out.contains(&format!("#image(\"annex.pdf\", page: {p}, width: 100%, height: 100%, fit: \"contain\")")), "{out}");
        }
        assert!(!out.contains("page: 4"), "{out}");
        // A weak page break before each page, and one after the last.
        assert_eq!(out.matches("#pagebreak(weak: true)\n#image(\"annex.pdf\"").count(), 3, "{out}");
        assert!(out.contains("fit: \"contain\")\n#pagebreak(weak: true)\n"), "{out}");
        assert!(!out.contains("[#image(\"annex.pdf\""), "{out}");
    }

    #[test]
    fn pdf_embed_full_pages_drop_margins_header_and_footer() {
        let out = pdf_out("![[annex.pdf]]", PdfEmbedMode::Full, &[("annex.pdf", 2)]);
        assert_eq!(out.matches("#page(margin: 0pt, header: none, footer: none, background: none, foreground: none)[#image(").count(), 2, "{out}");
        assert!(!out.contains("#pagebreak(weak: true)\n#image(\"annex.pdf\""), "{out}");
    }

    fn pdf_page_numbers(out: &str) -> Vec<u32> {
        out.split("page: ").skip(1).filter_map(|t| t.split(',').next()?.parse().ok()).collect()
    }

    #[test]
    fn pdf_embed_page_selections() {
        let pages = |fragment: &str, count: &[(&str, u32)]| {
            pdf_page_numbers(&pdf_out(&format!("![[annex.pdf#{fragment}]]"), PdfEmbedMode::Full, count))
        };
        // `page=3` starts at page 3 (as in Obsidian's viewer) and runs to the last page.
        assert_eq!(pages("page=3", &[("annex.pdf", 5)]), vec![3, 4, 5]);
        assert_eq!(pages("page=3&height=400", &[("annex.pdf", 5)]), vec![3, 4, 5]);
        // A closed range needs no page count and is cut off at the real last page.
        assert_eq!(pages("page=2-4", &[]), vec![2, 3, 4]);
        assert_eq!(pages("page=3-3", &[]), vec![3]);
        assert_eq!(pages("page=2-9", &[("annex.pdf", 4)]), vec![2, 3, 4]);
        // A list names exactly the pages wanted, in the order written; spaces are fine.
        assert_eq!(pages("page=3,5, 7", &[]), vec![3, 5, 7]);
        assert_eq!(pages("page=2-3,7", &[]), vec![2, 3, 7]);
        assert_eq!(pages("page=7,3", &[("annex.pdf", 5)]), vec![3]);
    }

    #[test]
    fn pdf_embed_unclear_page_selections_stay_placeholders() {
        // Open ranges are not supported; "to the last page" needs the page count.
        for (fragment, count) in [
            ("page=3-", Some(5)), ("page=-5", Some(5)), ("page=x", Some(5)), ("page=0", Some(5)),
            ("page=3,", Some(5)), ("page=3,a", Some(5)), ("page=5-3", Some(5)), ("page=3", None), ("page=9", Some(5)),
        ] {
            let pages: Vec<(&str, u32)> = count.map(|n| ("annex.pdf", n)).into_iter().collect();
            let out = pdf_out(&format!("![[annex.pdf#{fragment}]]"), PdfEmbedMode::Fit, &pages);
            assert!(out.contains("Embed not supported:"), "{fragment}: {out}");
        }
    }

    #[test]
    fn pdf_embed_skip_shows_nothing() {
        let out = pdf_out("Before\n\n![[annex.pdf]]\n\nAfter", PdfEmbedMode::Skip, &[("annex.pdf", 3)]);
        assert!(!out.contains("Embed not supported:") && !out.contains("page: 1"), "{out}");
        assert!(out.contains("Before") && out.contains("After"), "{out}");
    }

    #[test]
    fn pdf_embed_stays_a_placeholder_when_off_or_pages_unknown() {
        for out in [
            pdf_out("![[annex.pdf]]", PdfEmbedMode::Off, &[("annex.pdf", 3)]),
            pdf_out("![[annex.pdf]]", PdfEmbedMode::Fit, &[]),
        ] {
            assert!(out.contains("Embed not supported:") && !out.contains("page: 1"), "{out}");
        }
    }

    #[test]
    fn filename_caption_only_without_alt_text() {
        assert!(image_out("![[_assets/laptop.png]]", true).contains("caption: [laptop]"));
        assert!(image_out("![[_assets/laptop.png|200]]", true).contains("caption: [laptop]"));
        assert!(image_out("![](_assets/my%20pic.png)", true).contains("caption: [my%20pic]"));
        // Alt text wins over the file name.
        let out = image_out("![[_assets/laptop.png|200|alt text]]", true);
        assert!(out.contains("caption: [alt text]") && !out.contains("[laptop]"), "{out}");
        // Off by default; plugin-generated images never get one.
        assert!(!image_out("![[_assets/laptop.png]]", false).contains("[laptop]"));
        assert!(!image_out("![](<doc-mermaid-0.svg>)", true).contains("[doc-mermaid-0]"));
        assert!(!image_out("![](<doc-excalidraw-1.png>)", true).contains("[doc-excalidraw-1]"));
    }

    #[test]
    fn highlight_unknown_emoji_stays_text() {
        let out = render_typst(&parse_markdown("==⚫text==\n"), None, &RenderOptions::default());
        assert!(out.contains("#highlight(fill: rgb(\"#fdf1a5\"))[⚫text]"), "{out}");
    }

    #[test]
    fn base_embed_emits_warning_not_image() {
        // A `![[x.base]]` embed must NOT become an image() call (Typst rejects
        // `.base` as an image format); it should render a visible warning.
        let doc = parse_markdown("![[_assets/test.base]]\n");
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(
            out.contains("Embed not supported:"),
            "Expected embed warning in output:\n{out}"
        );
        assert!(
            !out.contains("image(\"_assets/test.base\")"),
            "Base embed must not be rendered as an image():\n{out}"
        );
    }

    #[test]
    fn non_image_embeds_emit_warning() {
        // The warning generalizes beyond .base: PDFs, note transclusions,
        // audio/video, etc. must all render a placeholder, never image().
        for src in ["doc.pdf", "note.md", "clip.mp4", "sound.mp3", "board.canvas"] {
            let doc = parse_markdown(&format!("![[{src}]]\n"));
            let out = render_typst(&doc, None, &RenderOptions::default());
            assert!(
                out.contains("Embed not supported:"),
                "Expected warning for {src}:\n{out}"
            );
            assert!(
                !out.contains(&format!("image(\"{src}\")")),
                "{src} must not be rendered as an image():\n{out}"
            );
        }
    }

    #[test]
    fn regular_images_still_render_as_figure() {
        // The carve-out must not affect real image formats.
        for src in ["pic.png", "photo.JPG", "diagram.svg", "anim.gif", "shot.webp"] {
            let doc = parse_markdown(&format!("![alt]({src})\n"));
            let out = render_typst(&doc, None, &RenderOptions::default());
            assert!(
                out.contains(&format!("image(\"{src}\")")),
                "Expected {src} to still render as image():\n{out}"
            );
        }
    }

    #[test]
    fn appendix_from_emits_switch_before_matched_chapter() {
        let md = "---\ntitle: Doc\nappendix-from: Appendices\n---\n\n\
                  ## Intro\n\n## Appendices\n\n## More\n";
        let doc = parse_markdown(md);
        let out = render_typst(&doc, None, &RenderOptions::default());
        // The state-gated switch is emitted…
        assert!(
            out.contains("state(\"omd-appendix-label\", none)"),
            "expected appendix switch in output:\n{out}"
        );
        // …immediately before the "Appendices" chapter, not before "Intro".
        let switch_at = out.find("omd-appendix-label").unwrap();
        let appendices_at = out.find("= Appendices").unwrap();
        let intro_at = out.find("= Intro").unwrap();
        assert!(switch_at < appendices_at, "switch must precede the appendix heading");
        assert!(switch_at > intro_at, "switch must not precede a normal chapter");
        // Graceful design: no template symbol is imported/referenced.
        assert!(!out.contains("appendix-numbering"), "must not reference a template symbol");
    }

    #[test]
    fn no_appendix_from_leaves_output_unchanged() {
        let md = "---\ntitle: Doc\n---\n\n## Intro\n\n## Body\n";
        let doc = parse_markdown(md);
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(!out.contains("omd-appendix-label"), "no switch without appendix-from:\n{out}");
        assert!(appendix_unmatched(&doc).is_none());
    }

    #[test]
    fn appendix_from_no_match_warns_and_emits_no_switch() {
        // Names a chapter that doesn't exist → no switch, and the CLI-facing
        // helper reports the unmatched value.
        let md = "---\ntitle: Doc\nappendix-from: Ghost\n---\n\n## Intro\n\n## Body\n";
        let doc = parse_markdown(md);
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(!out.contains("omd-appendix-label"), "no switch when unmatched:\n{out}");
        assert_eq!(appendix_unmatched(&doc), Some("Ghost".to_string()));
    }

    #[test]
    fn appendix_from_matches_only_top_level_chapters() {
        // A level-2 heading with the same text must NOT trigger the switch.
        let md = "---\ntitle: Doc\nappendix-from: Details\n---\n\n## Intro\n\n### Details\n";
        let doc = parse_markdown(md);
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(!out.contains("omd-appendix-label"), "level-2 match must not switch:\n{out}");
        assert_eq!(appendix_unmatched(&doc), Some("Details".to_string()));
    }

    #[test]
    fn numbering_from_unnumbers_chapters_before_and_restarts_at_match() {
        let md = "---\ntitle: Doc\nnumbering-from: Introduction\n---\n\n\
                  ## Reading guide\n\n## Introduction\n\n## Body\n";
        let doc = parse_markdown(md);
        let out = render_typst(&doc, None, &RenderOptions::default());
        let off = out.find("numbering: none").expect("numbering must be switched off");
        let guide = out.find("= Reading guide").unwrap();
        let start = out.find("counter(heading).update(0)").expect("numbering must restart");
        let intro = out.find("= Introduction").unwrap();
        assert!(off < guide, "numbering off before the first chapter");
        assert!(guide < start && start < intro, "restart between guide and Introduction");
        assert!(numbering_unmatched(&doc).is_none());
    }

    #[test]
    fn numbering_from_missing_or_empty_changes_nothing() {
        for fm in ["title: Doc\n", "title: Doc\nnumbering-from:\n"] {
            let md = format!("---\n{fm}---\n\n## Intro\n\n## Body\n");
            let doc = parse_markdown(&md);
            let out = render_typst(&doc, None, &RenderOptions::default());
            assert!(!out.contains("numbering: none"), "no switch for {fm:?}:\n{out}");
            assert!(numbering_unmatched(&doc).is_none());
        }
    }

    #[test]
    fn numbering_from_no_match_warns_and_changes_nothing() {
        let md = "---\ntitle: Doc\nnumbering-from: Ghost\n---\n\n## Intro\n\n### Details\n";
        let doc = parse_markdown(md);
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(!out.contains("numbering: none"), "unmatched must not unnumber:\n{out}");
        assert_eq!(numbering_unmatched(&doc), Some("Ghost".to_string()));
        // Only top-level chapters match.
        let md = "---\ntitle: Doc\nnumbering-from: Details\n---\n\n## Intro\n\n### Details\n";
        assert_eq!(numbering_unmatched(&parse_markdown(md)), Some("Details".to_string()));
    }

    #[test]
    fn reading_guide_is_hoisted_out_of_the_body() {
        let md = "---\ntitle: Doc\nreading-guide: Guide\n---\n\n\
                  ## Guide\n\nRead this first.\n\n## Intro\n\nBody.\n";
        let doc = parse_markdown(md);
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(out.contains("#let _reading_guide_blocks = ["), "hoisted variable:\n{out}");
        assert!(out.contains("\"reading-guide-blocks\": _reading_guide_blocks"), "fm entry:\n{out}");
        assert!(!out.contains("= Guide"), "section must not be rendered in the body:\n{out}");
        assert!(out.contains("= Intro"), "other chapters stay");
        assert!(reading_guide_unmatched(&doc).is_none());
    }

    #[test]
    fn reading_guide_missing_empty_or_unmatched_changes_nothing() {
        for fm in ["title: Doc\n", "title: Doc\nreading-guide:\n"] {
            let md = format!("---\n{fm}---\n\n## Guide\n\nText.\n");
            let doc = parse_markdown(&md);
            let out = render_typst(&doc, None, &RenderOptions::default());
            assert!(!out.contains("_reading_guide_blocks"), "no hoist for {fm:?}:\n{out}");
            assert!(out.contains("= Guide"));
            assert!(reading_guide_unmatched(&doc).is_none());
        }
        let md = "---\ntitle: Doc\nreading-guide: Ghost\n---\n\n## Guide\n\nText.\n";
        let doc = parse_markdown(md);
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(out.contains("= Guide"), "unmatched keeps the chapter in the body");
        assert_eq!(reading_guide_unmatched(&doc), Some("Ghost".to_string()));
    }

    fn render_wiki(md: &str, mode: crate::WikilinkMode, vault: Option<&str>) -> String {
        let doc = parse_markdown(md);
        let opts = RenderOptions { wikilinks: mode, obsidian_vault: vault.map(str::to_string), ..Default::default() };
        render_typst(&doc, None, &opts)
    }

    const WIKI_MD: &str = "# T\n\n## Intro\n\nSee [[Other Note]], [[Other Note|the alias]], \
        [[Other Note#Part]], [[#Intro]], [[#intro|back]] and [[#Missing]].\n";

    #[test]
    fn wikilinks_text_mode_is_the_default_and_drops_brackets() {
        let doc = parse_markdown(WIKI_MD);
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(out.contains("See Other Note, the alias, Other Note > Part, Intro, back and Missing."), "{out}");
        assert!(!out.contains("\\[\\["), "no raw brackets:\n{out}");
    }

    #[test]
    fn wikilinks_bold_mode() {
        let out = render_wiki(WIKI_MD, crate::WikilinkMode::Bold, None);
        assert!(out.contains("#strong[the alias]"), "{out}");
        assert!(out.contains("#strong[Other Note > Part]"), "{out}");
    }

    #[test]
    fn wikilinks_keep_mode_leaves_them_as_typed() {
        let out = render_wiki(WIKI_MD, crate::WikilinkMode::Keep, None);
        assert!(out.contains("\\[\\[Other Note|the alias\\]\\]"), "{out}");
        assert!(out.contains("\\[\\[Other Note\\#Part\\]\\]"), "{out}");
    }

    #[test]
    fn wikilinks_link_mode_links_headings_and_labels_the_target() {
        let out = render_wiki(WIKI_MD, crate::WikilinkMode::Link, None);
        // The heading carries a label, and both same-note links (case-insensitive) point at it.
        let label = out.lines().find(|l| l.starts_with("= Intro <omd-h-")).expect("labelled heading");
        let name = &label["= Intro <".len()..label.len() - 1];
        assert!(out.contains(&format!("#link(<{name}>)[Intro]")), "{out}");
        assert!(out.contains(&format!("#link(<{name}>)[back]")), "{out}");
        // A missing heading and (without a vault) other notes degrade to plain text.
        assert!(out.contains("and Missing."), "{out}");
        assert!(out.contains("See Other Note, the alias,"), "{out}");
        assert!(!out.contains("obsidian://"), "{out}");
    }

    #[test]
    fn wikilinks_link_mode_with_vault_builds_obsidian_urls() {
        let out = render_wiki(WIKI_MD, crate::WikilinkMode::Link, Some("My Vault"));
        assert!(out.contains("#link(\"obsidian://open?vault=My%20Vault&file=Other%20Note\")[Other Note]"), "{out}");
        assert!(out.contains("file=Other%20Note%23Part\")[Other Note > Part]"), "{out}");
        assert!(out.contains("[the alias]"));
    }

    #[test]
    fn wikilinks_only_first_duplicate_heading_is_labelled_and_hoisted_ones_are_not() {
        let md = "---\ntitle: T\nreading-guide: Guide\n---\n\n## Guide\n\nSee [[#Same]].\n\n\
                  ## Same\n\none\n\n## Same\n\ntwo\n";
        let out = render_wiki(md, crate::WikilinkMode::Link, None);
        // One label on the first "Same" heading plus the one link that uses it.
        assert_eq!(out.matches("<omd-h-").count(), 2, "{out}");
        assert_eq!(out.matches("= Same").count(), 2);
        assert_eq!(out.lines().filter(|l| l.starts_with("= Same <")).count(), 1, "{out}");
        // The hoisted section itself is never a link target.
        let md2 = "---\ntitle: T\nreading-guide: Guide\n---\n\n## Guide\n\ntext\n\n## Body\n\n[[#Guide]]\n";
        let out2 = render_wiki(md2, crate::WikilinkMode::Link, None);
        assert!(!out2.contains("#link(<"), "link to a hoisted heading must be plain text:\n{out2}");
    }

    #[test]
    fn wikilinks_in_frontmatter_stay_plain_strings_in_text_mode() {
        let md = "---\ntitle: T\nauthor: \"[[Jane Doe]]\"\n---\n\nbody\n";
        let out = render_wiki(md, crate::WikilinkMode::Text, None);
        assert!(out.contains("#let author = \"Jane Doe\""), "{out}");
    }

    #[test]
    fn wikilinks_in_code_are_left_alone_and_images_still_work() {
        let md = "`[[Not a link]]`\n\n```\n[[Nor this]]\n```\n\n![[pic.png]]\n";
        let out = render_wiki(md, crate::WikilinkMode::Text, None);
        assert!(out.contains("[[Not a link]]") && out.contains("[[Nor this]]"), "{out}");
        assert!(out.contains("pic.png"), "{out}");
    }

    #[test]
    fn table_cells_are_not_justified_but_hyphenate() {
        // Body text uses par(justify: true); tables must scope it off so wrapped
        // cells are left-aligned (ragged) — and must turn hyphenation ON so long
        // words wrap instead of overflowing into the next column.
        let doc = parse_markdown("| A | B |\n|---|---|\n| one two three four | x |\n");
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(
            out.contains("set par(justify: false)"),
            "table must disable paragraph justification:\n{out}"
        );
        assert!(
            out.contains("set text(hyphenate: true)"),
            "table must enable hyphenation so long words wrap:\n{out}"
        );
    }

    #[test]
    fn spaced_wikilink_filenames_parse_as_embeds() {
        // A wikilink whose filename contains spaces must still parse as an embed
        // (the destination is angle-bracket wrapped so the space doesn't
        // terminate the URL). Previously this degraded to literal text.
        let doc = parse_markdown("![[Some Note.md]]\n");
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(
            out.contains("Embed not supported:"),
            "spaced .md embed should warn, not render as text:\n{out}"
        );
        assert_eq!(collect_unsupported_embeds(&doc), vec!["Some Note.md"]);

        // A spaced *image* filename renders as image() with the space preserved.
        let doc2 = parse_markdown("![[My Image.png]]\n");
        let out2 = render_typst(&doc2, None, &RenderOptions::default());
        assert!(
            out2.contains("image(\"My Image.png\")"),
            "spaced image should render as image():\n{out2}"
        );
    }

    #[test]
    fn collect_unsupported_embeds_dedupes_and_finds_nested() {
        // Walks into paragraphs, lists, callouts, etc.; de-duplicates.
        let md = "\
![[a.base]]\n\n\
- item with ![[a.base]] again\n\
- ![[movie.mp4]]\n\n\
> [!note] title\n> ![[doc.pdf]]\n\n\
![real](pic.png)\n";
        let doc = parse_markdown(md);
        let found = collect_unsupported_embeds(&doc);
        assert_eq!(found, vec!["a.base", "movie.mp4", "doc.pdf"], "got: {found:?}");
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

    #[test]
    fn frontmatter_block_scalar_renders_as_content_block() {
        // YAML | block scalar must produce a Typst content block with \ line
        // breaks, not a flat string with the lines joined by spaces.
        let md = "---\nsummary: |\n  First line\n  Second line\n---\n\n# Body\n";
        let doc = parse_markdown(md);
        let out = render_typst(&doc, None, &RenderOptions::default());
        assert!(
            out.contains("#let summary = [First line\\\nSecond line]"),
            "Expected content block with Typst line break:\n{out}"
        );
    }
}
