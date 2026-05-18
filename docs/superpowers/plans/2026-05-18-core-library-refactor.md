# Core Library Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the single-crate `omd2typst` into a Cargo workspace with four crates — `core` (library), `cli` (binary), `wasm` (cdylib), `web` (stub) — with zero logic changes.

**Architecture:** The existing `src/` files split by responsibility: library code moves to `crates/core/`, the CLI binary to `crates/cli/`. Two new crates are scaffolded: `crates/wasm/` exposes WASM bindings for the core, and `crates/web/` is a `todo!()` stub. The root `Cargo.toml` becomes a workspace manifest. The only code changes are adding a `_options: &RenderOptions` parameter to `render_typst` and updating `main.rs` imports to use `omd2typst_core`.

**Tech Stack:** Rust stable, Cargo workspaces, `wasm-bindgen 0.2`

---

## File Structure

| Action | Path | Purpose |
|--------|------|---------|
| Replace | `Cargo.toml` | Workspace manifest — no package code |
| Create | `crates/core/Cargo.toml` | `omd2typst-core` library crate |
| Create | `crates/core/src/lib.rs` | Public API re-exports + `RenderOptions` stub |
| Create | `crates/core/src/ast.rs` | Verbatim copy of `src/ast.rs` |
| Create | `crates/core/src/parser.rs` | Verbatim copy of `src/parser.rs` |
| Create | `crates/core/src/renderer.rs` | Copy of `src/renderer.rs` + API update |
| Create | `crates/cli/Cargo.toml` | `omd2typst` binary crate |
| Create | `crates/cli/src/main.rs` | Copy of `src/main.rs` + import update |
| Create | `crates/wasm/Cargo.toml` | `omd2typst-wasm` cdylib crate |
| Create | `crates/wasm/src/lib.rs` | WASM entry point + smoke tests |
| Create | `crates/web/Cargo.toml` | `omd2typst-web` stub binary |
| Create | `crates/web/src/main.rs` | `todo!()` stub |
| Delete | `src/` | Replaced by `crates/` structure |
| Create | `.github/workflows/ci.yml` | CI: test + clippy + wasm-pack build |

---

## Task 1: Scaffold workspace — manifests and stub sources

Creates the workspace skeleton. After this task `cargo check --workspace` passes; no logic is in place yet.

**Files:**
- Replace: `Cargo.toml`
- Create: `crates/core/Cargo.toml`, `crates/cli/Cargo.toml`, `crates/wasm/Cargo.toml`, `crates/web/Cargo.toml`
- Create (stubs): `crates/core/src/lib.rs`, `crates/cli/src/main.rs`, `crates/wasm/src/lib.rs`, `crates/web/src/main.rs`

- [ ] **Step 1: Create directory tree**

```bash
mkdir -p crates/core/src crates/cli/src crates/wasm/src crates/web/src
```

- [ ] **Step 2: Write workspace Cargo.toml** (replaces existing root `Cargo.toml`)

```toml
[workspace]
members = ["crates/core", "crates/cli", "crates/wasm", "crates/web"]
resolver = "2"

[workspace.package]
edition = "2021"
rust-version = "1.70"
license = "MIT"
```

- [ ] **Step 3: Write crates/core/Cargo.toml**

```toml
[package]
name = "omd2typst-core"
version = "0.4.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Core Markdown→Typst conversion library for omd2typst"

[dependencies]
comrak = "0.28"
regex = "1.10"
```

- [ ] **Step 4: Write crates/cli/Cargo.toml**

```toml
[package]
name = "omd2typst"
version = "0.4.0"
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
```

- [ ] **Step 5: Write crates/wasm/Cargo.toml**

```toml
[package]
name = "omd2typst-wasm"
version = "0.4.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "WASM bindings for omd2typst-core"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
omd2typst-core = { path = "../core" }
wasm-bindgen = "0.2"
```

- [ ] **Step 6: Write crates/web/Cargo.toml**

```toml
[package]
name = "omd2typst-web"
version = "0.4.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "HTTP web service for omd2typst (stub)"

[dependencies]
omd2typst-core = { path = "../core" }
```

- [ ] **Step 7: Write stub source files so cargo can parse the workspace**

`crates/core/src/lib.rs`:
```rust
// stub — replaced in Task 2
```

`crates/cli/src/main.rs`:
```rust
fn main() {}
```

`crates/wasm/src/lib.rs`:
```rust
// stub — replaced in Task 4
```

`crates/web/src/main.rs`:
```rust
fn main() {
    todo!("web service — see spec 4")
}
```

- [ ] **Step 8: Verify workspace structure parses**

```bash
cargo check --workspace
```

Expected: all four crates compile (warnings are fine, errors are not).

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml crates/
git commit -m "chore: scaffold cargo workspace with four crate stubs"
```

---

## Task 2: Implement core crate

Populates `crates/core/src/` with the four source files. This is where all the library code lives. The only code change from the originals is in `renderer.rs`.

**Files:**
- Create: `crates/core/src/lib.rs` (new — public API + `RenderOptions`)
- Create: `crates/core/src/ast.rs` (verbatim copy)
- Create: `crates/core/src/parser.rs` (verbatim copy)
- Create: `crates/core/src/renderer.rs` (copy + two changes)

- [ ] **Step 1: Write crates/core/src/lib.rs**

```rust
pub mod ast;
mod parser;
mod renderer;

pub use ast::{
    Block, ColAlign, Document, FrontmatterValue,
    Inline, ListItem, TableRow,
};
pub use parser::parse_markdown;
pub use renderer::{render_typst, BUILTIN_TEMPLATE};

/// Rendering options — currently empty, extended as consumers reveal needs.
/// Present now so the render_typst() signature is stable when options arrive.
#[derive(Default)]
pub struct RenderOptions {}
```

- [ ] **Step 2: Copy src/ast.rs verbatim**

```bash
cp src/ast.rs crates/core/src/ast.rs
```

- [ ] **Step 3: Copy src/parser.rs verbatim**

```bash
cp src/parser.rs crates/core/src/parser.rs
```

- [ ] **Step 4: Copy src/renderer.rs and apply two changes**

```bash
cp src/renderer.rs crates/core/src/renderer.rs
```

Change 1 — add `RenderOptions` import after line 1 (`use crate::ast::*;`):

```rust
use crate::ast::*;
use crate::RenderOptions;
```

Change 2 — update the `render_typst` signature (currently line 7):

```rust
// Before:
pub fn render_typst(doc: &Document, template: Option<&str>) -> String {

// After:
pub fn render_typst(doc: &Document, template: Option<&str>, _options: &RenderOptions) -> String {
```

The function body is unchanged. `_options` is intentionally unused until spec 3.

- [ ] **Step 5: Verify core crate compiles**

```bash
cargo check -p omd2typst-core
```

Expected: no errors. Warnings about unused `_options` are acceptable — the leading `_` suppresses them.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/
git commit -m "feat: populate omd2typst-core with library source files"
```

---

## Task 3: Implement CLI crate

Updates `main.rs` to consume `omd2typst-core` instead of declaring inline modules.

**Files:**
- Create: `crates/cli/src/main.rs` (copy of `src/main.rs` + three targeted changes)

- [ ] **Step 1: Copy src/main.rs**

```bash
cp src/main.rs crates/cli/src/main.rs
```

- [ ] **Step 2: Replace the three module declarations at the top of the file**

Remove these three lines (currently lines 1–3):
```rust
mod ast;
mod parser;
mod renderer;
```

Replace with a single `use` import:
```rust
use omd2typst_core::{parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE};
```

- [ ] **Step 3: Update the three call sites that reference the old module paths**

Find and replace `renderer::BUILTIN_TEMPLATE` → `BUILTIN_TEMPLATE` (one occurrence, inside the `--export-template` block).

Find and replace `parser::parse_markdown` → `parse_markdown` (one occurrence).

Find and replace `renderer::render_typst(&doc, template_rel.as_deref())` → `render_typst(&doc, template_rel.as_deref(), &RenderOptions::default())` (one occurrence).

The resulting call sites look like this:
```rust
// --export-template block:
fs::write(path, BUILTIN_TEMPLATE)

// main conversion:
let doc = parse_markdown(&input);
let typst_src = render_typst(&doc, template_rel.as_deref(), &RenderOptions::default());
```

- [ ] **Step 4: Build the CLI crate**

```bash
cargo build -p omd2typst
```

Expected: binary compiles, no errors.

- [ ] **Step 5: Smoke-test the compiled binary on a fixture file**

```bash
echo "# Hello\n\nWorld." > /tmp/test.md
./target/debug/omd2typst /tmp/test.md /tmp/test.typ
cat /tmp/test.typ
```

Expected: a non-empty `.typ` file containing `= Hello` and `World.`.

- [ ] **Step 6: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat: migrate cli crate to consume omd2typst-core"
```

---

## Task 4: Implement WASM crate (TDD)

Writes the WASM entry point using test-driven development. The `rlib` crate-type allows `cargo test` to run the smoke tests on the native target without needing wasm-pack.

**Files:**
- Modify: `crates/wasm/src/lib.rs` (replace stub with tests + implementation)

- [ ] **Step 1: Write the failing tests**

Write `crates/wasm/src/lib.rs` with tests only and stub functions that `todo!()`:

```rust
use wasm_bindgen::prelude::*;
use omd2typst_core::{parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE};

#[wasm_bindgen]
pub fn render_to_typst(markdown: &str, template_path: Option<String>) -> String {
    todo!()
}

#[wasm_bindgen]
pub fn get_builtin_template() -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_to_typst_returns_non_empty() {
        let result = render_to_typst("# Hello", None);
        assert!(!result.is_empty());
    }

    #[test]
    fn get_builtin_template_returns_non_empty() {
        assert!(!get_builtin_template().is_empty());
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p omd2typst-wasm
```

Expected: both tests FAIL with `thread panicked at 'not yet implemented'`.

- [ ] **Step 3: Implement the two functions**

Replace the `todo!()` bodies with real implementations:

```rust
use wasm_bindgen::prelude::*;
use omd2typst_core::{parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE};

#[wasm_bindgen]
pub fn render_to_typst(markdown: &str, template_path: Option<String>) -> String {
    let doc = parse_markdown(markdown);
    render_typst(&doc, template_path.as_deref(), &RenderOptions::default())
}

#[wasm_bindgen]
pub fn get_builtin_template() -> String {
    BUILTIN_TEMPLATE.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_to_typst_returns_non_empty() {
        let result = render_to_typst("# Hello", None);
        assert!(!result.is_empty());
    }

    #[test]
    fn get_builtin_template_returns_non_empty() {
        assert!(!get_builtin_template().is_empty());
    }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo test -p omd2typst-wasm
```

Expected: both tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wasm/src/lib.rs
git commit -m "feat: add wasm crate with render_to_typst and smoke tests"
```

---

## Task 5: Delete src/ and run full verification

Removes the original `src/` directory now that all files have been migrated to `crates/`. Verifies the entire workspace builds and all tests pass.

**Files:**
- Delete: `src/` (all four files inside it)

- [ ] **Step 1: Delete the src/ directory**

```bash
rm -rf src/
```

- [ ] **Step 2: Verify full workspace**

```bash
cargo test --workspace
```

Expected: all tests pass. The only tests are the two WASM smoke tests in `crates/wasm/src/lib.rs`.

- [ ] **Step 3: Verify clippy is clean**

```bash
cargo clippy --workspace -- -D warnings
```

Expected: no warnings treated as errors. If clippy warns about `_options` being unused despite the leading underscore, add `#[allow(unused_variables)]` above the function — but this is unlikely since `_`-prefixed parameters are suppressed by default.

- [ ] **Step 4: Verify CLI still works end-to-end**

```bash
echo "# Hello\n\nWorld." > /tmp/test.md
./target/debug/omd2typst /tmp/test.md /tmp/test.typ && cat /tmp/test.typ
```

Expected: non-empty `.typ` output, same as Task 3 Step 5.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: delete src/ — all code now lives in crates/"
```

---

## Task 6: Add GitHub Actions CI workflow

Adds CI that runs on every push and pull request to `main`. Two jobs: `test` (native build + clippy) and `wasm` (wasm-pack build verification).

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create the workflows directory**

```bash
mkdir -p .github/workflows
```

- [ ] **Step 2: Write .github/workflows/ci.yml**

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace
      - run: cargo clippy --workspace -- -D warnings

  wasm:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install wasm-pack
      - run: wasm-pack build crates/wasm --target bundler
```

- [ ] **Step 3: Commit**

```bash
git add .github/
git commit -m "ci: add GitHub Actions — cargo test, clippy, wasm-pack build"
```

---

## Self-Review

**Spec coverage:**
- [x] Workspace layout with four crates — Task 1
- [x] Workspace Cargo.toml — Task 1 Step 2
- [x] `crates/core` Cargo.toml — Task 1 Step 3
- [x] `crates/cli` Cargo.toml — Task 1 Step 4
- [x] `crates/wasm` Cargo.toml — Task 1 Step 5
- [x] `crates/web` Cargo.toml — Task 1 Step 6
- [x] `RenderOptions {}` stub in lib.rs — Task 2 Step 1
- [x] `pub use` re-exports in lib.rs — Task 2 Step 1
- [x] `ast.rs` moved verbatim — Task 2 Step 2
- [x] `parser.rs` moved verbatim — Task 2 Step 3
- [x] `renderer.rs` moved + `use crate::RenderOptions` + `_options` param — Task 2 Step 4
- [x] `main.rs` module declarations replaced — Task 3 Step 2
- [x] `main.rs` three call sites updated — Task 3 Step 3
- [x] WASM `render_to_typst` + `get_builtin_template` — Task 4 Step 3
- [x] WASM smoke tests — Task 4 Steps 1–4
- [x] Web stub `main.rs` — Task 1 Step 7
- [x] `src/` deleted — Task 5 Step 1
- [x] `cargo test --workspace` gate — Task 5 Step 2
- [x] CI workflow — Task 6

**Placeholder scan:** No TBDs, no incomplete sections.

**Type consistency:** `render_typst(doc: &Document, template: Option<&str>, _options: &RenderOptions)` — used consistently in Tasks 2, 3, and 4. `RenderOptions::default()` call sites match the `#[derive(Default)]` in lib.rs.
