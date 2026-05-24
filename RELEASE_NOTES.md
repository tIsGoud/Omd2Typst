# Release Notes

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
