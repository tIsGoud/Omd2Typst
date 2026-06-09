# Multi-line Summary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support YAML `|` literal block scalars in frontmatter so that multi-line summary text renders as separate lines in the PDF cover page.

**Architecture:** Add a dedicated `Multiline(Vec<Vec<Inline>>)` variant to `FrontmatterValue` in `ast.rs`. The parser detects `|`, collects indented body lines, parses each line independently with `parse_inline_markdown`, and stores the result as `Multiline`. The renderer adds a match arm for `Multiline` in `render_fm_value` that always emits a Typst content block with `\` line breaks — no existing behaviour is changed. `is_plain_inlines` is not touched. The plugin then rebuilds its WASM bundle from the updated submodule.

**Tech Stack:** Rust, comrak (Markdown parser), wasm-pack, Node.js/TypeScript (plugin)

---

## Files

| Action | Path | Purpose |
|--------|------|---------|
| Modify | `crates/core/src/ast.rs` | Add `Multiline(Vec<Vec<Inline>>)` variant to `FrontmatterValue` |
| Modify | `crates/core/src/parser.rs` | Add `\|` block scalar handler producing `Multiline` |
| Modify | `crates/core/src/renderer.rs` | Add `Multiline` match arms in `render_fm_value` and `fm_plain_text` |
| Modify | `crates/core/Cargo.toml` | Bump `omd2typst-core` version `0.5.0` → `0.5.1` |
| Modify | `crates/wasm/Cargo.toml` | Bump `omd2typst-wasm` version `0.5.0` → `0.5.1` |
| Modify | `crates/cli/Cargo.toml` | Bump CLI version `0.10.2` → `0.10.3` |
| Modify | `obsidian-omd2typst/libs/omd2typst` | Update submodule pointer to new commit |
| Replaced by build | `obsidian-omd2typst/src/wasm/omd2typst-pkg/` | Rebuilt by `npm run build:wasm` — do not edit by hand |

---

## Task 1: Add `Multiline` variant to `FrontmatterValue`

**Files:**
- Modify: `crates/core/src/ast.rs` lines 55–62
- Modify: `crates/core/src/renderer.rs` — `render_fm_value` (line 698) and `fm_plain_text` (line 720)

Adding the variant before writing tests lets the code compile so failing tests can run (rather than fail to build).

- [ ] **Step 1: Add `Multiline` to the enum in `crates/core/src/ast.rs`**

  Replace:
  ```rust
  /// `Raw` — an already-formatted Typst literal (inline arrays `(…)`).
  #[derive(Debug)]
  pub enum FrontmatterValue {
      Inlines(Vec<Inline>),
      Raw(String),
  }
  ```

  With:
  ```rust
  /// `Raw` — an already-formatted Typst literal (inline arrays `(…)`).
  ///
  /// `Multiline` — a YAML `|` block scalar; each inner `Vec<Inline>` is one line.
  /// Always rendered as a Typst content block with `\` line breaks.
  #[derive(Debug)]
  pub enum FrontmatterValue {
      Inlines(Vec<Inline>),
      Raw(String),
      Multiline(Vec<Vec<Inline>>),
  }
  ```

- [ ] **Step 2: Add `Multiline` match arm to `render_fm_value` in `crates/core/src/renderer.rs`**

  Replace:
  ```rust
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
  ```

  With:
  ```rust
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
  ```

- [ ] **Step 3: Add `Multiline` match arm to `fm_plain_text` in `crates/core/src/renderer.rs`**

  Replace:
  ```rust
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
  ```

  With:
  ```rust
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
  ```

- [ ] **Step 4: Verify the crate still compiles with no new tests yet**

  ```bash
  cargo build --package omd2typst-core 2>&1 | grep -E "error|warning: unused"
  ```

  Expected: no errors. (Unused variant warning is acceptable at this point.)

---

## Task 2: Write failing tests

**Files:**
- Modify: `crates/core/src/parser.rs` — append to `mod tests` block (line 592)
- Modify: `crates/core/src/renderer.rs` — append to `mod tests` block (line 767)

- [ ] **Step 1: Add parser unit tests to `crates/core/src/parser.rs`**

  Append inside the existing `mod tests { ... }` block before the closing `}`:

  ```rust
  #[test]
  fn yaml_block_scalar_produces_multiline_variant() {
      let yaml = "summary: |\n  First line\n  Second line\n";
      let result = parse_yaml_frontmatter(yaml);
      assert_eq!(result.len(), 1);
      let (key, val) = &result[0];
      assert_eq!(key, "summary");
      assert!(
          matches!(val, FrontmatterValue::Multiline(_)),
          "Expected Multiline variant, got: {val:?}"
      );
  }

  #[test]
  fn yaml_block_scalar_line_count() {
      let yaml = "summary: |\n  First line\n  Second line\n  Third line\n";
      let result = parse_yaml_frontmatter(yaml);
      let (_, val) = &result[0];
      if let FrontmatterValue::Multiline(lines) = val {
          assert_eq!(lines.len(), 3, "Expected 3 lines, got {}", lines.len());
      } else {
          panic!("Expected Multiline, got: {val:?}");
      }
  }

  #[test]
  fn yaml_block_scalar_strips_trailing_blank_lines() {
      let yaml = "summary: |\n  Only line\n\n\nother: value\n";
      let result = parse_yaml_frontmatter(yaml);
      let summary = result.iter().find(|(k, _)| k == "summary")
          .expect("summary key missing");
      if let FrontmatterValue::Multiline(lines) = &summary.1 {
          assert_eq!(lines.len(), 1, "Trailing blank lines must be stripped");
      } else {
          panic!("Expected Multiline, got: {:?}", summary.1);
      }
  }

  #[test]
  fn yaml_block_scalar_stops_at_next_key() {
      let yaml = "summary: |\n  Body line\ntitle: My Title\n";
      let result = parse_yaml_frontmatter(yaml);
      assert_eq!(result.len(), 2, "Both keys should be parsed");
      assert_eq!(result[0].0, "summary");
      assert_eq!(result[1].0, "title");
  }
  ```

- [ ] **Step 2: Add integration test to `crates/core/src/renderer.rs`**

  Append inside the existing `mod tests { ... }` block before the closing `}`:

  ```rust
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
  ```

- [ ] **Step 3: Run tests — confirm the new tests fail**

  ```bash
  cargo test --package omd2typst-core 2>&1 | grep -E "FAILED|ok|test result"
  ```

  Expected: 5 new tests FAILED, existing 8 tests still pass.

---

## Task 3: Implement the parser fix

**Files:**
- Modify: `crates/core/src/parser.rs` — `parse_yaml_frontmatter` function (lines 101–143)

- [ ] **Step 1: Add the `|` block scalar branch in `parse_yaml_frontmatter`**

  Replace the final `else` branch (currently `} else { result.push(...) }`):

  ```rust
  // Before:
  } else {
      result.push((key, yaml_scalar_to_fm_value(rest)));
  }
  ```

  With:

  ```rust
  } else if rest == "|" {
      // YAML literal block scalar — collect indented body lines.
      // Indent depth is determined from the first non-empty body line.
      let mut indent: Option<usize> = None;
      let mut body: Vec<String> = Vec::new();

      loop {
          match lines.peek() {
              None => break,
              Some(raw) => {
                  let raw: &str = raw;
                  if raw.trim().is_empty() {
                      body.push(String::new());
                      lines.next();
                  } else {
                      let line_indent = raw.len() - raw.trim_start().len();
                      let ind = *indent.get_or_insert(line_indent);
                      if line_indent >= ind {
                          body.push(raw[ind..].trim_end().to_string());
                          lines.next();
                      } else {
                          break;
                      }
                  }
              }
          }
      }

      // YAML clip chomping: strip trailing empty lines.
      while body.last().map_or(false, |s| s.is_empty()) {
          body.pop();
      }

      if !body.is_empty() {
          // Parse each line independently so rich inline formatting
          // (bold, italic) works per-line. Lines are stored separately;
          // the renderer joins them with Typst \ line breaks.
          let lines_parsed: Vec<Vec<Inline>> = body
              .iter()
              .map(|line| parse_inline_markdown(line))
              .collect();
          result.push((key, FrontmatterValue::Multiline(lines_parsed)));
      }
  } else {
      result.push((key, yaml_scalar_to_fm_value(rest)));
  }
  ```

- [ ] **Step 2: Run parser tests — confirm they pass**

  ```bash
  cargo test --package omd2typst-core parser 2>&1 | grep -E "FAILED|ok"
  ```

  Expected: all 4 new parser tests pass alongside the existing 4.

- [ ] **Step 3: Confirm integration test still fails (renderer match not yet added — wait, it was added in Task 1)**

  The renderer match arm was added in Task 1, so the integration test should already pass. Run it:

  ```bash
  cargo test --package omd2typst-core frontmatter_block_scalar 2>&1
  ```

  Expected: PASS.

- [ ] **Step 4: Run full test suite — all 13 tests pass**

  ```bash
  cargo test --package omd2typst-core 2>&1 | grep "test result"
  ```

  Expected: `test result: ok. 13 passed; 0 failed`

- [ ] **Step 5: Commit**

  ```bash
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst add \
      crates/core/src/ast.rs \
      crates/core/src/parser.rs \
      crates/core/src/renderer.rs
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst commit -m \
      "feat: support YAML | block scalar for multi-line frontmatter values

  Add Multiline(Vec<Vec<Inline>>) variant to FrontmatterValue so block
  scalars are represented explicitly rather than overloading HardBreak.
  Parser collects indented body lines, parses each independently.
  Renderer emits a Typst content block with backslash line breaks.
  is_plain_inlines and all string-comparison paths are unchanged."
  ```

---

## Task 4: Release omd2typst

**Files:**
- Modify: `crates/core/Cargo.toml` — version `0.5.0` → `0.5.1`
- Modify: `crates/wasm/Cargo.toml` — version `0.5.0` → `0.5.1`
- Modify: `crates/cli/Cargo.toml` — version `0.10.2` → `0.10.3`
- Modify: `RELEASE_NOTES.md`

- [ ] **Step 1: Add release notes to `RELEASE_NOTES.md`**

  Prepend after the `# Release Notes` heading:

  ```markdown
  ## v0.10.3 — Support YAML `|` block scalar for multi-line frontmatter

  Frontmatter fields now accept the YAML literal block scalar syntax (`|`).
  Multi-line values — most usefully `summary` — render as separate lines in
  the PDF output instead of being collapsed to a flat string.

  ---
  ```

- [ ] **Step 2: Bump versions**

  In `crates/core/Cargo.toml`, change:
  ```toml
  version = "0.5.1"
  ```

  In `crates/wasm/Cargo.toml`, change:
  ```toml
  version = "0.5.1"
  ```

  In `crates/cli/Cargo.toml`, change:
  ```toml
  version = "0.10.3"
  ```

- [ ] **Step 3: Update `Cargo.lock` and run full test suite**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/omd2typst && cargo test --package omd2typst-core 2>&1 | grep "test result"
  ```

  Expected: `test result: ok. 13 passed; 0 failed`

- [ ] **Step 4: Commit, tag, and push**

  ```bash
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst add \
      crates/core/Cargo.toml crates/wasm/Cargo.toml crates/cli/Cargo.toml \
      Cargo.lock RELEASE_NOTES.md
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst commit -m \
      "chore: bump versions to 0.10.3 / core 0.5.1 / wasm 0.5.1"
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst tag v0.10.3
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst push && \
      git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst push --tags
  ```

---

## Task 5: Update plugin — submodule, WASM rebuild, release

**Files:**
- Modify: `obsidian-omd2typst/libs/omd2typst` (submodule pointer)
- Replaced by build: `obsidian-omd2typst/src/wasm/omd2typst-pkg/` (do not edit by hand)
- Modify: `obsidian-omd2typst/manifest.json` — version `0.8.11` → `0.8.12`
- Modify: `obsidian-omd2typst/package.json` — version `0.8.11` → `0.8.12`
- Modify: `obsidian-omd2typst/RELEASE_NOTES.md`

- [ ] **Step 1: Update the submodule to the new omd2typst commit**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst/libs/omd2typst && git pull origin main
  ```

  Verify it is on the `v0.10.3` commit:
  ```bash
  git -C /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst/libs/omd2typst log --oneline -1
  ```

  Expected: the commit message contains `chore: bump versions to 0.10.3`.

- [ ] **Step 2: Rebuild the WASM bundle**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst && npm run build:wasm 2>&1 | tail -3
  ```

  Expected: `✓ omd2typst WASM written to .../src/wasm/omd2typst-pkg`

- [ ] **Step 3: Build the plugin**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst && node esbuild.config.mjs production 2>&1
  ```

  Expected: `main.js  ~1.8mb` with no errors.

- [ ] **Step 4: Run lint and tests**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst && npm run lint && npm test 2>&1 | tail -5
  ```

  Expected: lint clean, `27 passed`.

- [ ] **Step 5: Bump plugin version**

  In `manifest.json`, change:
  ```json
  "version": "0.8.12"
  ```

  In `package.json`, change:
  ```json
  "version": "0.8.12"
  ```

- [ ] **Step 6: Add release notes to `obsidian-omd2typst/RELEASE_NOTES.md`**

  Prepend after the `# Release Notes` heading:

  ```markdown
  ## v0.8.12 — Support multi-line summary on cover page

  The `summary` frontmatter field now supports multi-line text using the YAML
  literal block scalar syntax:

  ```yaml
  summary: |
    First line of the summary.
    Second line of the summary.
  ```

  Each line is rendered as a separate line in the summary box on the PDF cover
  page. Built on omd2typst v0.10.3.

  ---
  ```

- [ ] **Step 7: Commit, tag, and push**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst
  git add libs/omd2typst src/wasm/omd2typst-pkg manifest.json \
      package.json package-lock.json RELEASE_NOTES.md
  git commit -m "v0.8.12 — support multi-line summary via YAML block scalar"
  git tag 0.8.12
  git push && git push --tags
  ```
