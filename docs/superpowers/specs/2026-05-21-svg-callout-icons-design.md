# SVG Callout Icons Design

> **For agentic workers:** Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this spec task-by-task.

**Goal:** Replace emoji prefixes in callout block titles with inline Lucide SVG icons that match each callout's accent colour.

**Architecture:** SVG bytes are embedded in the `_co-icons` Typst dict inside `BUILTIN_PREAMBLE` and `BUILTIN_TEMPLATE`. The `callout()` Typst function renders the icon inline before the title using `image(svg, format: "svg")`. Rust stops prepending emoji to the title string.

**Tech Stack:** Rust (renderer const strings), Typst 0.13 (`image()` with `bytes()`), Lucide icon paths.

---

## Scope

Changes are confined to `crates/core/src/renderer.rs`. No other crate, binary, or plugin layer is touched. The WASM, CLI, and Obsidian plugin pass through unchanged.

Custom templates are unaffected by design: they import and define their own `callout()` function. They now receive a clean title string (no emoji prefix) and can add whatever icons they like. The emoji removal is the only externally visible behaviour change for custom template users.

---

## Icon Mapping

All icons are Lucide line style — thin stroke, no fill, `stroke-width="2"`, `stroke-linecap="round"`, `stroke-linejoin="round"`. The `stroke` colour is the callout type's accent colour, hardcoded in the SVG (no dynamic substitution required).

| Kind | Icon | Accent colour |
|---|---|---|
| note | `info` (circle + i dot) | `#1d4ed8` |
| info | `info` | `#1d4ed8` |
| tip | `lightbulb` | `#15803d` |
| hint | `lightbulb` | `#15803d` |
| important | `circle-alert` (circle + ! mark) | `#15803d` |
| warning | `triangle-alert` | `#a16207` |
| caution | `triangle-alert` | `#a16207` |
| attention | `triangle-alert` | `#a16207` |
| danger | `flame` | `#b91c1c` |
| error | `circle-x` | `#b91c1c` |
| bug | `bug` | `#b91c1c` |
| quote | `quote` | `#475569` |
| cite | `quote` | `#475569` |
| _(unknown)_ | _(none — title only)_ | `#374151` |

---

## Rust Changes (`renderer.rs`)

### 1. Delete `callout_icon()`

Remove the function entirely.

### 2. Remove emoji from title in `render_block`

The `Block::Callout` arm currently does:

```rust
let icon = callout_icon(kind);
out.push_str(&format!("#callout({}, {})[\n",
    typst_string_val(kind),
    typst_string_val(&format!("{} {}", icon, title))));
```

Change to:

```rust
out.push_str(&format!("#callout({}, {})[\n",
    typst_string_val(kind),
    typst_string_val(title)));
```

### 3. Add `_co-icons` dict to `BUILTIN_PREAMBLE`

Insert immediately after the `_co-colors` dict. Each SVG uses single-quoted attributes (valid XML, no escaping needed inside the Rust raw string `r##"..."##`).

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
```

### 4. Update `callout()` in `BUILTIN_PREAMBLE`

Replace the existing function:

```typst
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

### 5. Apply identical changes to `BUILTIN_TEMPLATE`

`BUILTIN_TEMPLATE` in `renderer.rs` already has a `_co-colors` dict near the top of the template string (around the `// Callout colours` comment). Insert `_co-icons` on the line immediately following the closing `)` of `_co-colors`. Then replace the existing `callout()` function body with the same updated version from step 4.

---

## Typst API Note

`image.decode()` is deprecated as of Typst 0.13. The correct API is:

```typst
image(bytes("…svg string…"), format: "svg")
```

`bytes()` converts a Typst string to a `bytes` value. `image()` accepts bytes directly as its first positional argument.

---

## Testing

### Unit tests (render output)

In `crates/core/tests/` (or the inline `#[cfg(test)]` block in `renderer.rs`):

- For each of the 13 callout kinds: assert the generated Typst string contains `image(bytes(` and does **not** contain any emoji character in the callout output.
- For an unknown callout kind (e.g. `"foobar"`): assert no `image(` call is emitted and no panic occurs.
- Assert the rendered title string is the bare title with no emoji prefix (e.g. `"My Note"` not `"📝 My Note"`).

### Integration test (PDF compilation)

In `crates/cli/tests/` or the existing `world_compiles_minimal_document` test: add a document containing one callout of each colour group (blue, green, yellow, red, grey). Assert compilation succeeds without error. This verifies the `bytes()` + `image()` pipeline works end-to-end through the embedded Typst compiler.

---

## Non-Goals

- Configurable icon opt-out via `RenderOptions` (YAGNI — add if a user requests it)
- Custom icon injection by template authors (they define their own `callout()` and can do whatever they like)
- Icon support for custom templates (they receive a clean title and own their own rendering)
