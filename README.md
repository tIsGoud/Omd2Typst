# omd2typst

Convert Obsidian Markdown notes to publication-quality PDFs via Typst.

---

## Why this tool exists

[Obsidian](https://obsidian.md) is an excellent writing and knowledge-management tool, but its built-in PDF export is limited: no custom cover pages, no structured headers/footers, no table of contents with numbering, and limited control over typography and layout.

[Typst](https://typst.app) is a modern typesetting system — fast, scriptable, and capable of producing professional documents. It uses its own markup language rather than LaTeX, making it far easier to customise.

**omd2typst** bridges the two: it reads an Obsidian Markdown file and produces either a `.typ` source file or a compiled `.pdf`, using a Typst template to control all visual aspects of the document.

---

## Workflow

```
Obsidian note (.md)
        │
        ▼
  ┌─────────────┐
  │   omd2typst │  parse → intermediate AST → render → compile (in-process)
  └─────────────┘
        │
        ├──► Typst source (.typ)   ← inspect or hand-edit if needed
        │
        └──► PDF document          ← compiled in-process, no external tools
```

### Step by step

1. **Write** your document in Obsidian using standard Markdown with YAML frontmatter.
2. **Run omd2typst** to convert it to a Typst source file (`.typ`) or directly to a PDF.
3. **Customise the template** (optional) to match your house style — cover page, fonts, colours, headers/footers, callout styles, etc.

The intermediate `.typ` output is optional — requesting a `.typ` output file produces a human-readable source that can be inspected or hand-edited before compiling, useful for one-off adjustments. When exporting directly to PDF, compilation happens in-process without writing an intermediate file to disk.

---

## Why this architecture

| Decision | Reason |
|---|---|
| **Intermediate AST** | A clean separation between parsing (comrak) and rendering (Typst) makes it straightforward to add new Markdown features or change the output format independently. |
| **Template-based styling** | All visual decisions live in a single `.typ` file. Users can customise fonts, colours, cover pages and layout without modifying Rust code. |
| **comrak for parsing** | comrak is a fully CommonMark-compliant Rust parser with Obsidian-relevant extensions (tables, strikethrough, math, footnotes). |
| **Typst for output** | Typst produces high-quality PDFs, has native math support, handles fonts and colours well, and its scripting model makes dynamic content (list of figures, conditional pages) simple. |
| **Two-step output** | Emitting `.typ` first allows the user to inspect, tweak, and reuse the intermediate file. Direct PDF output is also supported when inspection is not needed. |

---

## Installation

### Pre-built binary (Linux x86_64)

Download the latest `omd2typst-linux-x86_64` binary from the [Releases page](https://codeberg.org/tisgoud/omd2typst/releases), make it executable, and place it on your PATH:

```bash
chmod +x omd2typst-linux-x86_64
mv omd2typst-linux-x86_64 ~/.local/bin/omd2typst
```

### Build from source (macOS, Windows, Linux)

Requires [Rust](https://rustup.rs) (stable):

```bash
cargo install omd2typst
```

Or clone and build:

```bash
git clone https://codeberg.org/tisgoud/omd2typst
cd omd2typst
cargo build --release
# Binary: target/release/omd2typst
```

---

## Usage

```bash
# Convert to Typst source
omd2typst input.md output.typ

# Convert directly to PDF
omd2typst input.md output.pdf

# Use a custom template
omd2typst input.md output.pdf --template my-template.typ

# Export the built-in template as a starting point for customisation
omd2typst --export-template my-template.typ
```

### YAML frontmatter

The following frontmatter keys have special meaning in the built-in template:

| Key | Description |
|---|---|
| `title` | Document title — shown on the cover page; also offsets heading levels (`##` → `=`). Optional: if omitted, the text of the first `# Heading` is used automatically. |
| `subtitle` | Subtitle shown below the title on the cover page |
| `author` | Author name |
| `date` | Document date |
| `version` | Version string |
| `status` | Status label (e.g. *Concept*, *Final*) |
| `summary` | Abstract shown between the title and metadata on the cover page |
| `figure-list` | Set to `true` to generate a list of figures after the table of contents |
| `revision-table` | Name of the level-2 section that contains the revision history table |
| `approval-table` | Name of the level-2 section that contains the approval table |
| `language` | UI language for template strings. Supported out of the box: `nl` (default), `en`. See *Language support* below. |

All frontmatter keys are also available in the template as a Typst dictionary `fm`, so you can add your own keys and reference them freely with `fm.at("key", default: "")`.

#### Title fallback

The `title` key is optional. When it is absent, omd2typst automatically uses the text of the first level-1 heading (`# …`) as the document title. The heading is then omitted from the body so it does not appear twice.

> **A title is required for correct rendering.** Either set `title` in the frontmatter or open the document with a level-1 heading (`# …`). If both are absent, the heading level offset is not applied and the table of contents will not render correctly.

#### Revision and approval tables

Two optional front-matter keys — `revision-table` and `approval-table` — enable a dedicated page that appears between the cover page and the table of contents. Each key holds the exact name of a level-2 section (`## …`) in the document body. omd2typst extracts the content of that section, places it on the pre-TOC page, and removes the section from the document body entirely. The headings are not numbered and do not appear in the table of contents.

Both keys are independent: you can use either one or both together. When both are present their tables are placed on the same page separated by whitespace.

**Frontmatter:**

```yaml
revision-table: Revision History
approval-table: Approval
```

**Document body** — standard level-2 headings with Markdown tables underneath:

```markdown
## Revision History

| Version | Date       | Author | Description             |
|---------|------------|--------|-------------------------|
| 0.9     | 2026-05-01 | Alice  | Initial draft           |
| 1.0     | 2026-05-16 | Alice  | Review comments applied |

## Approval

| Role     | Name  | Date       | Signature |
|----------|-------|------------|-----------|
| Author   | Alice | 2026-05-16 |           |
| Reviewer | Bob   | 2026-05-16 |           |
```

The section name in the frontmatter must match the heading text exactly (case-sensitive). Any content that is valid under a normal heading — tables, paragraphs, lists — can be used inside these sections.

---

## Templates

A template is a `.typ` file that must export two symbols:

- **`template`** — a show-rule function `(doc) => { … }` that wraps the entire document. It controls page setup, fonts, heading styles, and all other global styling.
- **`callout`** — a function `(kind, title, body)` that renders Obsidian callout blocks.

Export the built-in template as a starting point:

```bash
omd2typst --export-template my-template.typ
```

Then edit it and pass it back with `--template my-template.typ`.

### Language support

Templates contain a `_lang_strings` dictionary that maps language codes to UI strings (table-of-contents label, page counter, figure list header, metadata labels, etc.). The active language is selected with the `language` frontmatter key; when the key is absent the template falls back to its own default language.

**Supported languages** in the built-in templates: `nl` (Dutch, default) and `en` (English).

**Selecting a language:**

```yaml
language: en
```

**Adding a new language** — open the template file and add an entry to `_lang_strings`:

```typst
#let _lang_strings = (
  "nl": ( toc: "Inhoudsopgave", figures: "Lijst van figuren", ... ),
  "en": ( toc: "Table of Contents", figures: "List of Figures", ... ),
  "de": ( toc: "Inhaltsverzeichnis", figures: "Abbildungsverzeichnis", ... ),
)
```

The keys required for each language entry are: `toc`, `figures`, `summary`, `version`, `status`, `date`, `author`, `page`, `of`, `fig_nr`, `fig_desc`. If an unknown code is passed the template silently falls back to Dutch.

**Creating a language-default template** — if you always write in English, change the fallback in the template function:

```typst
let _lang = fm.at("language", default: "en")
```

---

## Font handling

Typst always embeds all fonts used by a document directly into the PDF. The output file is therefore self-contained: recipients do not need to install any font to view or print it correctly.

### Embedded fallback fonts

omd2typst bundles **Liberation Sans** and **Liberation Serif** inside the binary itself. They are always available regardless of what the host system has installed.

These fonts are the **global last-resort fallback** for every document compiled by omd2typst:

- When a preferred font such as Verdana or Arial is present on the system, it is used.
- When none of the fonts in a template's font stack can be found — including external templates whose fonts are not installed — Liberation Sans is used instead of Typst's built-in default of Libertinus Serif.
- A document always uses a sans-serif body font; it never silently falls back to a serif or monospace face.

| Font | Metric-compatible with | Role |
|---|---|---|
| Liberation Sans | Arial / Helvetica | Sans-serif fallback — last resort for any unavailable font |
| Liberation Serif | Times New Roman | Serif fallback for templates that explicitly use it |

### User fonts directory

Place any `.ttf` or `.otf` font files in a `fonts/` directory next to the `omd2typst` binary. These fonts are discovered automatically at startup and are available to any template.

```
my-project/
  omd2typst          ← binary
  fonts/
    CorpSans-Regular.ttf
    CorpSans-Bold.ttf
  document.md
  my-template.typ
```

This layout is convenient for CI/CD pipelines where the binary, input files, and custom fonts travel together as a unit.

### Using Liberation fonts in custom templates

Because Liberation Sans and Liberation Serif are always available, you can reference them directly in any custom template as a reliable fallback at the end of your font stack:

```typst
// Sans-serif template — Liberation Sans guarantees a consistent fallback
set text(font: ("Your Custom Font", "Verdana", "Arial", "Liberation Sans"))

// Serif template — Liberation Serif guarantees a consistent fallback
set text(font: ("Your Serif Font", "Georgia", "Times New Roman", "Liberation Serif"))
```

---

## Supported Markdown / Obsidian features

| Feature | Syntax | Notes |
|---|---|---|
| YAML frontmatter | `---` block | Strings, numbers, booleans, inline arrays |
| Headings | `# H1` … `###### H6` | Level automatically offset when document has a title |
| Bold | `**text**` or `__text__` | |
| Italic | `*text*` or `_text_` | |
| Strikethrough | `~~text~~` | Obsidian native |
| Highlight | `==text==` | Obsidian native |
| Inline code | `` `code` `` | |
| Links | `[text](url)` | |
| Images — standard | `![alt](path)` | |
| Images — with width | `![alt\|200](path)` | Width in points |
| Images — wikilink | `![[path\|200]]` | Obsidian wikilink format |
| Code block | ` ```lang ``` ` | Language tag preserved for syntax highlighting |
| Table | `| col |` | Left / center / right column alignment |
| Callout | `> [!type] Title` | 13 built-in types with Lucide SVG icons in each type's accent colour |
| Block quote | `> text` | Left accent bar with indented text; same colour as the summary box border |
| Bullet list | `- item` | Nested to any depth |
| Ordered list | `1. item` | Nested to any depth |
| Checkbox list | `- [ ]`, `- [x]`, `- [i]` | 10 emoji variants — see table below |
| Superscript | `<sup>text</sup>` | HTML inline; also renders in Obsidian preview |
| Subscript | `<sub>text</sub>` | HTML inline; also renders in Obsidian preview |
| Inline math | `$formula$` | Typst math syntax |
| Display math | `$$formula$$` | Centred display block |
| Footnotes | `[^1]` / `[^1]: text` | Rendered as Typst footnotes (bottom of the page) |
| Thematic break | `---` | Full-width horizontal rule |

### Callout types and icons

Each callout type has a Lucide SVG icon rendered inline before the title, stroked in the type's accent colour. Unknown types show the title without an icon.

| Type | Lucide icon | Accent colour |
|---|---|---|
| `note` / `info` | info (circle + i) | blue `#1d4ed8` |
| `tip` / `hint` | lightbulb | green `#15803d` |
| `important` | circle-alert (circle + !) | green `#15803d` |
| `warning` / `caution` / `attention` | triangle-alert | amber `#a16207` |
| `danger` | flame | red `#b91c1c` |
| `error` | circle-x | red `#b91c1c` |
| `bug` | bug | red `#b91c1c` |
| `quote` / `cite` | quote | slate `#475569` |
| *(unknown type)* | *(none — title only)* | grey `#374151` |

Callouts are non-breakable by default — they will not be split across a page break.

### Checkbox variants

| Syntax | Emoji | Meaning |
|---|---|---|
| `- [ ]` | 🔲 | Empty / to do |
| `- [x]` or `- [X]` | ✅ | Done |
| `- [/]` | 🔄 | In progress |
| `- [-]` | 🚫 | Cancelled |
| `- [>]` | ➡️ | Forwarded |
| `- [!]` | ⚠️ | Important |
| `- [?]` | ❓ | Question |
| `- [i]` | 💡 | Idea |
| `- [I]` | 📖 | Info |
| `- [*]` | ⭐ | Star |

### Block quotes

A standard Markdown block quote is written with a `>` prefix:

```markdown
> "The best way to predict the future is to invent it." — Alan Kay
```

In the PDF the quote is rendered with a thin vertical bar on the left margin and the text indented by 1 em. The bar uses the same colour (`#c5d8f0`) and stroke weight (1 pt) as the border of the summary box on the cover page, giving a consistent visual language across the document.

Block quotes and callouts share the same `>` prefix syntax. omd2typst distinguishes them by the presence of a `[!type]` marker on the first line: a line that starts with `> [!note]` (or any recognised callout type) is treated as a callout; everything else is a plain block quote.

```markdown
> This is a plain block quote.

> [!note] This is a callout
> The [!type] marker makes the difference.
```

---

## Not supported

The following Obsidian and Markdown features are **not** handled. They will either be silently dropped or passed through as plain text.

| Feature | Reason |
|---|---|
| Wikilinks `[[Page Name]]` | Internal note links have no meaning in a single-file PDF export |
| Note embeds `![[Note Name]]` | Embedding other notes would require resolving the full vault at runtime |
| Obsidian comments `%%…%%` | Stripped from output (inline and block) |
| Tags `#tag` | No equivalent concept in a PDF document |
| Mermaid diagrams | Typst has no native Mermaid renderer |
| Obsidian Tasks plugin fields | Due dates, priorities, recurrence — tool-specific syntax not in standard Markdown |
| Dataview queries | Require vault-wide evaluation at Obsidian runtime |
| Canvas files (`.canvas`) | JSON-based spatial format, not Markdown |
| Multi-file documents | Each run converts a single `.md` file |
| Definition lists | Not part of CommonMark; no comrak extension available |
| Subscript `~text~` / Superscript `^text^` | Not native to Obsidian; use `<sub>`/`<sup>` HTML instead |
| LaTeX math | Typst uses its own math syntax; formulas must be written in Typst notation |

---

## Math notation

Math content is passed directly to Typst. Typst uses **its own math notation**, which is similar to but not identical to LaTeX.

```markdown
Inline:  $a^2 + b^2 = c^2$

Display:
$$sum_(k=1)^n k = (n(n+1)) / 2$$
```

Common differences from LaTeX:

| LaTeX | Typst |
|---|---|
| `\frac{a}{b}` | `a/b` or `frac(a, b)` |
| `\sqrt{x}` | `sqrt(x)` |
| `\sum_{i=1}^{n}` | `sum_(i=1)^n` |
| `\alpha`, `\beta` | `alpha`, `beta` |
| `\times` | `times` |

For LaTeX compatibility the [`mitex`](https://typst.app/universe/package/mitex) Typst package can be added to the template, but this is not built in.

---

## Project structure

This repository is a Cargo workspace with four crates:

```
crates/
  core/         — omd2typst-core (library)
    src/
      lib.rs        — public API: parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE
      ast.rs        — Intermediate AST: Block, Inline, Document
      parser.rs     — Markdown → AST via comrak + custom preprocessing
      renderer.rs   — AST → Typst source; built-in preamble and template
  cli/          — omd2typst (binary) — thin shell over omd2typst-core
    src/main.rs     — CLI (clap), file I/O, typst invocation
  wasm/         — omd2typst-wasm (cdylib) — WASM bindings for the Obsidian plugin
    src/lib.rs      — render_to_typst, get_builtin_template via wasm-bindgen
  web/          — omd2typst-web (stub, specced separately)
    src/main.rs     — todo!()
```

The AST is intentionally minimal — only the constructs that have a direct Typst equivalent are represented. Everything else is either preprocessed before parsing (Obsidian wikilink images) or silently ignored.

---

## Platform

omd2typst-core is the shared foundation for four consumers:

| Consumer | How it uses the core |
|---|---|
| **CLI** (`crates/cli`) | Native binary; Typst embedded — no external tools required |
| **Obsidian plugin** ([obsidian-omd2typst](https://codeberg.org/tisgoud/obsidian-omd2typst)) | `crates/wasm` compiled via wasm-pack; PDF via system `typst` CLI |
| **Web service** (`crates/web`) | Stub — specced separately |
| **CI/CD pipelines** | CLI binary |
