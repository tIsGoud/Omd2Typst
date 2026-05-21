# SVG Callout Icons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace emoji prefixes in callout block titles with Lucide SVG icons that match each callout's accent colour, rendered inline before the title text.

**Architecture:** All changes are confined to `crates/core/src/renderer.rs`. The Rust `callout_icon()` function is deleted and the emoji is removed from the title string. Both `BUILTIN_PREAMBLE` and `BUILTIN_TEMPLATE` Typst const strings gain a `_co-icons` dict (SVG bytes per kind) and an updated `callout()` function that renders the icon using `image(svg, format: "svg")` inline before the title. Unknown callout kinds fall back gracefully — no icon, no panic.

**Tech Stack:** Rust, Typst 0.13 (`image()` with `bytes()`), Lucide icon SVG paths.

---

## File Structure

| File | Change |
|---|---|
| `crates/core/src/renderer.rs` | Delete `callout_icon()`, fix `render_block`, add `_co-icons` dict + update `callout()` in both `BUILTIN_PREAMBLE` and `BUILTIN_TEMPLATE`, add `#[cfg(test)]` module |
| `crates/cli/src/world.rs` | Add callout compilation integration test to existing test module |

No other files change.

---

## Context you need

**How `render_typst` works:** It returns a complete Typst source string. When no template is used, it prepends `BUILTIN_PREAMBLE` (which defines `_co-colors` and `callout()`) then emits `#callout("kind", "title")[body]` calls for each callout block. The title string currently has an emoji prepended by Rust.

**The Typst `image()` API (Typst 0.13):** `image.decode()` is deprecated. The correct call is `image(bytes("…svg…"), format: "svg")`. `bytes("string")` converts a UTF-8 string to a `bytes` value.

**SVG single-quote rule:** The SVG content uses single-quoted attribute values (e.g. `stroke='#1d4ed8'`). This avoids conflicts with Typst string delimiters (`"`) inside the Rust raw string (`r##"…"##`).

**Core crate name:** `omd2typst-core` (Rust module name: `omd2typst_core`). Run tests with `cargo test -p omd2typst-core`.

**CLI crate name:** `omd2typst`. Run tests with `cargo test -p omd2typst`.

---

## Task 1: Write failing unit tests in the core crate

**Files:**
- Modify: `crates/core/src/renderer.rs` (add `mod tests` at the bottom)

- [ ] **Step 1: Append the test module to `renderer.rs`**

Add at the very end of `crates/core/src/renderer.rs` (after line 630):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_markdown, RenderOptions};

    fn render_callout(md: &str) -> String {
        let doc = parse_markdown(md);
        render_typst(&doc, None, &RenderOptions::default())
    }

    #[test]
    fn callout_title_has_no_emoji() {
        // The #callout() call in the output must pass a bare title — no emoji prefix.
        let out = render_callout("> [!note] My Note\n> body text\n");
        assert!(
            out.contains("#callout(\"note\", \"My Note\")["),
            "Expected bare title in callout call:\n{out}"
        );
    }

    #[test]
    fn callout_emits_svg_icon_in_preamble() {
        // The preamble must define an image(bytes(...), format: "svg") for icons.
        let out = render_callout("> [!note] My Note\n> body text\n");
        assert!(
            out.contains("image(bytes("),
            "Expected image(bytes( in preamble:\n{out}"
        );
        assert!(
            out.contains("format: \"svg\""),
            "Expected format: \"svg\" in preamble:\n{out}"
        );
    }

    #[test]
    fn callout_unknown_kind_bare_title_no_panic() {
        // Unknown kinds must still render without emoji and without panicking.
        let out = render_callout("> [!foobar] Unknown\n> body\n");
        assert!(
            out.contains("#callout(\"foobar\", \"Unknown\")["),
            "Expected bare title for unknown kind:\n{out}"
        );
    }
}
```

- [ ] **Step 2: Run the tests to confirm they all fail**

```bash
cargo test -p omd2typst-core 2>&1 | tail -30
```

Expected: 2–3 test failures. `callout_title_has_no_emoji` fails because the current output has `"📝 My Note"`. `callout_emits_svg_icon_in_preamble` fails because no `image(bytes(` exists in the preamble yet. `callout_unknown_kind_bare_title_no_panic` passes (unknown kind already gets bare title via the `fmt!("{} {}", icon, title)` with `📌` — wait, actually it will have `"📌 Unknown"` so this test fails too).

- [ ] **Step 3: Commit the failing tests**

```bash
git add crates/core/src/renderer.rs
git commit -m "test(core): add failing tests for SVG callout icons"
```

---

## Task 2: Remove emoji from the Rust title string

**Files:**
- Modify: `crates/core/src/renderer.rs:296-313` (delete `callout_icon`), `renderer.rs:366-374` (fix `render_block`)

This makes `callout_title_has_no_emoji` and `callout_unknown_kind_bare_title_no_panic` pass. `callout_emits_svg_icon_in_preamble` still fails.

- [ ] **Step 1: Delete the `callout_icon` function**

Remove lines 292–313 entirely (the comment block, the function signature, and its body):

```rust
// ---------------------------------------------------------------------------
// Callout icon mapping
// ---------------------------------------------------------------------------

fn callout_icon(kind: &str) -> &'static str {
    match kind {
        "note"      => "📝",
        "info"      => "📖",
        "tip"       => "💡",
        "hint"      => "💡",
        "important" => "❗",
        "warning"   => "⚠️",
        "caution"   => "⚠️",
        "attention" => "⚠️",
        "danger"    => "🔥",
        "error"     => "❌",
        "bug"       => "🐛",
        "quote"     => "💬",
        "cite"      => "💬",
        _           => "📌",
    }
}
```

Delete all of the above. Nothing replaces it.

- [ ] **Step 2: Fix the `render_block` callout arm**

Find this block (around line 366 after deletion adjustment):

```rust
        Block::Callout { kind, title, body } => {
            let icon = callout_icon(kind);
            out.push_str(&format!("#callout({}, {})[\n",
                typst_string_val(kind),
                typst_string_val(&format!("{} {}", icon, title))));
            for b in body {
                render_block(out, b, level_offset);
            }
            out.push_str("]\n\n");
        }
```

Replace with:

```rust
        Block::Callout { kind, title, body } => {
            out.push_str(&format!("#callout({}, {})[\n",
                typst_string_val(kind),
                typst_string_val(title)));
            for b in body {
                render_block(out, b, level_offset);
            }
            out.push_str("]\n\n");
        }
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p omd2typst-core
```

Expected: no errors.

- [ ] **Step 4: Run tests — two should now pass**

```bash
cargo test -p omd2typst-core 2>&1 | tail -20
```

Expected: `callout_title_has_no_emoji` PASS, `callout_unknown_kind_bare_title_no_panic` PASS, `callout_emits_svg_icon_in_preamble` FAIL.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/renderer.rs
git commit -m "feat(core): remove emoji prefix from callout titles"
```

---

## Task 3: Add `_co-icons` dict and update `callout()` in `BUILTIN_PREAMBLE`

**Files:**
- Modify: `crates/core/src/renderer.rs:143-149` (the `BUILTIN_PREAMBLE` const string)

This makes all three unit tests pass.

- [ ] **Step 1: Replace the `callout()` definition in `BUILTIN_PREAMBLE`**

Find this block inside the `BUILTIN_PREAMBLE` const string (lines 143–149):

```typst
#let callout(kind, title, body) = {
  let c = _co-colors.at(kind, default: (fill: rgb("#f3f4f6"), accent: rgb("#374151")))
  block(fill: c.fill, inset: (x: 12pt, y: 10pt), radius: 4pt, width: 100%)[
    #text(fill: c.accent, weight: "bold")[#title] \
    #body
  ]
}
```

Replace with the `_co-icons` dict followed by the updated `callout()`:

```typst
#let _co-icons = (
  "note":      bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#1d4ed8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/></svg>"),
  "info":      bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#1d4ed8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/></svg>"),
  "tip":       bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#15803d' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5'/><path d='M9 18h6'/><path d='M10 22h4'/></svg>"),
  "hint":      bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#15803d' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5'/><path d='M9 18h6'/><path d='M10 22h4'/></svg>"),
  "important": bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#15803d' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><line x1='12' y1='8' x2='12' y2='12'/><line x1='12' y1='16' x2='12.01' y2='16'/></svg>"),
  "warning":   bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#a16207' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/></svg>"),
  "caution":   bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#a16207' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/></svg>"),
  "attention": bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#a16207' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/></svg>"),
  "danger":    bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#b91c1c' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z'/></svg>"),
  "error":     bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#b91c1c' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><path d='m15 9-6 6'/><path d='m9 9 6 6'/></svg>"),
  "bug":       bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#b91c1c' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m8 2 1.88 1.88'/><path d='M14.12 3.88 16 2'/><path d='M9 7.13v-1a3.003 3.003 0 1 1 6 0v1'/><path d='M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6z'/><path d='M12 20v-9'/><path d='M6.53 9C4.6 8.8 3 7.1 3 5'/><path d='M6 13H2'/><path d='M3 21c0-2.1 1.7-3.9 4-4'/><path d='M20.97 5c0 2.1-1.6 3.8-3.5 4'/><path d='M22 13h-4'/><path d='M17 17c2.3.1 4 1.9 4 4'/></svg>"),
  "quote":     bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#475569' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z'/><path d='M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z'/></svg>"),
  "cite":      bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#475569' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z'/><path d='M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z'/></svg>"),
)
#let callout(kind, title, body) = {
  let c = _co-colors.at(kind, default: (fill: rgb("#f3f4f6"), accent: rgb("#374151")))
  let svg = _co-icons.at(kind, default: none)
  let hdr = if svg != none {
    box(height: 0.75em, baseline: 20%, image(svg, format: "svg")) + h(4pt) + title
  } else { title }
  block(fill: c.fill, inset: (x: 12pt, y: 10pt), radius: 4pt, width: 100%)[
    #text(fill: c.accent, weight: "bold")[#hdr] \
    #body
  ]
}
```

- [ ] **Step 2: Run all core tests — all three must pass**

```bash
cargo test -p omd2typst-core 2>&1 | tail -20
```

Expected output:
```
test tests::callout_emits_svg_icon_in_preamble ... ok
test tests::callout_title_has_no_emoji ... ok
test tests::callout_unknown_kind_bare_title_no_panic ... ok

test result: ok. 3 passed; 0 failed
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/renderer.rs
git commit -m "feat(core): add SVG icons to BUILTIN_PREAMBLE callout function"
```

---

## Task 4: Mirror the changes in `BUILTIN_TEMPLATE`

**Files:**
- Modify: `crates/core/src/renderer.rs:207-216` (inside `BUILTIN_TEMPLATE` const string)

`BUILTIN_TEMPLATE` is the file exported by `--export-template`. It has its own `_co-colors` and `callout()`. No new unit tests are needed here — the integration test in Task 5 will compile a document that uses `BUILTIN_TEMPLATE` via the template path.

- [ ] **Step 1: Update `BUILTIN_TEMPLATE` — insert `_co-icons` and replace `callout()`**

Find this block inside the `BUILTIN_TEMPLATE` const string (around lines 207–216 in the source, between the `_co-colors` closing `)` and the `// template(doc)` comment):

```typst
// callout(kind, title)[body]
//   kind  — lowercase type: "note", "warning", …
//   title — display title string
#let callout(kind, title, body) = {
  let c = _co-colors.at(kind, default: (fill: rgb("#f3f4f6"), accent: rgb("#374151")))
  block(fill: c.fill, inset: (x: 12pt, y: 10pt), radius: 4pt, width: 100%)[
    #text(fill: c.accent, weight: "bold")[#title] \
    #body
  ]
}
```

Replace with:

```typst
// callout(kind, title)[body]
//   kind  — lowercase type: "note", "warning", …
//   title — display title string (no emoji prefix — icon is rendered by this function)
#let _co-icons = (
  "note":      bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#1d4ed8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/></svg>"),
  "info":      bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#1d4ed8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><line x1='12' y1='16' x2='12' y2='12'/><line x1='12' y1='8' x2='12.01' y2='8'/></svg>"),
  "tip":       bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#15803d' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5'/><path d='M9 18h6'/><path d='M10 22h4'/></svg>"),
  "hint":      bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#15803d' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5'/><path d='M9 18h6'/><path d='M10 22h4'/></svg>"),
  "important": bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#15803d' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><line x1='12' y1='8' x2='12' y2='12'/><line x1='12' y1='16' x2='12.01' y2='16'/></svg>"),
  "warning":   bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#a16207' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/></svg>"),
  "caution":   bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#a16207' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/></svg>"),
  "attention": bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#a16207' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3'/><line x1='12' y1='9' x2='12' y2='13'/><line x1='12' y1='17' x2='12.01' y2='17'/></svg>"),
  "danger":    bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#b91c1c' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z'/></svg>"),
  "error":     bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#b91c1c' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><circle cx='12' cy='12' r='10'/><path d='m15 9-6 6'/><path d='m9 9 6 6'/></svg>"),
  "bug":       bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#b91c1c' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m8 2 1.88 1.88'/><path d='M14.12 3.88 16 2'/><path d='M9 7.13v-1a3.003 3.003 0 1 1 6 0v1'/><path d='M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6z'/><path d='M12 20v-9'/><path d='M6.53 9C4.6 8.8 3 7.1 3 5'/><path d='M6 13H2'/><path d='M3 21c0-2.1 1.7-3.9 4-4'/><path d='M20.97 5c0 2.1-1.6 3.8-3.5 4'/><path d='M22 13h-4'/><path d='M17 17c2.3.1 4 1.9 4 4'/></svg>"),
  "quote":     bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#475569' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z'/><path d='M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z'/></svg>"),
  "cite":      bytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='#475569' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V20c0 1 0 1 1 1z'/><path d='M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3c0 1 0 1 1 1z'/></svg>"),
)
#let callout(kind, title, body) = {
  let c = _co-colors.at(kind, default: (fill: rgb("#f3f4f6"), accent: rgb("#374151")))
  let svg = _co-icons.at(kind, default: none)
  let hdr = if svg != none {
    box(height: 0.75em, baseline: 20%, image(svg, format: "svg")) + h(4pt) + title
  } else { title }
  block(fill: c.fill, inset: (x: 12pt, y: 10pt), radius: 4pt, width: 100%)[
    #text(fill: c.accent, weight: "bold")[#hdr] \
    #body
  ]
}
```

- [ ] **Step 2: Verify it compiles cleanly**

```bash
cargo check -p omd2typst-core && cargo test -p omd2typst-core 2>&1 | tail -10
```

Expected: no errors, all 3 tests still pass.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/renderer.rs
git commit -m "feat(core): add SVG icons to BUILTIN_TEMPLATE callout function"
```

---

## Task 5: CLI integration test — compile a callout document end-to-end

**Files:**
- Modify: `crates/cli/src/world.rs` (add test to the existing `mod tests` block at line 197)

This verifies that the `bytes()` + `image()` pipeline works through the embedded Typst 0.13 compiler without errors.

- [ ] **Step 1: Add the integration test to the existing `mod tests` block in `world.rs`**

The current test module ends at line 231 with `}`. Add the new test before the closing `}`:

```rust
    #[test]
    fn world_compiles_callout_document() {
        use omd2typst_core::{parse_markdown, render_typst, RenderOptions};

        // One callout from each colour group: blue, green, yellow, red, grey.
        let md = "\
> [!note] A Note\n\
> This is a note.\n\
\n\
> [!tip] A Tip\n\
> This is a tip.\n\
\n\
> [!warning] A Warning\n\
> Be careful.\n\
\n\
> [!danger] Danger\n\
> Watch out.\n\
\n\
> [!quote] A Quote\n\
> Someone said something.\n\
";
        let doc = parse_markdown(md);
        let typst_src = render_typst(&doc, None, &RenderOptions::default());

        let world = OmdWorld::new(std::env::current_dir().unwrap(), typst_src);
        let result = typst::compile::<typst::layout::PagedDocument>(&world);
        assert!(
            result.output.is_ok(),
            "Typst compilation failed: {:?}",
            result.output.err()
        );
    }
```

- [ ] **Step 2: Run the CLI tests**

```bash
cargo test -p omd2typst 2>&1 | tail -20
```

Expected output:
```
test tests::liberation_fonts_load ... ok
test tests::system_font_dirs_is_non_empty ... ok
test tests::world_compiles_callout_document ... ok
test tests::world_compiles_minimal_document ... ok

test result: ok. 4 passed; 0 failed
```

If `world_compiles_callout_document` fails with a Typst compile error, the error message will contain the Typst diagnostic. Common issues: malformed SVG (check that single quotes are consistent throughout the `bytes("...")` strings), or incorrect `image()` call syntax.

- [ ] **Step 3: Run the full test suite to confirm no regressions**

```bash
cargo test --workspace 2>&1 | tail -20
```

Expected: all tests pass across all crates.

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/world.rs
git commit -m "test(cli): verify callout SVG icons compile through Typst"
```

---

## Done

All tasks complete when:
- `cargo test --workspace` passes with 0 failures
- The 3 unit tests in `omd2typst-core` pass
- The integration test `world_compiles_callout_document` passes
- No emoji characters appear in the rendered Typst output for any callout kind
