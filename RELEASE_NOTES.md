# Release Notes

## v0.18.0 — PDF embeds

**PDF pages in the document.** The engine can now include the pages of an
embedded PDF (`![[annex.pdf]]`): every selected PDF page becomes a page of the
document, as a vector image, so an annex keeps the document's page numbering
and the chapter headings in front of it. It is an option of the engine (the
Obsidian plugin has the matching setting); the compiled document needs
**Typst 0.14 or newer**.

- **Modes.** *Fit* puts every PDF page on its own page inside the margins,
  keeping the document's header and footer. *Full* makes every PDF page a full
  page without margins, header, footer or background. *Off* (the default) shows
  the "Embed not supported" placeholder, and *skip* leaves the embed out.
- **Page selection** (Obsidian's `#page=` syntax): `#page=3` is page 3 through
  the last page (in Obsidian it is where the viewer starts), `#page=2-4` is
  pages 2 to 4, and `#page=3,5,7` is exactly those pages (`#page=2-3,7` mixes
  ranges in). A selection the engine cannot read, such as `#page=3-`, keeps the
  placeholder. One embed can include at most 2000 pages.
- **Page counts.** Typst cannot count the pages of a PDF, so the caller supplies
  them (`pdf_pages`). A selection that runs to the last page, or an embed
  without `#page=`, needs the count; closed ranges and lists do not.
- **The CLI does not include PDFs yet**: its embedded Typst is 0.13, so it keeps
  the placeholder. That follows with an upgrade of the embedded Typst.

The WASM `render_to_typst` takes two new optional arguments, `pdf_embeds`
(`off`, `skip`, `fit` or `full`) and `pdf_pages` (one `<count>\t<path>` line per
PDF). The example documents in `input/` embed pages 5, 6 and 7 of
`input/_assets/example.pdf`.

---

## v0.17.1 — Tables, figure captions

**Text below a table.** A text line directly under the last table row (no blank
line between) became an extra table row in the PDF. A table now ends at the
first line without a `|`, as in Obsidian, so such a line is a paragraph.
Fenced code is not touched, and tables in call-outs and quotes keep their `>`
prefix.

**Figure captions.** Captions are smaller (0.85em) and there is more space
below a figure (2em instead of 1.5em). This is in the built-in template and in
all shipped templates (`show figure.caption: set text(size: 0.85em)` and
`show figure: set block(below: 2em)`). Custom templates can add the same two
lines.

---

## v0.17.0 — Image sizes as a share of the text width

**Behaviour change.** An image size such as `![[image.png|400]]` is in pixels in
Obsidian, but the engine used it as points, so images came out much larger in
the PDF: `|400` was about 90% of the text width on A4, where it is about half
the note width in Obsidian. The size is now converted to the same *share* of the
text width: the size divided by a reference width, at most 100%.

With the default reference width of **700** (Obsidian's default readable line
length), `|400` becomes 57%, `|350` becomes 50% and `|700` or more becomes
100%. It does not depend on the template's margins or the paper size, and it
works for every form: `![alt|400](…)`, `![[…|400]]`, `![[…|400|alt text]]` and
sized Excalidraw drawings.

**Setting the reference width.** CLI: `--image-reference-width PX`. Plugin:
*Image width reference (px)* in the export settings. Enter your own Obsidian
line width if you changed *Readable line length* or use a wider editor. `0`
reads the numbers as points again (the old behaviour).

Existing documents with sized images will look smaller than before. The WASM
`render_to_typst` takes a new optional `image_reference_width` argument.

---

## v0.16.0 — Image alt text and size in double bracket embeds

**Obsidian's image options.** `![[image.png|200|alt text]]` now gives both
the width and the alt text; before, the alt text came out as `|200|alt text`
and the width was lost. The options after the path may be in any order: a size
(`200`, or `100x145` where only the width is used) sets the width, anything
else is the alt text. Examples: `![[image.png|alt text]]`,
`![[image.png|alt text|200]]`. The alt text becomes the figure caption, as for
`![alt text|200](image.png)`.

**File names as captions (optional).** New `--filename-captions` option: an
image without alt text is captioned with its file name (`laptop.png` →
"laptop"). Alt text always wins. Off by default. The WASM `render_to_typst`
takes a new optional `filename_captions` argument.

---

## v0.15.0 — LaTeX math

**Math is written in LaTeX by default.** Formulas between `$…$` and `$$…$$`
are now read as LaTeX, the notation Obsidian renders, and converted to Typst
math. A note that looks right in Obsidian now looks right in the PDF, for
example `$\log$`, `$$\frac{a}{b}$$`, `$\sqrt{x}$`, `$\alpha$` and
`$$\sum_{k=1}^n k$$`. Matrices (`pmatrix`, `bmatrix`, `Bmatrix`, `vmatrix`,
`Vmatrix`) and `cases` are supported.

**Behaviour change.** Notes written in Typst math notation (`sum_(k=1)^n`,
`log(x)`) must now be exported with `--math typst` (CLI) or the *Math syntax*
setting (plugin), which passes formulas through unchanged. The choice applies to
the whole document. `input/input-en-typst-math.md` is an example in Typst syntax.

A formula that cannot be converted is passed through as typed, and the CLI
prints a warning. An unknown LaTeX command is not detected and then fails in
Typst as an unknown variable.

The WASM `render_to_typst` takes a new optional `math` argument (`latex` or
`typst`). Conversion uses the `tex2typst-rs` crate.

---

## v0.14.2 — Highlight colours and super-/subscript size

**Highlight colours.** The coloured highlights (`==🔴text==` … `==🟣text==`)
now use a lighter, closer-to-Obsidian palette: red `#f8bcb9`, orange `#ffdfbf`,
yellow `#f4e5b8`, green `#c8f0c9`, blue `#b3ccf5`, purple `#e8cbf6`. The plain
`==text==` colour (`#fdf1a5`) is unchanged.

**Superscript and subscript size.** `<sup>` and `<sub>` text came out tiny in
fonts whose own script-size metrics are small (for example Helvetica Neue, used
by the purple and tig templates). The engine now sets the size explicitly
(0.7em), so the result no longer depends on the font.

---

## v0.14.1 — Coloured highlights

**Coloured highlights.** Obsidian's highlight colours are supported. A coloured
dot right after the opening `==` selects the colour: `==🔴text==`, `==🟠text==`,
`==🟡text==`, `==🟢text==`, `==🔵text==` or `==🟣text==` (red, orange, yellow,
green, blue, purple). The dot is not shown in the output. Plain `==text==` uses
Obsidian's default highlight colour (`#fdf1a5`) instead of Typst's default
yellow, so existing highlights look slightly different. Any other emoji after
`==` stays part of the text.

**Clearer placeholder for unsupported embeds.** The placeholder (and the CLI
warning) no longer says "in the CLI": PDFs, note transclusions and audio/video
embeds are not rendered by the plugin either. Only Obsidian Bases embeds are
rendered by the plugin.

---

## v0.14.0 — Wikilink conversion

**Wikilinks are converted.** `[[Note]]`, `[[Note|alias]]`, `[[Note#Heading]]`,
`[[#Heading]]` and block references (`[[Note#^id]]`) no longer appear as literal
brackets in the output. A new option chooses how they render:

| Mode | Result |
|------|--------|
| `text` (default) | The display text only: the alias, or `Note`, `Heading`, `Note > Heading`. |
| `bold` | The display text in bold. |
| `link` | Real links (see below). |
| `keep` | The link as typed, brackets included. |

CLI: `--wikilinks text|bold|link|keep` and `--obsidian-vault NAME`.

**Link mode.** `[[#Heading]]` becomes an internal link to that heading in the
PDF (case-insensitive, first matching heading wins). Links to other notes become
`obsidian://open?vault=…&file=…` links, but only when a vault name is given
(`--obsidian-vault`; the plugin supplies it automatically). Without a vault name,
and for a heading that does not exist, the display text is shown as plain text.
The vault name is stored in the PDF. Links use the template's accent colour; the
built-in template uses dark navy `#1e3a5f`.

Wikilinks in code and in frontmatter values stay plain text. Only top-level
headings of the document can be link targets.

The WASM `render_to_typst` takes two new optional arguments: `wikilinks` and
`obsidian_vault`.

---

## v0.13.0 — `reading-guide` frontmatter key

**Reading guide page.** A new `reading-guide` frontmatter key names an H2 section
(for example `reading-guide: Leeswijzer`). The engine takes that section out of
the numbered body, like `revision-table` and `approval-table`, and hands it to
the template as `fm.at("reading-guide-blocks")`. A template can render it as an
unnumbered page of its own. `duo-template.typ` and `duo-template-no-ribbon.typ`
show it right after the revision/approval page, before the table of contents.

```yaml
reading-guide: Leeswijzer
```

The value is the plain heading text, without `##`. A missing or empty value
changes nothing. A name that matches no `## ` section leaves the chapter in the
body, and the CLI prints a stderr warning.

Templates that do not render `reading-guide-blocks` (and the built-in template)
drop the hoisted section from the output, the same as for the revision and
approval sections — only use the key with a template that supports it.

The `appendix-from`, `numbering-from` and `reading-guide` keys are now listed in
`templates/omd2typst-frontmatter.md`.

---

## v0.12.0 — `numbering-from` frontmatter key

**Unnumbered front matter chapters.** A new `numbering-from` frontmatter key
names the first chapter that should be numbered. Top-level chapters before it
(for example a reading guide) render without a number but stay in the table of
contents; numbering restarts at 1 on the named chapter. An empty or missing
value numbers from the beginning, as before.

```yaml
numbering-from: Introduction
```

The value must match a top-level chapter. A name that matches nothing (or only a
subheading) changes nothing, and the CLI prints a stderr warning. The key works
with every template, including the built-in one, and combines with
`appendix-from`. From the named chapter on, headings use the standard `1.1.`
numbering scheme that all shipped templates use.

The key is also listed in `templates/omd2typst-frontmatter.md`.

---

## v0.11.0 — Appendix headings, table layout & embed handling

**Appendix headings.** A new `appendix-from` frontmatter key names the first
appendix chapter; that chapter and every top-level chapter after it render as
`Appendix A - <title>` with letter-prefixed sub-numbering (`A.1`, `A.1.1`).
The prefix is localized — `Appendix` (en), `Bijlage` (nl), `Anhang` (de),
`Apéndice` (es), `Appendice` (fr). Appendix-aware public templates:
`tig-template.typ` and `purple-template.typ`. Templates that do not opt in — and
the built-in template — silently ignore `appendix-from` (no compile error). The
CLI prints a stderr warning when `appendix-from` names no top-level chapter.

**Non-image embeds no longer break PDF generation.** Embeds that are not
renderable images — Obsidian Bases (`.base`), PDFs, note transclusions,
audio/video — previously failed Typst compilation with `unknown image format`.
They now render a visible placeholder note, and the CLI prints a stderr warning,
so the PDF is still produced. (These embeds are fully rendered only by the
Obsidian plugin.)

**Wikilink filenames with spaces now resolve.** `![[Some Note.md]]` and image
embeds whose path contains spaces or parentheses are angle-bracketed before
parsing, so they are recognized instead of degrading to literal text.

**Table layout.** Table cells are no longer justified — they render ragged-left —
and hyphenation is enabled inside tables so a long unbreakable word wraps within
its column instead of overflowing into the neighbouring one.

**Chapters start on a new page.** In `tig-template.typ` and
`purple-template.typ`, each top-level chapter (including appendices) begins on a
fresh page.

**Versioning.** All workspace crates unified at `0.11.0` (cli, core, wasm,
pdf-wasm, web).

---

## v0.10.7 — Figure spacing

Figures now set `show figure: set block(below: 1.5em)` for consistent spacing
below each figure/caption.

---

## v0.10.6 — Add `--version` / `-V` flag to CLI

`omd2typst --version` and `omd2typst -V` now print the version string.

---

## v0.10.5 — Fix clippy lint in block scalar parser

`map_or(false, …)` replaced with `is_some_and(…)` in the YAML block scalar
chomping loop — flagged by `clippy::unnecessary_map_or` with `-D warnings`.

---

## v0.10.4 — Accept `|-` and `|+` chomping variants for block scalars

Obsidian's YAML editor automatically writes `|-` (strip chomping) when the
user saves a `|` block scalar. The parser now accepts `|`, `|-`, and `|+`
and treats all three identically.

---

## v0.10.3 — Support YAML `|` block scalar for multi-line frontmatter

Frontmatter fields now accept the YAML literal block scalar syntax (`|`).
Multi-line values — most usefully `summary` — render as separate lines in
the PDF output instead of being collapsed to a flat string.

---

## v0.10.2 — Figure list in all templates; examples download

- **Figure list page** added to `purple-template.typ` and `tig-template.typ` — set `figure-list: true` in frontmatter to include a numbered list of figures before the document body
- **`omd2typst-examples.zip`** added as a release asset — contains `input/` (sample Markdown files for all five languages) and `templates/` (public Typst templates and frontmatter template), giving new users a ready-to-run starting point
- Clippy fix in `crates/pdf-wasm`: removed bare `use serde_json;` import

---

## v0.10.1 — CLI version bump

Version bump to match release tag. No functional changes.

---

## v0.10.0 — PDF compiler WASM for on-demand download

**New: `omd2typst-pdf-compiler.wasm` release artifact**

A new `crates/pdf-wasm` crate adds a `render_to_pdf()` WASM export that compiles Typst source directly to PDF bytes, without requiring a system `typst` binary. The compiled WASM (~27 MB) is published as `omd2typst-pdf-compiler.wasm` in this release and is consumed by the Obsidian plugin as a fallback PDF compiler.

- Embedded fonts: Liberation Sans/Serif (4 faces each) + typst-assets (math, serif, monospace)
- Template files passed in via a JSON map `{ "vault-relative/path.typ": "content" }`
- `typst_version()` export returns the embedded Typst version string ("0.13.1")
- No system fonts scanned; no filesystem access at runtime

**No changes to the CLI binary.**

---

## v0.9.0 — License corrected to AGPL-3.0

License corrected from MIT to AGPL-3.0-only across all manifests and the LICENSE file. No functional changes.

---

## v0.8.0 — Sample templates, input examples, AGPL-3.0 license

### What changed

**Sample templates added**
Two public Typst templates are now included in the repository under `templates/`:
- `purple-template.typ` — Obsidian-branded purple accent template
- `tig-template.typ` — tiG-branded orange gradient template

Both templates support all five languages (`nl`, `en`, `de`, `es`, `fr`) and have fully embedded SVG logos.

**Input examples added**
Example input documents in all five supported languages are available under `input/`.

**License**
The project is now published under the GNU Affero General Public License v3.0 (AGPL-3.0-only).

---

## v0.7.5 — Cover page, de/es/fr language support, example documents

### What changed

**The built-in template (used when no `--template` flag is given) now renders a structured cover page.**

When your document has frontmatter, the cover page is populated automatically:

| Field | Displayed as |
|---|---|
| `title` | Large heading (28 pt, dark blue) |
| `subtitle` | Sub-heading below the title |
| `summary` | Left-bordered block below the dividing line |
| `version` | Metadata grid, bottom-left |
| `author` | Metadata grid, bottom-right |
| `date` | Metadata grid, bottom-left |
| `status` | Metadata grid, bottom-right |

Only non-empty fields are shown — missing fields leave no blank space. When no `title` is provided a placeholder message points to the README frontmatter section.

Content pages (page 3 onwards) now carry a header with the document title and a centred page-number footer. The cover page (page 1) and table of contents (page 2) have no header or footer.

**Exported templates updated**

The `template` function exported by `--export-template` gains an `fm` parameter (`template(fm: (:), doc)`). This matches the calling convention already used by the bundled templates (`#show: template.with(fm: fm)`). Existing customised templates that declare `template(doc)` continue to work — the new parameter has a safe default.

**Bug fix: empty frontmatter no longer crashes**

Documents with no frontmatter previously generated `#let fm = ()` (an empty Typst array). Calling `.at("key", default: …)` on an array expects an integer index, not a string key, so any template that accessed `fm` would fail with *"expected integer, found string"*. The renderer now emits `#let fm = (:)` (an empty Typst dictionary) in this case.

**German, Spanish, and French added to all bundled templates**

All five bundled templates (`template-duo`, `template-duo-ribbon`, `template-purple`, `template-ro`, `template-vnet`) now include language strings for `de`, `es`, and `fr` in addition to the existing `nl` and `en` entries. Select a language with the `language` frontmatter key:

```yaml
language: de   # Deutsch
language: es   # Español
language: fr   # Français
```

**Example documents published**

The `input/` directory now contains five example documents (Dutch, English, German, Spanish, French) that exercise the full feature set: cover page, table of contents, revision and approval tables, images, callouts, tables, lists, and a figure list.

---

## v0.7.4 — README improvements

Minor typographic improvements to the README.

---

## v0.7.3 — Strip Obsidian comments

### What changed

**`%%…%%` comment spans are now stripped from the output.**

Both inline and block forms are handled:

```markdown
Some text %%this is a comment%% more text

%%
This entire block is a comment.
It will not appear in the PDF.
%%
```

Content inside fenced code blocks (```` ``` ```` or `~~~`) is preserved unchanged. Previously these spans were passed through as plain text.

---

## v0.7.2 — SVG icons for checkboxes

### What changed

**Checkbox items now display inline Lucide SVG icons instead of emoji.**

Each checkbox type renders a coloured SVG glyph — no emoji font required.

| Syntax | Icon | Colour |
|---|---|---|
| `- [ ]` | square outline | grey `#9ca3af` |
| `- [x]` / `- [X]` | check-square | green `#16a34a` |
| `- [/]` | clock | blue `#2563eb` |
| `- [-]` | x-circle | grey `#6b7280` |
| `- [>]` | arrow-right | blue `#2563eb` |
| `- [!]` | alert-circle (circle + !) | red `#b91c1c` |
| `- [?]` | help-circle | purple `#7c3aed` |
| `- [i]` | info-circle (circle + i) | blue `#2563eb` |
| `- [I]` | lightbulb | amber `#d97706` |
| `- [*]` | star | amber `#d97706` |

Icons are embedded as SVG bytes in the Typst preamble alongside the existing callout icons — no external files, fonts, or network access required.

---

## v0.7.0 — Lucide SVG icons in callout blocks

### What changed

**Callout titles now display inline Lucide SVG icons instead of emoji prefixes.**

Each of the 13 built-in callout types has a dedicated icon from the [Lucide](https://lucide.dev) icon set, rendered inline before the title text. The icon is stroked in the type's accent colour, matching the coloured background of the callout block itself.

| Type | Icon | Accent colour |
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

Icons are embedded as SVG bytes directly in the Typst preamble — no external files or network access required. They scale with the title text and are always available regardless of what fonts or images are installed on the host system.

**Custom template users:** the `callout(kind, title, body)` function signature is unchanged. The only visible difference is that `title` no longer carries an emoji prefix — it is now the bare title string. Custom templates that render their own icons or no icons are unaffected.

---

## v0.6.1 — Liberation Sans as true global font fallback

### What changed

**Typst's internal fallback chain now routes through Liberation Sans**

v0.6.0 embedded Liberation Sans in the binary and placed it at the end of every built-in font stack. That fixed the case where a built-in template's preferred fonts were unavailable. But there was a remaining gap: when an **external template** specified fonts that were all unavailable, the output still used Libertinus Serif (a serif face) — because Typst has a hardcoded internal fallback list that is appended to every font stack, and `libertinus serif` appears first on that list.

v0.6.1 patches Typst's internal fallback list to insert `liberation sans` **before** `libertinus serif`. Because Liberation Sans is always embedded in the binary, it is always found. The result:

| Scenario | Before v0.6.1 | After v0.6.1 |
|---|---|---|
| Built-in template, preferred font missing | Liberation Sans ✓ | Liberation Sans ✓ |
| External template, all specified fonts missing | Libertinus Serif ✗ | Liberation Sans ✓ |
| No template, no font set | Libertinus Serif ✗ | Liberation Sans ✓ |

**Library-level default font**

The embedded Typst compiler's default text font (used when no `set text(font: …)` rule is present anywhere) is now set to Liberation Sans instead of the Typst built-in default of Libertinus Serif. Documents with no explicit font setting get a sans-serif output rather than a serif one.

---

## v0.6.0 — Font consistency across all platforms

### What changed

**Guaranteed font fallbacks embedded in the binary**

omd2typst now embeds **Liberation Sans** and **Liberation Serif** directly inside the binary. These open-source fonts (SIL Open Font License 1.1) serve as style-correct fallbacks in every built-in template and preamble.

Before this release, the font stack `("Verdana", "Arial", "DejaVu Sans")` could silently fall back to a serif or monospace face on systems where none of those fonts were installed — for example, bare Linux CI runners. The resulting PDF used a completely different typeface than intended, without any warning to the user.

With v0.6.0, Liberation Sans is always available. The fallback chain now ends on a guaranteed sans-serif face on every platform.

**Font stack positions updated**

All built-in templates and the default preamble have been updated so that Liberation Sans appears at the **end** of the font stack, not the front. Preferred system fonts (Verdana, Arial, Helvetica Neue) still take priority when available. Liberation Sans activates only as a last resort.

| Template | Font stack |
|---|---|
| Built-in preamble | `("Verdana", "Arial", "Liberation Sans")` |
| template-duo | `("Verdana", "Arial", "Liberation Sans")` |
| template-duo-ribbon | `("Verdana", "Arial", "Liberation Sans")` |
| template-ro | `("Verdana", "Arial", "Helvetica Neue", "Liberation Sans")` |
| template-purple | `("Helvetica Neue", "Arial", "Liberation Sans")` |

**User fonts directory**

A `fonts/` directory placed next to the `omd2typst` binary is now scanned automatically at startup. Drop any `.ttf` or `.otf` file there to make it available to all templates — no system installation required. This is especially useful for CI/CD pipelines where the binary, fonts, and input files travel together.

**Binary size**

Embedding 8 Liberation font faces (Sans Regular/Bold/Italic/BoldItalic + Serif Regular/Bold/Italic/BoldItalic) adds approximately 3 MB to the binary. The release binary remains well below the pre-v0.5.0 baseline of 39 MB.

---

### How fonts work in omd2typst PDFs

Typst always embeds every font used by a document directly into the output PDF. The PDF is self-contained: no font needs to be installed on the reader's system.

The font stack in a template is a priority list. Typst tries each family left to right and uses the first one it finds. The Liberation fonts being embedded in the binary means they are always found — making them a reliable last resort that preserves the intended font style (sans-serif or serif) even when no other font in the list is available.

---

### Advice for custom templates

To benefit from the guaranteed fallbacks in your own template, place `"Liberation Sans"` or `"Liberation Serif"` at the **end** of your font stack:

```typst
// Sans-serif body text — consistent on every platform
set text(font: ("Your Corporate Font", "Verdana", "Arial", "Liberation Sans"))

// Serif body text — consistent on every platform
set text(font: ("Your Serif Font", "Georgia", "Times New Roman", "Liberation Serif"))
```

If your preferred font is installed and available, it will be used and embedded in the PDF. If it is absent — on a CI runner, a colleague's machine, or any other environment — Liberation preserves the correct font style automatically.

---

## v0.5.1 — Font directory fix for macOS

Added `/System/Library/Fonts/Supplemental/` to the font search path, resolving warnings about unknown font families Arial and Verdana on macOS.

## v0.5.0 — Zero-dependency PDF generation

Replaced the external `typst` CLI subprocess with an embedded Typst compiler. omd2typst now produces PDFs without any external tools. Binary size reduced from 39 MB to 23 MB via LTO and size optimisation in the release profile.
