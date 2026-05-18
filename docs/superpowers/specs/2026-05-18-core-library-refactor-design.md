# Core Library Refactor Design

**Date:** 2026-05-18
**Status:** Approved for implementation planning

---

## Overview

Refactor the `omd2typst` repository from a single-crate CLI tool into a Cargo workspace with four crates. This is a pure structural migration — no logic changes, no new features, no behavioral differences. The goal is a clean foundation that all four planned consumers can build on independently.

---

## Platform Context (Umbrella Architecture)

This refactor establishes the shared foundation for the full omd2typst platform. Four consumers exist or are planned:

```
omd2typst workspace
└── crates/core  (omd2typst-core)
        │
        ├── crates/cli      → zero-dependency CLI binary (this refactor; Typst embedding in spec 2)
        │                     consumed by: CI/CD pipelines, direct user invocation
        │
        ├── crates/wasm     → WASM build via wasm-pack
        │                     consumed by: obsidian-omd2typst plugin (separate repo, git submodule)
        │
        └── crates/web      → HTTP web service (stub; specced separately)
                              consumed by: browser upload workflow
```

Each consumer depends only on `omd2typst-core`. The Obsidian plugin is an external consumer in its own repository; it is not a workspace member. It consumes `crates/wasm` by including this repo as a git submodule and running `wasm-pack build crates/wasm` at plugin build time.

**Planned specs following this one:**
- Spec 2: Embed Typst library into core + font loading strategy → zero-dependency CLI binary + release pipeline
- Spec 3: SVG callout icons with accent-colour matching (extends `RenderOptions`)
- Spec 4: Web service (Axum HTTP server using core natively)
- Plugin plan update: target `crates/wasm` instead of the provisional `src/lib.rs` from the original plugin plan Task 1

---

## Workspace Layout

```
omd2typst/
├── Cargo.toml                  ← workspace manifest only (no code)
└── crates/
    ├── core/                   ← omd2typst-core  (library)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs          ← public API + RenderOptions stub
    │       ├── ast.rs          ← moved from src/ast.rs
    │       ├── parser.rs       ← moved from src/parser.rs
    │       └── renderer.rs     ← moved from src/renderer.rs
    ├── cli/                    ← omd2typst  (binary)
    │   ├── Cargo.toml
    │   └── src/
    │       └── main.rs         ← moved from src/main.rs
    ├── wasm/                   ← omd2typst-wasm  (cdylib)
    │   ├── Cargo.toml
    │   └── src/
    │       └── lib.rs          ← WASM entry point (new)
    └── web/                    ← omd2typst-web  (binary stub)
        ├── Cargo.toml
        └── src/
            └── main.rs         ← stub only
```

The existing `src/` directory is deleted once all files are moved. The root `Cargo.toml` is replaced by the workspace manifest.

---

## Workspace Cargo.toml

```toml
[workspace]
members = ["crates/core", "crates/cli", "crates/wasm", "crates/web"]
resolver = "2"

[workspace.package]
edition = "2021"
rust-version = "1.70"
license = "MIT"
```

---

## Core Crate

### Public API (crates/core/src/lib.rs)

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

### Updated render_typst signature

```rust
pub fn render_typst(
    doc: &Document,
    template: Option<&str>,
    _options: &RenderOptions,   // unused until spec 3
) -> String
```

### Cargo.toml

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

---

## CLI Crate

Thin shell — all path helpers, CLI argument parsing, and `typst compile` invocation stay here. No logic moves to core.

### Module changes in main.rs

Replace:
```rust
mod ast;
mod parser;
mod renderer;
```

With:
```rust
use omd2typst_core::{parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE};
```

The one call site that changes:
```rust
// Before:
renderer::render_typst(&doc, template_rel.as_deref())

// After:
render_typst(&doc, template_rel.as_deref(), &RenderOptions::default())
```

### Cargo.toml

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

---

## WASM Crate

### crates/wasm/src/lib.rs

```rust
use wasm_bindgen::prelude::*;
use omd2typst_core::{parse_markdown, render_typst, RenderOptions, BUILTIN_TEMPLATE};

/// Convert Markdown to a Typst source string.
/// Pass the full content of a .typ template as template_src, or None for the built-in.
#[wasm_bindgen]
pub fn render_to_typst(markdown: &str, template_src: Option<String>) -> String {
    let doc = parse_markdown(markdown);
    render_typst(&doc, template_src.as_deref(), &RenderOptions::default())
}

/// Return the built-in Typst template source.
#[wasm_bindgen]
pub fn get_builtin_template() -> String {
    BUILTIN_TEMPLATE.to_string()
}
```

### Cargo.toml

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

---

## Web Stub Crate

### crates/web/src/main.rs

```rust
fn main() {
    todo!("web service — see spec 4")
}
```

### Cargo.toml

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

---

## Migration

This is a mechanical file move. No logic changes in any file.

| Source | Destination | Changes |
|---|---|---|
| `src/ast.rs` | `crates/core/src/ast.rs` | None |
| `src/parser.rs` | `crates/core/src/parser.rs` | None |
| `src/renderer.rs` | `crates/core/src/renderer.rs` | Add `use crate::RenderOptions;` at top; add `_options: &RenderOptions` parameter to `render_typst` |
| `src/main.rs` | `crates/cli/src/main.rs` | Replace `mod` declarations with `use omd2typst_core::…`; update `render_typst` call site |
| `Cargo.toml` | Split: workspace root + `crates/cli/Cargo.toml` | — |
| `src/` | deleted | — |

---

## Testing

**Existing tests:** The inline `#[cfg(test)]` modules in `parser.rs` and `renderer.rs` move with those files into `crates/core/src/`. No test code changes.

**New WASM smoke tests** in `crates/wasm/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_to_typst_returns_non_empty() {
        let result = render_to_typst("# Hello".to_string(), None);
        assert!(!result.is_empty());
    }

    #[test]
    fn get_builtin_template_returns_non_empty() {
        assert!(!get_builtin_template().is_empty());
    }
}
```

Full WASM integration tests (headless browser) are deferred to when the plugin plan is executed.

**Verification gate:** `cargo test --workspace` must pass before the refactor is considered complete.

---

## CI

`.github/workflows/ci.yml` — runs on push and pull request to `main`:

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

No release pipeline — that lands with spec 2 (Typst embedding) when the binary is worth distributing.

---

## Dependency Map

| Dependency | core | cli | wasm | web |
|---|---|---|---|---|
| `comrak` | ✓ | | | |
| `regex` | ✓ | | | |
| `anyhow` | | ✓ | | |
| `clap` | | ✓ | | |
| `wasm-bindgen` | | | ✓ | |
| `omd2typst-core` | | ✓ | ✓ | ✓ |

---

## Out of Scope

- Typst library embedding and font loading (spec 2)
- SVG callout icons with accent-colour matching (spec 3)
- Web service implementation (spec 4)
- Release pipeline and pre-built binary distribution (spec 2)
- Plugin plan update to target `crates/wasm` (follows this refactor)
