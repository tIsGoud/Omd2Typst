# Spec 2: Zero-Dependency CLI Binary

**Date:** 2026-05-19
**Status:** Approved for implementation planning

---

## Overview

Replace the `Command::new("typst")` subprocess in `crates/cli` with the `typst` Rust crate compiled directly into the binary. The result is a self-contained CLI that produces PDFs without requiring a separately installed `typst` binary.

This spec applies to `crates/cli` only. The Obsidian plugin (`obsidian-omd2typst`) delegates PDF compilation to the user's installed `typst` binary and is not affected.

---

## Goals

- Zero runtime dependencies for PDF output — one binary, no external tools required
- Identical PDF output to the current subprocess-based path
- Font support: bundled standard Typst fonts + system fonts (Verdana, Arial, DejaVu)
- Linux x86_64 release binary on Codeberg via Forgejo CI; `cargo install` documented for macOS and Windows

---

## Scope

**In scope:**
- `crates/cli/src/world.rs` — new file implementing `typst::World`
- `crates/cli/src/main.rs` — replace subprocess with in-process compilation
- `crates/cli/Cargo.toml` — add `typst`, `typst-pdf`, `typst-assets`, `comemo`
- `.forgejo/workflows/release.yml` — new CI workflow for binary releases

**Out of scope:**
- `crates/core`, `crates/wasm`, `crates/web` — no changes
- Obsidian plugin — no changes
- Package downloading (Typst Universe packages) — not supported
- macOS or Windows pre-built binaries — documented as `cargo install`
- Font embedding beyond typst-assets + system scan

---

## Architecture

The change is contained entirely within `crates/cli`. Nothing in `omd2typst-core` changes.

```
main.rs                          world.rs (new)
  │                                 │
  ├─ read .md                       ├─ library()   → standard Typst stdlib
  ├─ parse_markdown()               ├─ main()      → .typ source (in memory)
  ├─ render_typst()                 ├─ source(id)  → template .typ files (from disk)
  │                                 ├─ file(id)    → images (from disk)
  ├─ build OmdWorld { ... }         └─ font(index) → fonts (embedded + system)
  ├─ typst::compile(&world)
  └─ write .pdf  (or .typ if --typ requested)
```

The intermediate `.typ` file is no longer written to disk for PDF output. It is still written when the user explicitly requests `.typ` output (`omd2typst input.md output.typ`).

---

## Dependencies

### crates/cli/Cargo.toml

```toml
[package]
name = "omd2typst"
version = "0.5.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Convert Obsidian Markdown notes to Typst/PDF via an AST pipeline"

[[bin]]
name = "omd2typst"
path = "src/main.rs"

[dependencies]
omd2typst-core = { path = "../core" }
anyhow = "1.0"
clap = { version = "4.5", features = ["derive"] }
typst = "0.13"
typst-pdf = "0.13"
typst-assets = { version = "0.13", features = ["fonts"] }
comemo = "0.4"
```

`typst-assets` with `features = ["fonts"]` embeds the standard Typst fonts (Latin Modern Math, DejaVu variants, etc.) as `&[u8]` slices at compile time. `comemo` is Typst's memoization library, required by `typst::compile`.

---

## world.rs — OmdWorld Implementation

New file: `crates/cli/src/world.rs`

### Struct

```rust
use std::path::PathBuf;
use typst::foundations::Bytes;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::{Library, World};
use comemo::Prehashed;

pub struct OmdWorld {
    library: Prehashed<Library>,
    book: Prehashed<FontBook>,
    fonts: Vec<FontSlot>,
    root: PathBuf,
    main: Source,
}

struct FontSlot {
    path: Option<PathBuf>,   // None for embedded fonts
    data: Option<Bytes>,     // embedded bytes, or loaded from path on demand
    index: u32,              // face index within the font file
}
```

### Font loading (in `OmdWorld::new`)

Two passes, in order:

1. **Embedded fonts** — iterate `typst_assets::fonts()`, parse each `&[u8]` slice into one or more `Font` faces, push face metadata into the `FontBook`, and store the bytes in the `FontSlot`. The exact API for extracting faces from a byte slice is `Font::new(bytes, index)` iterated until it returns `None`.
2. **System fonts** — walk the directories returned by `system_font_dirs()`, probe each `.ttf`/`.otf`/`.ttc` file by attempting `Font::new` at increasing face indices until it returns `None`, registering each discovered face. Unreadable directories and unparseable files are silently skipped.

> **Implementation note:** Verify the exact `FontBook` mutation API (e.g. `FontBook::push`) against the `typst` 0.13 crate docs during implementation. The public API for font registration is not stabilised across minor versions.

```rust
fn system_font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("C:\\Windows\\Fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Library/Fonts"));
    }
    dirs
}
```

### World trait implementation

```rust
impl World for OmdWorld {
    fn library(&self) -> &Prehashed<Library> { &self.library }
    fn book(&self) -> &Prehashed<FontBook> { &self.book }
    fn main(&self) -> Source { self.main.clone() }

    fn source(&self, id: FileId) -> typst::diag::FileResult<Source> {
        let path = self.root.join(id.vpath().as_rootless_path());
        let text = std::fs::read_to_string(&path)
            .map_err(|_| typst::diag::FileError::NotFound(path))?;
        Ok(Source::new(id, text))
    }

    fn file(&self, id: FileId) -> typst::diag::FileResult<Bytes> {
        let path = self.root.join(id.vpath().as_rootless_path());
        let data = std::fs::read(&path)
            .map_err(|_| typst::diag::FileError::NotFound(path))?;
        Ok(Bytes::from(data))
    }

    fn font(&self, index: usize) -> Option<Font> {
        let slot = self.fonts.get(index)?;
        let bytes = match &slot.data {
            Some(b) => b.clone(),
            None => {
                let data = std::fs::read(slot.path.as_ref()?).ok()?;
                Bytes::from(data)
            }
        };
        Font::new(bytes, slot.index)
    }

    fn today(&self, _offset: Option<i64>) -> Option<typst::foundations::Datetime> {
        None  // Typst renders an empty date field when None is returned
    }
}
```

`source` and `file` resolve paths relative to `self.root` (the vault base or CWD), mirroring the `--root` behaviour of the old subprocess call.

### Construction

```rust
pub fn new(root: PathBuf, typ_source: String) -> Self {
    // ... font loading ...
    let main_id = FileId::new(None, VirtualPath::new("/main.typ"));
    let main = Source::new(main_id, typ_source);
    Self { library: Prehashed::new(Library::default()), book: Prehashed::new(book), fonts, root, main }
}
```

---

## main.rs — Replacing the Subprocess

### Current code (to remove)

```rust
// Approximately:
Command::new("typst")
    .args(["compile", &typ_path, &pdf_path, "--root", &vault_root])
    .status()?;
```

### Replacement

```rust
let world = OmdWorld::new(root.clone(), typ_source_string);
let result = typst::compile(&world);

for warning in &result.warnings {
    eprintln!("warning: {}", warning.message);
}

let document = result.output.map_err(|errors| {
    let msg = errors.iter()
        .map(|e| {
            let file = e.span.id()
                .map(|id| id.vpath().as_str().to_string())
                .unwrap_or_else(|| "?".to_string());
            format!("  {file}: {}", e.message)
        })
        .collect::<Vec<_>>()
        .join("\n");
    anyhow::anyhow!("Typst compilation failed:\n{msg}")
})?;

let pdf_bytes = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())?;
std::fs::write(&pdf_path, pdf_bytes)?;
```

`mod world;` is added at the top of `main.rs`.

---

## Error Handling

| Failure | Behaviour |
|---|---|
| Template `.typ` file not found | `FileError::NotFound` propagates as `anyhow` error with path |
| Typst compile error | Error list formatted with file and message, returned as `anyhow` error |
| Font directory unreadable | Silently skipped; embedded fonts remain available |
| Unparseable font file | Silently skipped |
| No fonts at all | Typst will emit a compile error on first font use |

Warnings from `result.warnings` are printed to stderr but do not abort compilation.

---

## Testing

### Unit tests in world.rs

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_compiles_hello() {
        let world = OmdWorld::new(
            std::env::current_dir().unwrap(),
            "#set page(width: 100pt, height: 100pt)\nHello".to_string(),
        );
        let result = typst::compile(&world);
        assert!(result.output.is_ok(), "{:?}", result.output.err());
    }

    #[test]
    fn system_font_dirs_does_not_panic() {
        let dirs = system_font_dirs();
        assert!(!dirs.is_empty());
    }
}
```

The `world_compiles_hello` test verifies the full pipeline: font loading, World construction, and Typst compilation. If embedded fonts load correctly this test passes on any platform.

### Existing tests

The `#[cfg(test)]` modules in `crates/core` (parser and renderer) are unaffected. Any existing CLI integration test that invokes `typst compile` as a subprocess is updated to use the new in-process path.

**Verification gate:** `cargo test --workspace` must pass before the implementation is considered complete.

---

## Release Pipeline

New file: `.forgejo/workflows/release.yml`

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Build release binary
        run: cargo build --release -p omd2typst

      - name: Strip binary
        run: strip target/release/omd2typst

      - name: Upload release asset
        uses: actions/upload-release-asset@v1
        with:
          upload_url: ${{ github.event.release.upload_url }}
          asset_path: target/release/omd2typst
          asset_name: omd2typst-linux-x86_64
          asset_content_type: application/octet-stream
```

Trigger: tag push matching `v*` (e.g. `v0.5.0`).

> **Implementation note:** Forgejo Actions is largely GitHub Actions–compatible, but the release asset upload step should be verified against available Forgejo actions (e.g. `forgejo/release-action`) during implementation. The `upload_url` event context may differ.

**macOS and Windows:** documented in README as `cargo install omd2typst` (requires Rust toolchain).

---

## Migration Notes

| Area | Change |
|---|---|
| Binary output path | No change — still `target/release/omd2typst` |
| CLI interface | No change — same arguments, same behaviour |
| Template resolution | Same root-relative logic, now in `world.rs` instead of `--root` flag |
| Intermediate `.typ` file | No longer written to disk for PDF output (still written for `.typ` output) |
| `typst` binary in PATH | No longer required |
