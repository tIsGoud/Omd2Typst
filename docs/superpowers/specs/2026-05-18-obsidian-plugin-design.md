# Obsidian Plugin Design: obsidian-omd2typst

**Date:** 2026-05-18
**Status:** Approved for implementation planning

---

## Overview

An official Obsidian community plugin that brings omd2typst functionality directly into Obsidian. Users can export any note to Typst source (`.typ`) or PDF without leaving the editor and without installing any external tools. The plugin is fully self-contained via bundled WASM.

---

## Goals

- Export Obsidian notes to `.typ` or PDF in one action
- Zero external dependencies — WASM-bundled, works on first install
- Desktop only (macOS, Windows, Linux)
- Publishable as an official Obsidian community plugin
- Supports multiple named Typst templates with a configurable default
- Company-specific templates stay in the user's vault, not in the plugin

---

## Architecture

### Approach: Two WASM modules

Two WASM modules, orchestrated by a TypeScript Obsidian plugin:

1. **omd2typst.wasm** — compiled from the Rust codebase via `wasm-pack`. Handles Markdown → Typst source conversion.
2. **typst_compiler.wasm** — from the `typst-ts` ecosystem. Handles Typst source → PDF bytes.

```
User triggers export (active note via palette, or any .md file via right-click)
       │
       ▼
 Read note from Obsidian vault
       │
       ▼
 omd2typst.wasm → Typst source (string, in memory)
       │
       ├──► write .typ   (if .typ output selected)
       │
       └──► typst_compiler.wasm → PDF bytes → write .pdf   (if PDF selected)
```

### Dependency model: git submodule

The plugin lives in a separate public GitHub repository (`obsidian-omd2typst`). The omd2typst Rust repo is included as a git submodule at `libs/omd2typst/`. The submodule is pinned to a specific commit, bumped deliberately when Rust changes are ready to pull in. This keeps the plugin and CLI able to evolve independently — important because callout rendering behaviour may diverge between the two.

### Relation to the broader platform

This plugin is one of four planned consumers of omd2typst-core:

| Consumer | How it uses the core |
|---|---|
| CLI (zero-dependency) | Native binary with embedded Typst |
| **Obsidian plugin** | **omd2typst WASM + typst WASM** |
| Web service | Native Axum server with embedded Typst |
| CI/CD pipelines | CLI binary |

The core library refactor (splitting into `crates/core`, `crates/cli`, `crates/wasm`) is a prerequisite tracked in a separate spec. The built-in template embedded in `renderer.rs` is inherited automatically by all consumers; the plugin's "(Built-in)" option passes `None` as the template source.

---

## Rust Changes (omd2typst repo)

A new `src/lib.rs` is added alongside the existing `main.rs`. It exposes a single WASM-callable function:

```rust
#[wasm_bindgen]
pub fn render_to_typst(markdown: &str, template_src: Option<&str>) -> String {
    let doc = parser::parse_markdown(markdown);
    renderer::render_typst(&doc, template_src)
}
```

`Cargo.toml` gains:
- A `[lib]` target with `crate-type = ["cdylib"]`
- `wasm-bindgen` as a dependency

The CLI binary target (`[[bin]]`) is unchanged.

Callout rendering options (and any other behaviour that diverges between CLI and plugin) are passed as parameters through `render_to_typst` rather than hardcoded, so both consumers can be served from the same codebase without branching.

---

## Plugin Repository Structure

```
obsidian-omd2typst/
├── src/
│   ├── main.ts           — plugin lifecycle, registers commands and context menus
│   ├── settings.ts       — settings data model and settings tab UI
│   ├── exporter.ts       — export pipeline: read note → WASM → write output
│   ├── frontmatter.ts    — frontmatter parse, merge, and insert logic
│   ├── template.ts       — template list management, language declaration parsing
│   ├── output.ts         — output path resolution for all three output location modes
│   └── wasm/
│       ├── omd2typst.ts  — TypeScript wrapper for omd2typst WASM
│       └── typst.ts      — TypeScript wrapper for Typst WASM compiler
├── wasm/
│   ├── omd2typst_bg.wasm    — built from libs/omd2typst via wasm-pack
│   └── typst_compiler.wasm  — from typst-ts npm package
├── libs/
│   └── omd2typst/           — git submodule (pinned commit)
├── scripts/
│   └── build-wasm.sh        — runs wasm-pack inside the submodule
├── manifest.json
├── package.json
└── esbuild.config.mjs
```

---

## Commands

| Command | Command Palette | Right-click (file explorer) |
|---|---|---|
| Export as PDF | ✓ | ✓ |
| Export as Typst source (.typ) | ✓ | ✓ |
| Insert omd2typst frontmatter | ✓ | — |
| Export built-in template | ✓ | — |

**Template selection at export time:** When more than one template is configured and the user triggers export via the command palette, a quick-switcher modal appears with the default template pre-selected. Right-click export always uses the default template silently.

**Export built-in template:** Writes the embedded `.typ` template to a user-specified location in the vault (file picker dialog). Mirrors the CLI's `--export-template` flag. The exported file serves as a customisation starting point; once edited, it can be added to the templates list and used for subsequent exports.

---

## Settings

All settings are stored in Obsidian's `data.json` via the standard plugin settings API.

### Templates

A user-managed list of named templates, each pointing to a `.typ` file inside the vault:

| Field | Description |
|---|---|
| Name | Display name (e.g. "DUO", "Purple", "Report") |
| Path | Vault-relative path to the `.typ` file |
| Languages | Derived from `// omd2typst-languages: nl, en` comment in the file |

The "(Built-in)" template is always available and requires no file. One entry is marked as the **default template**; this is used for right-click exports and as the pre-selected option in the palette modal.

### Default output format

Which format to produce: `Typst source (.typ)` or `PDF`. This is the vault-wide default; the explicit per-action commands ("Export as PDF", "Export as Typst source") always override it regardless of this setting.

### Output location

Three modes, selectable in settings:

| Mode | Behaviour |
|---|---|
| Same folder as note | `notes/report.md` → `notes/report.pdf` |
| Fixed folder | A vault-relative path (e.g. `exports/`), auto-created if absent |
| Ask every time | Obsidian native file save dialog on each export |

### Default language

A dropdown: `English (en)` / `Nederlands (nl)`. Applied when the note's frontmatter has no `language:` key. Language support is defined inside the Typst template, not enforced by the plugin — see Language Validation below.

The built-in embedded template defaults to **English**. When no Typst template is configured (i.e. the "(Built-in)" option is active), the plugin's default language setting determines the language passed to the embedded template. Custom templates declare their own supported languages via the `// omd2typst-languages:` comment and are not affected by this default.

### Default note frontmatter

Configures the YAML block inserted by the "Insert omd2typst frontmatter" command. Two sub-modes:

- **Inline editor** — edit the YAML block directly in plugin settings. Pre-filled with all supported keys; user removes the ones they never use.
- **Template file** — point to a `.md` file in the vault; the plugin reads its frontmatter block as the template. Compatible with Templater and similar plugins.

The full set of supported frontmatter keys:

```yaml
---
title:
subtitle:
author:
date:
version:
status:
language:
summary:
figure-list:
revision-table:
approval-table:
---
```

---

## Frontmatter Insert Behaviour

The "Insert omd2typst frontmatter" command **merges, never overwrites**:

1. Parse the note's existing frontmatter (if any).
2. Identify keys present in the configured template that are missing from the note.
3. Prepend those missing keys above the existing frontmatter keys, preserving all current values.
4. If the note has no frontmatter block, insert the full template at the top of the file.

Keys already present in the note are left untouched.

---

## Language Validation

Language support is declared inside each `.typ` template file via a standard comment:

```typst
// omd2typst-languages: nl, en
```

At export time:
1. Plugin reads the `language:` key from the note's frontmatter (or uses the default language setting).
2. If the chosen template contains an `omd2typst-languages` declaration, the plugin checks whether the note's language is listed.
3. On mismatch: a non-blocking warning notification is shown — *"Template 'DUO' supports nl, en — note language is 'fr'."* The user can proceed or cancel.
4. If no declaration is found in the template: no check is performed.

The templates list in settings shows declared languages next to each entry (or "language: not declared" when absent).

---

## Error Handling

| Situation | Behaviour |
|---|---|
| Template file not found | Warning notification with file path |
| Language mismatch | Non-blocking warning; user chooses to proceed or cancel |
| WASM parse/render error | Error notification with the message from Rust |
| Typst compilation failure | Error notification with Typst error output; intermediate `.typ` preserved for inspection |
| Output folder missing | Auto-created (fixed folder mode); error notification if vault write fails |
| Frontmatter merge: key conflict | Existing key left untouched; skipped keys logged to developer console |

---

## Build Pipeline

### Full build sequence (plugin repo)

```bash
git submodule update --init          # pull omd2typst Rust source
./scripts/build-wasm.sh              # wasm-pack build → wasm/omd2typst_bg.wasm
npm install                          # pulls typst-ts WASM via npm
npm run build                        # esbuild bundles everything → main.js
```

### omd2typst repo — new CI workflows

| Workflow | Trigger | Output |
|---|---|---|
| `release-cli.yml` | Git tag push | Binaries for Linux x86_64, macOS arm64, macOS x86_64, Windows x86_64 |
| `release-wasm.yml` | Git tag push | `omd2typst_bg.wasm` + TS bindings |
| `ci-example.yml` | Reference only | Documents how CI/CD consumers use the binary |

### obsidian-omd2typst repo — release workflow

On tag push: run full build sequence → commit `main.js` + `manifest.json` to release → publish GitHub release. Obsidian's plugin registry picks up the release automatically.

---

## Testing

### omd2typst Rust repo

- Existing unit tests (parser, renderer) are unchanged.
- New: `wasm-pack test --headless` integration tests verify `render_to_typst()` round-trips representative documents.
- New: snapshot tests for callout rendering (guards against unintended divergence between CLI and plugin behaviour).
- Smoke test in `release-cli.yml`: convert a fixture `.md` to PDF before publishing binaries.

### obsidian-omd2typst plugin repo

| Module | Test type | Coverage |
|---|---|---|
| `frontmatter.ts` | Unit (Jest) | Merge logic, inline vs. file template, all frontmatter keys |
| `template.ts` | Unit (Jest) | Language declaration parsing, mismatch detection |
| `output.ts` | Unit (Jest) | Path resolution for all three output location modes |
| `exporter.ts` | Integration | Mock Obsidian vault API, run full export with fixture `.md`, assert `.typ` snapshot |

Obsidian GUI layer (command registration, settings tab rendering) is covered by manual testing — Obsidian's test tooling does not support automated UI tests.

---

## Open Items (deferred to core library spec)

The platform-level sections of this document (multi-consumer architecture diagram, CI/CD integration, release workflows, web service) will be extracted into a dedicated umbrella architecture document when the core library spec is written. That document will be the authoritative reference for the full platform; this spec will then cover only the Obsidian plugin.

Specific items deferred:

- Refactor `omd2typst` into `crates/core` + `crates/cli` + `crates/wasm` workspace structure.
- Embed Typst library into CLI for zero-dependency binary.
- Font loading strategy for embedded Typst (system fonts vs. bundled fallback set).
- Callout rendering options passed as parameters (enables CLI/plugin divergence without forking).
- Web service design (Axum server or Cloudflare Worker).
- Umbrella architecture document covering all four consumers.
