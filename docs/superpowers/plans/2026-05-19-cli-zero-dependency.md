# CLI Zero-Dependency Binary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `typst compile` subprocess in `crates/cli` with the `typst` Rust crate embedded directly in the binary, producing PDFs without requiring a separately installed `typst` binary.

**Architecture:** A new `crates/cli/src/world.rs` implements the `typst::World` trait, providing Typst with fonts (bundled via `typst-assets` + system scan) and file access (template `.typ` files and images from disk). The PDF branch in `main.rs` is replaced with `typst::compile(&world)` + `typst_pdf::pdf(&document, ...)`. The CLI interface (all arguments, all output paths, all error messages) is unchanged.

**Tech Stack:** Rust, `typst` crate 0.13, `typst-pdf` crate 0.13, `typst-assets` crate 0.13 (with `fonts` feature), `comemo` 0.4.

---

## File Map

| File | Action | Purpose |
|---|---|---|
| `crates/cli/Cargo.toml` | Modify | Add `typst`, `typst-pdf`, `typst-assets`, `comemo`; bump version to 0.5.0 |
| `crates/cli/src/world.rs` | Create | `OmdWorld` struct implementing `typst::World`; font loading; file resolution |
| `crates/cli/src/main.rs` | Modify | Add `mod world`; replace PDF subprocess (lines 110–128) with in-process compile; remove now-unused helpers |

---

## Task 1: Add Typst dependencies

**Files:**
- Modify: `crates/cli/Cargo.toml`

- [ ] **Step 1: Open `crates/cli/Cargo.toml` and replace its contents**

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

- [ ] **Step 2: Verify the crate still compiles (no logic changed yet)**

Run: `cargo build -p omd2typst`

Expected: compiles successfully. The `typst` crates are large — first build will take a minute.

- [ ] **Step 3: Verify existing workspace tests still pass**

Run: `cargo test --workspace`

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/cli/Cargo.toml
git commit -m "chore(cli): add typst, typst-pdf, typst-assets, comemo dependencies"
```

---

## Task 2: Create `world.rs` — OmdWorld implementing `typst::World`

**Files:**
- Create: `crates/cli/src/world.rs`

### Background

The `typst::World` trait is how the Typst compiler requests resources during compilation. Our implementation:
- Holds the `.typ` source in memory (no intermediate file written to disk)
- Serves template and image files from disk relative to `root` (the working directory)
- Provides fonts: bundled ones from `typst-assets` are embedded as `&[u8]` at compile time; system fonts (Verdana, Arial, DejaVu on disk) are discovered at startup and loaded lazily

The `typst::compile()` call returns `Warned<SourceResult<Document>>`:
- `.output` is `Result<Document, EcoVec<SourceDiagnostic>>` — compilation result
- `.warnings` is `EcoVec<SourceDiagnostic>` — non-fatal warnings

- [ ] **Step 1: Write the failing tests first**

Create `crates/cli/src/world.rs` with only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_compiles_minimal_document() {
        let world = OmdWorld::new(
            std::env::current_dir().unwrap(),
            "#set page(width: 100pt, height: 100pt)\nHello".to_string(),
        );
        let result = typst::compile(&world);
        assert!(
            result.output.is_ok(),
            "Typst compilation failed: {:?}",
            result.output.err()
        );
    }

    #[test]
    fn system_font_dirs_is_non_empty() {
        let dirs = system_font_dirs();
        assert!(!dirs.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify the tests fail (struct not defined yet)**

Run: `cargo test -p omd2typst 2>&1 | head -20`

Expected: compile error — `OmdWorld`, `system_font_dirs` not found.

- [ ] **Step 3: Implement the full `world.rs`**

Replace the entire file with:

```rust
use std::path::PathBuf;
use comemo::Prehashed;
use typst::foundations::Bytes;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::{Library, World};

pub struct OmdWorld {
    library: Prehashed<Library>,
    book: Prehashed<FontBook>,
    fonts: Vec<FontSlot>,
    root: PathBuf,
    main: Source,
}

struct FontSlot {
    path: Option<PathBuf>,  // None for embedded fonts (data always present)
    data: Option<Bytes>,    // Some for embedded fonts; None for system fonts (load from path)
    index: u32,             // Face index within the font file
}

impl OmdWorld {
    pub fn new(root: PathBuf, typ_source: String) -> Self {
        let mut book = FontBook::new();
        let mut fonts: Vec<FontSlot> = Vec::new();

        // Pass 1: embedded fonts from typst-assets (compiled into the binary)
        for data in typst_assets::fonts() {
            let bytes = Bytes::from_static(data);
            let mut index = 0u32;
            while let Some(font) = Font::new(bytes.clone(), index) {
                book.push(font.info().clone());
                fonts.push(FontSlot { path: None, data: Some(bytes.clone()), index });
                index += 1;
            }
        }

        // Pass 2: system fonts (Verdana, Arial, DejaVu, etc.)
        for dir in system_font_dirs() {
            scan_font_dir(&dir, &mut book, &mut fonts);
        }

        let main_id = FileId::new(None, VirtualPath::new("/main.typ"));
        let main = Source::new(main_id, typ_source);

        Self {
            library: Prehashed::new(Library::default()),
            book: Prehashed::new(book),
            fonts,
            root,
            main,
        }
    }
}

impl World for OmdWorld {
    fn library(&self) -> &Prehashed<Library> {
        &self.library
    }

    fn book(&self) -> &Prehashed<FontBook> {
        &self.book
    }

    fn main(&self) -> Source {
        self.main.clone()
    }

    fn source(&self, id: FileId) -> typst::diag::FileResult<Source> {
        let path = self.root.join(id.vpath().as_rootless_path());
        let text = std::fs::read_to_string(&path)
            .map_err(|_| typst::diag::FileError::NotFound(path.into()))?;
        Ok(Source::new(id, text))
    }

    fn file(&self, id: FileId) -> typst::diag::FileResult<Bytes> {
        let path = self.root.join(id.vpath().as_rootless_path());
        let data = std::fs::read(&path)
            .map_err(|_| typst::diag::FileError::NotFound(path.into()))?;
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
        // Returning None causes Typst to leave date fields blank — acceptable default.
        None
    }
}

pub fn system_font_dirs() -> Vec<PathBuf> {
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

fn scan_font_dir(dir: &PathBuf, book: &mut FontBook, fonts: &mut Vec<FontSlot>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_font_dir(&path, book, fonts);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ttf" | "otf" | "ttc" | "otc")
        ) {
            let Ok(data) = std::fs::read(&path) else { continue };
            let bytes = Bytes::from(data);
            let mut index = 0u32;
            while let Some(font) = Font::new(bytes.clone(), index) {
                book.push(font.info().clone());
                fonts.push(FontSlot { path: Some(path.clone()), data: None, index });
                index += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_compiles_minimal_document() {
        let world = OmdWorld::new(
            std::env::current_dir().unwrap(),
            "#set page(width: 100pt, height: 100pt)\nHello".to_string(),
        );
        let result = typst::compile(&world);
        assert!(
            result.output.is_ok(),
            "Typst compilation failed: {:?}",
            result.output.err()
        );
    }

    #[test]
    fn system_font_dirs_is_non_empty() {
        let dirs = system_font_dirs();
        assert!(!dirs.is_empty());
    }
}
```

> **API note:** If `Font::new(bytes, index)` or `FontBook::push` don't compile, check the `typst` 0.13 crate docs — font registration APIs have changed across minor versions. The pattern above (iterate faces by index until `None`) is the correct approach regardless of the exact method names.

- [ ] **Step 4: Run tests and verify they pass**

Run: `cargo test -p omd2typst`

Expected: 2 tests pass. The `world_compiles_minimal_document` test exercises the full pipeline: font loading, World construction, and Typst compilation with embedded fonts.

If `world_compiles_minimal_document` fails with a font error, verify that `typst-assets` was added correctly in Cargo.toml (Task 1).

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/world.rs
git commit -m "feat(cli): add OmdWorld implementing typst::World with embedded + system fonts"
```

---

## Task 3: Replace the subprocess in `main.rs`

**Files:**
- Modify: `crates/cli/src/main.rs`

### Background

The current PDF branch (lines 110–128 in `main.rs`) writes an intermediate `.typ` file to disk and shells out to `typst compile`. We replace this with in-process compilation using `OmdWorld`.

The `resolve_template_path` helper was needed because the old approach embedded the template as a relative `#import` from the intermediate `.typ` file's directory. The new approach resolves all paths relative to CWD via `OmdWorld::root`, so the user-provided `--template` path (already relative to CWD) can be passed directly to `render_typst` without conversion.

Three helpers become dead code after this change and must be removed:
- `intermediate_typ_path` (lines 140–145)
- `current_timestamp` (lines 148–175)
- `resolve_template_path` (lines 200–224)

The `use std::process::Command` import (line 6) also becomes unused.

**The CLI interface does not change.** All arguments (`input`, `output`, `--format`, `--template`, `--export-template`) behave identically from the user's perspective.

- [ ] **Step 1: Replace `main.rs` with the updated version**

Write the complete file — it is shorter than the original because three helpers are removed:

```rust
mod world;

use omd2typst_core::{parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::fs;

#[derive(Parser)]
#[command(
    name = "omd2typst",
    about = "Convert Obsidian Markdown to a Typst file or PDF",
    after_help = "EXAMPLES:
    omd2typst notes.md output.typ
    omd2typst notes.md output.pdf
    omd2typst notes.md output.pdf --template my-template.typ
    omd2typst --export-template my-template.typ"
)]
struct Cli {
    /// Input Markdown file
    #[arg(required_unless_present = "export_template")]
    input: Option<String>,

    /// Output file (.typ or .pdf)
    #[arg(required_unless_present = "export_template")]
    output: Option<String>,

    /// Output format — inferred from the output file extension if omitted
    #[arg(short, long)]
    format: Option<Format>,

    /// Typst template file for styling. Must export `template` and `callout`.
    /// Use --export-template to get a starting point.
    #[arg(short, long)]
    template: Option<String>,

    /// Write the built-in styling to FILE as a Typst template and exit.
    /// Edit the file, then pass it back with --template.
    #[arg(long, value_name = "FILE")]
    export_template: Option<String>,
}

#[derive(ValueEnum, Clone)]
enum Format {
    Typst,
    Pdf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // --export-template writes the built-in template to disk and exits.
    if let Some(path) = &cli.export_template {
        fs::write(path, BUILTIN_TEMPLATE)
            .with_context(|| format!("Cannot write template to: {}", path))?;
        println!("Template written to: {}", path);
        println!("Edit it and use it with:  omd2typst input.md output.pdf --template {}", path);
        return Ok(());
    }

    let input_path = cli.input.as_deref()
        .ok_or_else(|| anyhow::anyhow!("input file is required"))?;
    let requested_output = cli.output.as_deref()
        .ok_or_else(|| anyhow::anyhow!("output file is required"))?;

    let format = cli.format.clone().unwrap_or_else(|| {
        if requested_output.ends_with(".pdf") { Format::Pdf } else { Format::Typst }
    });

    // For .typ output: detect collision with the template and rename with a warning.
    let output_path: String = match &format {
        Format::Typst => {
            if let Some(tmpl) = cli.template.as_deref() {
                if paths_are_same(requested_output, tmpl) {
                    let renamed = prefix_filename(requested_output, "output-");
                    eprintln!(
                        "Warning: output file '{}' has the same name as the template '{}'. \
                         A Typst file cannot import itself. \
                         Output renamed to '{}'.",
                        requested_output, tmpl, renamed
                    );
                    renamed
                } else {
                    requested_output.to_string()
                }
            } else {
                requested_output.to_string()
            }
        }
        Format::Pdf => requested_output.to_string(),
    };

    let input = fs::read_to_string(input_path)
        .with_context(|| format!("Cannot read input file: {}", input_path))?;

    let doc = parse_markdown(&input);
    // Template path is passed as-is (relative to CWD); OmdWorld resolves it against root.
    let typst_src = render_typst(&doc, cli.template.as_deref(), &RenderOptions::default());

    match format {
        Format::Typst => {
            fs::write(&output_path, &typst_src)
                .with_context(|| format!("Cannot write: {}", output_path))?;
        }
        Format::Pdf => {
            let root = std::env::current_dir()
                .context("Cannot determine current directory")?;
            let world = world::OmdWorld::new(root, typst_src);
            let result = typst::compile(&world);

            for warning in &result.warnings {
                eprintln!("warning: {}", warning.message);
            }

            let document = result.output.map_err(|errors| {
                let msg = errors
                    .iter()
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

            let pdf_bytes = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
                .map_err(|errors| {
                    let msg = errors
                        .iter()
                        .map(|e| format!("  {}", e.message))
                        .collect::<Vec<_>>()
                        .join("\n");
                    anyhow::anyhow!("PDF generation failed:\n{msg}")
                })?;

            fs::write(&output_path, pdf_bytes)
                .with_context(|| format!("Cannot write: {}", output_path))?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns true if `a` and `b` resolve to the same filesystem path.
fn paths_are_same(a: &str, b: &str) -> bool {
    use std::path::Path;
    let abs = |p: &str| {
        Path::new(p).canonicalize().ok()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().join(p))
    };
    abs(a) == abs(b)
}

/// Prepends `prefix` to the filename component of `path`.
/// E.g. prefix_filename("dir/foo.typ", "output-") → "dir/output-foo.typ"
fn prefix_filename(path: &str, prefix: &str) -> String {
    use std::path::Path;
    let p = Path::new(path);
    let filename = p.file_name().and_then(|n| n.to_str()).unwrap_or("output.typ");
    p.with_file_name(format!("{}{}", prefix, filename))
        .to_string_lossy()
        .into_owned()
}
```

> **API note:** `typst_pdf::pdf()` returns `Result<Vec<u8>, EcoVec<SourceDiagnostic>>` in Typst 0.13. If the return type differs, adjust the error handling accordingly — the important thing is writing `pdf_bytes` to `output_path`.

- [ ] **Step 2: Verify the whole workspace compiles and tests pass**

Run: `cargo test --workspace`

Expected: all tests pass (includes the 2 world tests from Task 2).

- [ ] **Step 3: Smoke-test PDF output (manual)**

With a test `.md` file and the built-in template:

```bash
cargo build --release -p omd2typst
echo "# Hello\n\nThis is a test." > /tmp/test.md
./target/release/omd2typst /tmp/test.md /tmp/test.pdf
```

Expected: `/tmp/test.pdf` is created, opens in a PDF viewer, contains the text "Hello".

- [ ] **Step 4: Verify CLI interface is unchanged**

```bash
./target/release/omd2typst --help
```

Expected output (identical to before):

```
Convert Obsidian Markdown to a Typst file or PDF

Usage: omd2typst [OPTIONS] [INPUT] [OUTPUT]

Arguments:
  [INPUT]   Input Markdown file
  [OUTPUT]  Output file (.typ or .pdf)

Options:
  -f, --format <FORMAT>              Output format — inferred from the output file extension if omitted [possible values: typst, pdf]
  -t, --template <TEMPLATE>          Typst template file for styling. Must export `template` and `callout`. Use --export-template to get a starting point.
      --export-template <FILE>       Write the built-in styling to FILE as a Typst template and exit. Edit the file, then pass it back with --template.
  -h, --help                         Print help

EXAMPLES:
    omd2typst notes.md output.typ
    omd2typst notes.md output.pdf
    omd2typst notes.md output.pdf --template my-template.typ
    omd2typst --export-template my-template.typ
```

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): replace typst subprocess with in-process compilation via typst crate"
```

---

## Task 4: Add Forgejo release CI workflow

**Files:**
- Create: `.forgejo/workflows/release.yml`

### Background

Codeberg uses Forgejo, which runs Forgejo Actions — a fork of GitHub Actions. Most GitHub Actions actions work, but some differ. The release upload mechanism on Forgejo uses the `forgejo/release-action` action rather than GitHub's `upload-release-asset`.

The workflow triggers on a tag push matching `v*`. Before using it, create a release on Codeberg manually or via the Forgejo web UI, then push the tag. The workflow attaches the binary to the matching release.

- [ ] **Step 1: Create the `.forgejo/workflows/` directory and the workflow file**

```bash
mkdir -p .forgejo/workflows
```

Create `.forgejo/workflows/release.yml`:

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

      - name: Create release and upload binary
        uses: forgejo/release-action@v1
        with:
          direction: upload
          release-dir: target/release/
          release-files: omd2typst
          tag: ${{ github.ref_name }}
          token: ${{ secrets.GITHUB_TOKEN }}
          rename-files: |
            omd2typst=omd2typst-linux-x86_64
```

> **Forgejo action note:** If `forgejo/release-action` is unavailable on your Codeberg runner, use the Forgejo API directly via `curl` with `${{ secrets.GITHUB_TOKEN }}` as the bearer token. The API endpoint is `POST /api/v1/releases/{id}/assets`. Check the Forgejo documentation for the currently recommended upload action.

- [ ] **Step 2: Verify the CI workflow file is valid YAML**

Run: `python3 -c "import yaml; yaml.safe_load(open('.forgejo/workflows/release.yml'))" && echo OK`

Expected: `OK`

- [ ] **Step 3: Update the existing `.github/workflows/ci.yml` comment (optional hygiene)**

The `.github/workflows/ci.yml` runs on GitHub. Since the repo lives on Codeberg (Forgejo), consider whether this file is still used. If Codeberg mirrors to GitHub, the file is harmless. No change required — leave as-is.

- [ ] **Step 4: Commit**

```bash
git add .forgejo/workflows/release.yml
git commit -m "ci: add Forgejo release workflow for Linux x86_64 binary on tag push"
```

---

## Task 5: Update README with installation instructions

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add a pre-built binary section to the Installation section in `README.md`**

Find the existing `## Installation` section (after `## Usage`) and add before the `### Prerequisites` subsection:

```markdown
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
```

- [ ] **Step 2: Remove the `typst` prerequisite from the `### Prerequisites` subsection**

The current Prerequisites list requires `typst` to be installed. After this change, `typst` is no longer needed for PDF output. Remove it:

Before:
```markdown
### Prerequisites

- [Rust](https://rustup.rs) (stable)
- [Typst](https://typst.app/docs/tutorial/) — required for PDF output
```

After:
```markdown
### Prerequisites

- [Rust](https://rustup.rs) (stable) — only needed to build from source
```

- [ ] **Step 3: Run tests to confirm nothing is broken**

Run: `cargo test --workspace`

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: update installation instructions for zero-dependency binary"
```

---

## Verification Checklist

After all tasks are complete, verify:

- [ ] `cargo test --workspace` passes
- [ ] `./target/release/omd2typst --help` output is identical to before (UI unchanged)
- [ ] `omd2typst input.md output.pdf` produces a valid PDF without `typst` in PATH
- [ ] `omd2typst input.md output.pdf --template my-template.typ` works with a vault-relative template path
- [ ] `omd2typst input.md output.typ` still produces a `.typ` source file (unchanged path)
- [ ] `omd2typst --export-template my-template.typ` still works (unchanged path)
