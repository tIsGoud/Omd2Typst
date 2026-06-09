# Multi-line Summary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support YAML `|` literal block scalars in frontmatter so that multi-line summary text renders as separate lines in the PDF cover page.

**Architecture:** Two targeted fixes in `omd2typst-core`. (1) The parser's `parse_yaml_frontmatter` gains a branch that collects indented body lines when it sees `|`, joins them with Markdown hard-break syntax so `comrak` produces `HardBreak` inlines. (2) The renderer's `is_plain_inlines` stops treating `HardBreak` as plain text, routing multi-line values through the content-block path which already emits correct Typst line-break syntax (`\` + newline). The plugin then rebuilds its WASM bundle from the updated submodule.

**Tech Stack:** Rust, comrak (Markdown parser), wasm-pack, Node.js/TypeScript (plugin)

---

## Files

| Action | Path | Purpose |
|--------|------|---------|
| Modify | `crates/core/src/parser.rs` | Add `|` block scalar handler in `parse_yaml_frontmatter` |
| Modify | `crates/core/src/renderer.rs` | Remove `HardBreak` from `is_plain_inlines` |
| Modify | `crates/core/Cargo.toml` | Bump `omd2typst-core` version `0.5.0` → `0.5.1` |
| Modify | `crates/wasm/Cargo.toml` | Bump `omd2typst-wasm` version `0.5.0` → `0.5.1` |
| Modify | `crates/cli/Cargo.toml` | Bump CLI version `0.10.2` → `0.10.3` |
| Modify | `obsidian-omd2typst/libs/omd2typst` | Update submodule pointer to new commit |
| Modify | `obsidian-omd2typst/src/wasm/omd2typst-pkg/` | Rebuilt by `npm run build:wasm` (do not edit by hand) |

---

## Task 1: Write failing tests

**Files:**
- Modify: `crates/core/src/parser.rs` (add to `mod tests` at line 592)
- Modify: `crates/core/src/renderer.rs` (add to `mod tests` at line 767)

- [ ] **Step 1: Add failing parser unit tests to `crates/core/src/parser.rs`**

  Append inside the existing `mod tests { ... }` block (before the closing `}`):

  ```rust
  #[test]
  fn yaml_block_scalar_produces_hard_breaks() {
      let yaml = "summary: |\n  First line\n  Second line\n";
      let result = parse_yaml_frontmatter(yaml);
      assert_eq!(result.len(), 1);
      let (key, val) = &result[0];
      assert_eq!(key, "summary");
      match val {
          FrontmatterValue::Inlines(inlines) => {
              assert!(
                  inlines.iter().any(|i| matches!(i, Inline::HardBreak)),
                  "Expected HardBreak between lines, got: {inlines:?}"
              );
          }
          other => panic!("Expected Inlines variant, got: {other:?}"),
      }
  }

  #[test]
  fn yaml_block_scalar_strips_trailing_blank_lines() {
      let yaml = "summary: |\n  Only line\n\n\nother: value\n";
      let result = parse_yaml_frontmatter(yaml);
      let summary = result.iter().find(|(k, _)| k == "summary")
          .expect("summary key missing");
      match &summary.1 {
          FrontmatterValue::Inlines(inlines) => {
              assert!(
                  !matches!(inlines.last(), Some(Inline::HardBreak)),
                  "Trailing HardBreak should be stripped by clip chomping"
              );
          }
          other => panic!("Expected Inlines, got: {other:?}"),
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

- [ ] **Step 2: Add failing integration test to `crates/core/src/renderer.rs`**

  Append inside the existing `mod tests { ... }` block (before the closing `}`):

  ```rust
  #[test]
  fn frontmatter_block_scalar_renders_as_content_block() {
      // A YAML | block scalar summary must produce a Typst content block
      // with \ line breaks, not a flat string with spaces.
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
  cargo test --package omd2typst-core 2>&1 | grep -E "FAILED|ok|error"
  ```

  Expected: `yaml_block_scalar_produces_hard_breaks` FAILED, `yaml_block_scalar_strips_trailing_blank_lines` FAILED, `yaml_block_scalar_stops_at_next_key` FAILED, `frontmatter_block_scalar_renders_as_content_block` FAILED. Existing 8 tests still pass.

---

## Task 2: Implement the parser fix

**Files:**
- Modify: `crates/core/src/parser.rs` — `parse_yaml_frontmatter` function (lines 101–143)

- [ ] **Step 1: Add the `|` block scalar branch in `parse_yaml_frontmatter`**

  In `parse_yaml_frontmatter`, replace the final `else` branch:

  ```rust
  // Before (lines ~136–138):
  } else {
      result.push((key, yaml_scalar_to_fm_value(rest)));
  }
  ```

  With:

  ```rust
  } else if rest == "|" {
      // YAML literal block scalar — collect indented body lines.
      // Indent is determined from the first non-empty body line.
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
          // Join with Markdown hard-break syntax (two trailing spaces + newline)
          // so comrak emits HardBreak inlines, which render_fm_value routes to
          // a Typst content block instead of a plain string.
          let text = body.join("  \n");
          result.push((key, FrontmatterValue::Inlines(parse_inline_markdown(&text))));
      }
  } else {
      result.push((key, yaml_scalar_to_fm_value(rest)));
  }
  ```

- [ ] **Step 2: Run the parser tests — confirm they pass**

  ```bash
  cargo test --package omd2typst-core parser 2>&1 | grep -E "FAILED|ok"
  ```

  Expected: all four parser tests pass (`strip_inline_comment`, `strip_block_comment`, `comment_preserved_in_code_block`, `no_comments_unchanged`, `yaml_block_scalar_produces_hard_breaks`, `yaml_block_scalar_strips_trailing_blank_lines`, `yaml_block_scalar_stops_at_next_key`).

- [ ] **Step 3: Confirm integration test still fails (renderer not yet fixed)**

  ```bash
  cargo test --package omd2typst-core frontmatter_block_scalar 2>&1
  ```

  Expected: FAILED — the output will contain `#let summary = "First line Second line"` (flat string, HardBreak still considered plain).

---

## Task 3: Implement the renderer fix

**Files:**
- Modify: `crates/core/src/renderer.rs` — `is_plain_inlines` function (line 716)

- [ ] **Step 1: Remove `HardBreak` from `is_plain_inlines`**

  ```rust
  // Before:
  fn is_plain_inlines(inlines: &[Inline]) -> bool {
      inlines.iter().all(|i| matches!(i, Inline::Text(_) | Inline::SoftBreak | Inline::HardBreak))
  }

  // After:
  fn is_plain_inlines(inlines: &[Inline]) -> bool {
      inlines.iter().all(|i| matches!(i, Inline::Text(_) | Inline::SoftBreak))
  }
  ```

- [ ] **Step 2: Run all tests — confirm everything passes**

  ```bash
  cargo test --package omd2typst-core 2>&1 | grep -E "FAILED|ok|test result"
  ```

  Expected: `test result: ok. 12 passed; 0 failed`

- [ ] **Step 3: Commit**

  ```bash
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst add crates/core/src/parser.rs crates/core/src/renderer.rs
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst commit -m "feat: support YAML | block scalar for multi-line frontmatter values

  Parser: detect | indicator, collect indented body lines, join with
  Markdown hard-break syntax so comrak produces HardBreak inlines.
  Renderer: exclude HardBreak from is_plain_inlines so multi-line values
  render as Typst content blocks instead of flat strings."
  ```

---

## Task 4: Release omd2typst

**Files:**
- Modify: `crates/core/Cargo.toml` — version `0.5.0` → `0.5.1`
- Modify: `crates/wasm/Cargo.toml` — version `0.5.0` → `0.5.1`
- Modify: `crates/cli/Cargo.toml` — version `0.10.2` → `0.10.3`

- [ ] **Step 1: Add release notes to `RELEASE_NOTES.md` in the omd2typst repo**

  Prepend after the `# Release Notes` heading:

  ```markdown
  ## v0.10.3 — Support YAML `|` block scalar for multi-line frontmatter

  Frontmatter fields now accept the YAML literal block scalar syntax (`|`).
  Multi-line values — most usefully `summary` — render as separate lines in
  the PDF output instead of being collapsed to a flat string.

  ---
  ```

- [ ] **Step 2: Bump versions in all three crates**

  In `crates/core/Cargo.toml`:
  ```toml
  version = "0.5.1"
  ```

  In `crates/wasm/Cargo.toml`:
  ```toml
  version = "0.5.1"
  ```

  In `crates/cli/Cargo.toml`:
  ```toml
  version = "0.10.3"
  ```

- [ ] **Step 3: Update `Cargo.lock` and run full test suite**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/omd2typst && cargo test --package omd2typst-core 2>&1 | grep "test result"
  ```

  Expected: `test result: ok. 12 passed; 0 failed`

- [ ] **Step 4: Commit and tag**

  ```bash
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst add crates/core/Cargo.toml crates/wasm/Cargo.toml crates/cli/Cargo.toml Cargo.lock
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst commit -m "chore: bump versions to 0.10.3 / core 0.5.1 / wasm 0.5.1"
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst tag v0.10.3
  git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst push && git -C /Users/albert/Projects/Omd2Typst@Github/omd2typst push --tags
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

  Verify it points to the `v0.10.3` commit:
  ```bash
  git -C /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst/libs/omd2typst log --oneline -1
  ```

- [ ] **Step 2: Rebuild the WASM bundle**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst && npm run build:wasm 2>&1 | tail -5
  ```

  Expected: `✓ omd2typst WASM written to .../src/wasm/omd2typst-pkg`

- [ ] **Step 3: Build the plugin**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst && node esbuild.config.mjs production 2>&1
  ```

  Expected: `main.js  ~1.8mb` with no errors.

- [ ] **Step 4: Run lint and tests**

  ```bash
  cd /Users/albert/Projects/Omd2Typst@Github/obsidian-omd2typst && npm run lint && npm test 2>&1 | tail -10
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

- [ ] **Step 6: Add release notes**

  Prepend to `RELEASE_NOTES.md` after the `# Release Notes` heading:

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
  git add libs/omd2typst src/wasm/omd2typst-pkg manifest.json package.json package-lock.json RELEASE_NOTES.md
  git commit -m "v0.8.12 — support multi-line summary via YAML block scalar"
  git tag 0.8.12
  git push && git push --tags
  ```
