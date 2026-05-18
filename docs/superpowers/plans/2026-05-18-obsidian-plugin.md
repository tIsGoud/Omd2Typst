# Obsidian Plugin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `obsidian-omd2typst`, an official Obsidian community plugin that exports notes to Typst source or PDF using two bundled WASM modules — omd2typst (MD→Typst) and typst-ts (Typst→PDF).

**Architecture:** The existing `omd2typst` Rust repo gains a `src/lib.rs` WASM entry point compiled via `wasm-pack`; a new `obsidian-omd2typst` TypeScript repo consumes it as a git submodule. The omd2typst WASM is inlined into `main.js` at build time; the larger Typst WASM (~20 MB) ships as a separate file in the plugin directory and is loaded at runtime via the Obsidian vault adapter.

**Tech Stack:** Rust + wasm-bindgen + wasm-pack; TypeScript + Obsidian API; esbuild + esbuild-plugin-wasm; `@myriaddreamin/typst-ts-web-compiler`; Jest + ts-jest.

---

## File Map

### Phase 1 — omd2typst Rust repo (changes to existing repo)

| File | Action | Purpose |
|---|---|---|
| `src/lib.rs` | Create | WASM entry point: exports `render_to_typst` and `get_builtin_template` |
| `Cargo.toml` | Modify | Add `[lib]` cdylib target and `wasm-bindgen` dependency |

### Phase 2 — obsidian-omd2typst (new repo)

| File | Action | Purpose |
|---|---|---|
| `manifest.json` | Create | Obsidian plugin metadata |
| `package.json` | Create | npm deps and build scripts |
| `tsconfig.json` | Create | TypeScript compiler config |
| `esbuild.config.mjs` | Create | Bundle src/ + inline omd2typst WASM → main.js; copy typst WASM |
| `scripts/build-wasm.sh` | Create | Runs wasm-pack inside the submodule |
| `.gitignore` | Create | Excludes node_modules, main.js, wasm build artifacts, .superpowers/ |
| `libs/omd2typst/` | Submodule | Pinned to current omd2typst commit |
| `src/__mocks__/obsidian.ts` | Create | Obsidian API mock for Jest |
| `src/__mocks__/wasm.ts` | Create | WASM module mock for Jest |
| `src/settings.ts` | Create | Settings types, DEFAULT_SETTINGS, SettingTab UI |
| `src/template.ts` | Create | Parse template language declarations; resolve active template |
| `src/frontmatter.ts` | Create | Parse, merge, and insert frontmatter blocks |
| `src/output.ts` | Create | Resolve output file path for all three output-location modes |
| `src/wasm/omd2typst.ts` | Create | Lazy-init wrapper around omd2typst WASM |
| `src/wasm/typst.ts` | Create | Lazy-init wrapper around Typst WASM compiler |
| `src/exporter.ts` | Create | Orchestrates the full MD → Typst → (PDF) pipeline |
| `src/main.ts` | Create | Plugin entry point; registers commands, context menus, settings tab |
| `tests/template.test.ts` | Create | Unit tests for template.ts |
| `tests/frontmatter.test.ts` | Create | Unit tests for frontmatter.ts |
| `tests/output.test.ts` | Create | Unit tests for output.ts |
| `tests/exporter.test.ts` | Create | Integration test for exporter.ts with mocked WASM |

---

## Task 1: WASM API in the Rust repo

**Repo:** `omd2typst`

**Files:**
- Create: `src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add wasm-bindgen to Cargo.toml and declare the lib target**

Open `Cargo.toml` and add:

```toml
[package]
name = "omd2typst"
version = "0.4.0"
edition = "2021"
description = "Convert Obsidian Markdown notes to Typst/PDF via an AST pipeline"
license = "MIT"
rust-version = "1.70"

[lib]
crate-type = ["cdylib", "rlib"]

[[bin]]
name = "omd2typst"
path = "src/main.rs"

[dependencies]
comrak = "0.28"
anyhow = "1.0"
clap = { version = "4.5", features = ["derive"] }
regex = "1.10"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
```

The `rlib` crate type keeps unit tests working; `cdylib` is required for wasm-pack.

- [ ] **Step 2: Create src/lib.rs**

```rust
use wasm_bindgen::prelude::*;

mod ast;
mod parser;
mod renderer;

/// Convert Markdown to a Typst source string.
/// Pass `template_src` as the full text of a .typ template, or `None` to use
/// the built-in template.
#[wasm_bindgen]
pub fn render_to_typst(markdown: &str, template_src: Option<String>) -> String {
    let doc = parser::parse_markdown(markdown);
    renderer::render_typst(&doc, template_src.as_deref())
}

/// Return the built-in Typst template source so the plugin can offer
/// "Export built-in template" without embedding it in TypeScript.
#[wasm_bindgen]
pub fn get_builtin_template() -> String {
    renderer::BUILTIN_TEMPLATE.to_string()
}
```

- [ ] **Step 3: Verify the native build still compiles**

```bash
cargo build
```

Expected: `Finished dev profile` with no errors. The CLI binary is unchanged.

- [ ] **Step 4: Install wasm-pack (if not already installed)**

```bash
cargo install wasm-pack
```

Expected: `Installed package wasm-pack` or `wasm-pack v... is already installed`.

- [ ] **Step 5: Build the WASM and verify the exports are present**

```bash
wasm-pack build --target bundler --out-dir /tmp/omd2typst-wasm-check
grep -l "render_to_typst\|get_builtin_template" /tmp/omd2typst-wasm-check/omd2typst.d.ts
```

Expected: the `.d.ts` file is found, confirming both functions are exported.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs Cargo.toml Cargo.lock
git commit -m "feat: add WASM API (render_to_typst, get_builtin_template) via wasm-bindgen"
```

---

## Task 2: Scaffold the plugin repo

**Repo:** new directory `obsidian-omd2typst` (sibling of or separate from the Rust repo)

**Files:**
- Create: `manifest.json`, `package.json`, `tsconfig.json`, `esbuild.config.mjs`, `scripts/build-wasm.sh`, `.gitignore`
- Add submodule: `libs/omd2typst/`

- [ ] **Step 1: Initialise the git repo**

```bash
mkdir obsidian-omd2typst && cd obsidian-omd2typst
git init
```

- [ ] **Step 2: Create manifest.json**

```json
{
  "id": "omd2typst",
  "name": "omd2typst",
  "version": "0.1.0",
  "minAppVersion": "1.4.0",
  "description": "Export Obsidian notes to Typst source or PDF",
  "author": "A.W. Alberts",
  "authorUrl": "",
  "isDesktopOnly": true
}
```

- [ ] **Step 3: Create package.json**

```json
{
  "name": "obsidian-omd2typst",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "build:wasm": "bash scripts/build-wasm.sh",
    "build": "node esbuild.config.mjs production",
    "dev": "node esbuild.config.mjs",
    "test": "jest"
  },
  "devDependencies": {
    "@types/jest": "^29.5.0",
    "@types/node": "^20.0.0",
    "builtin-modules": "^3.3.0",
    "esbuild": "^0.21.0",
    "esbuild-plugin-wasm": "^1.1.0",
    "jest": "^29.5.0",
    "obsidian": "latest",
    "ts-jest": "^29.5.0",
    "typescript": "^5.4.0"
  },
  "dependencies": {
    "@myriaddreamin/typst-ts-web-compiler": "^0.5.0"
  }
}
```

- [ ] **Step 4: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2018",
    "module": "commonjs",
    "lib": ["ES2018", "DOM"],
    "strict": true,
    "moduleResolution": "node",
    "esModuleInterop": true,
    "resolveJsonModule": true,
    "outDir": "dist",
    "rootDir": "src",
    "skipLibCheck": true
  },
  "include": ["src/**/*.ts", "tests/**/*.ts"],
  "exclude": ["node_modules"]
}
```

- [ ] **Step 5: Create jest.config.js**

```javascript
module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'node',
  roots: ['<rootDir>/tests'],
  moduleNameMapper: {
    '\\.wasm$': '<rootDir>/src/__mocks__/wasm.ts',
    '^obsidian$': '<rootDir>/src/__mocks__/obsidian.ts',
  },
};
```

- [ ] **Step 6: Create esbuild.config.mjs**

```javascript
import esbuild from 'esbuild';
import { wasmLoader } from 'esbuild-plugin-wasm';
import builtins from 'builtin-modules';
import { copyFile, mkdir } from 'fs/promises';
import process from 'process';

const prod = process.argv[2] === 'production';

// Copy the large Typst WASM to the plugin root so it can be loaded at runtime.
// The omd2typst WASM is small enough to inline via wasmLoader().
await mkdir('wasm-runtime', { recursive: true });
await copyFile(
  'node_modules/@myriaddreamin/typst-ts-web-compiler/pkg/typst_ts_web_compiler_bg.wasm',
  'wasm-runtime/typst_compiler.wasm',
);

const OBSIDIAN_EXTERNALS = [
  'obsidian', 'electron',
  '@codemirror/autocomplete', '@codemirror/collab', '@codemirror/commands',
  '@codemirror/language', '@codemirror/lint', '@codemirror/search',
  '@codemirror/state', '@codemirror/view',
  '@lezer/common', '@lezer/highlight', '@lezer/lr',
  ...builtins,
];

await esbuild.build({
  entryPoints: ['src/main.ts'],
  bundle: true,
  external: OBSIDIAN_EXTERNALS,
  format: 'cjs',
  target: 'es2018',
  sourcemap: prod ? false : 'inline',
  treeShaking: true,
  outfile: 'main.js',
  minify: prod,
  plugins: [wasmLoader()],  // inlines omd2typst WASM as base64 in main.js
  logLevel: 'info',
});
```

- [ ] **Step 7: Create scripts/build-wasm.sh**

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SUBMODULE="$REPO_ROOT/libs/omd2typst"
OUT="$REPO_ROOT/src/wasm/omd2typst-pkg"

cd "$SUBMODULE"
wasm-pack build --target bundler --out-dir "$OUT"
echo "✓ omd2typst WASM written to $OUT"
```

```bash
chmod +x scripts/build-wasm.sh
```

- [ ] **Step 8: Create .gitignore**

```
node_modules/
main.js
main.js.map
dist/
wasm-runtime/
src/wasm/omd2typst-pkg/
.superpowers/
```

- [ ] **Step 9: Add the omd2typst repo as a git submodule**

Run this from the `obsidian-omd2typst` root, substituting the actual path or URL to your omd2typst repo:

```bash
git submodule add <path-or-url-to-omd2typst-repo> libs/omd2typst
```

Example (local path):
```bash
git submodule add ../omd2typst libs/omd2typst
```

- [ ] **Step 10: Install npm dependencies**

```bash
npm install
```

Expected: `node_modules/` populated, no errors.

- [ ] **Step 11: Commit the scaffold**

```bash
git add .
git commit -m "chore: scaffold obsidian-omd2typst plugin repo"
```

---

## Task 3: Mocks and settings model

**Files:**
- Create: `src/__mocks__/obsidian.ts`
- Create: `src/__mocks__/wasm.ts`
- Create: `src/settings.ts`

- [ ] **Step 1: Create src/__mocks__/obsidian.ts**

```typescript
export class Plugin {
  app: any = {};
  manifest: any = { dir: '.obsidian/plugins/omd2typst', id: 'omd2typst' };
  addCommand = jest.fn();
  addSettingTab = jest.fn();
  registerEvent = jest.fn();
  loadData = jest.fn().mockResolvedValue({});
  saveData = jest.fn().mockResolvedValue(undefined);
}

export class PluginSettingTab {
  containerEl: any = {
    empty: jest.fn(),
    createEl: jest.fn().mockReturnValue({ createEl: jest.fn() }),
  };
  constructor(public app: any, public plugin: any) {}
  display() {}
}

export class TFile {
  path: string;
  extension: string;
  basename: string;
  name: string;
  parent: { path: string } | null;
  constructor(path: string) {
    this.path = path;
    const parts = path.split('/');
    this.name = parts[parts.length - 1];
    this.extension = this.name.includes('.') ? this.name.split('.').pop()! : '';
    this.basename = this.name.replace(/\.[^.]+$/, '');
    this.parent = parts.length > 1 ? { path: parts.slice(0, -1).join('/') } : null;
  }
}

export class Notice {
  constructor(public message: string, public timeout?: number) {}
}

export class Setting {
  constructor(containerEl: any) {}
  setName = jest.fn().mockReturnThis();
  setDesc = jest.fn().mockReturnThis();
  addText = jest.fn().mockReturnThis();
  addDropdown = jest.fn().mockReturnThis();
  addToggle = jest.fn().mockReturnThis();
  addButton = jest.fn().mockReturnThis();
  addTextArea = jest.fn().mockReturnThis();
}
```

- [ ] **Step 2: Create src/__mocks__/wasm.ts**

```typescript
// Used by Jest moduleNameMapper for any *.wasm import.
// WASM modules are tested via their TypeScript wrappers with mocked internals.
export default {};
```

- [ ] **Step 3: Create src/settings.ts** (types and defaults only — UI added in Task 10)

```typescript
export type OutputFormat = 'typ' | 'pdf';
export type OutputMode  = 'same-folder' | 'fixed-folder' | 'ask';
export type FrontmatterTemplateMode = 'inline' | 'file';

export interface TemplateEntry {
  name: string;
  /** Vault-relative path to the .typ file. Empty string for the built-in template. */
  path: string;
  /** Populated by parseTemplateLanguages(); empty if no declaration found. */
  languages: string[];
}

export interface Omd2TypstSettings {
  templates: TemplateEntry[];
  /** Name of the active default template, or 'built-in'. */
  defaultTemplate: string;
  defaultOutputFormat: OutputFormat;
  outputMode: OutputMode;
  /** Vault-relative folder used when outputMode === 'fixed-folder'. */
  outputFolder: string;
  /** Applied when the note frontmatter has no 'language:' key. */
  defaultLanguage: string;
  frontmatterTemplateMode: FrontmatterTemplateMode;
  /** YAML lines (no --- delimiters) used in inline mode. */
  frontmatterInline: string;
  /** Vault-relative path to a .md file used in file mode. */
  frontmatterFilePath: string;
}

export const DEFAULT_SETTINGS: Omd2TypstSettings = {
  templates: [],
  defaultTemplate: 'built-in',
  defaultOutputFormat: 'pdf',
  outputMode: 'same-folder',
  outputFolder: 'exports',
  defaultLanguage: 'en',
  frontmatterTemplateMode: 'inline',
  frontmatterInline: [
    'title:',
    'subtitle:',
    'author:',
    'date:',
    'version:',
    'status:',
    'language:',
    'summary:',
    'figure-list:',
    'revision-table:',
    'approval-table:',
  ].join('\n'),
  frontmatterFilePath: '',
};
```

- [ ] **Step 4: Commit**

```bash
git add src/
git commit -m "feat: add settings model and Jest mocks"
```

---

## Task 4: Template management

**Files:**
- Create: `src/template.ts`
- Create: `tests/template.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `tests/template.test.ts`:

```typescript
import {
  parseTemplateLanguages,
  checkLanguageCompatibility,
  resolveDefaultTemplate,
} from '../src/template';
import type { TemplateEntry, Omd2TypstSettings } from '../src/settings';

const SETTINGS_BASE: Omd2TypstSettings = {
  templates: [],
  defaultTemplate: 'built-in',
  defaultOutputFormat: 'pdf',
  outputMode: 'same-folder',
  outputFolder: 'exports',
  defaultLanguage: 'en',
  frontmatterTemplateMode: 'inline',
  frontmatterInline: '',
  frontmatterFilePath: '',
};

describe('parseTemplateLanguages', () => {
  it('returns empty array when no declaration present', () => {
    expect(parseTemplateLanguages('#set page(paper: "a4")')).toEqual([]);
  });

  it('parses a single language', () => {
    expect(parseTemplateLanguages('// omd2typst-languages: nl')).toEqual(['nl']);
  });

  it('parses multiple languages and trims whitespace', () => {
    const src = '// page setup\n// omd2typst-languages: nl, en\n#set text(size: 11pt)';
    expect(parseTemplateLanguages(src)).toEqual(['nl', 'en']);
  });
});

describe('checkLanguageCompatibility', () => {
  const entry: TemplateEntry = { name: 'DUO', path: 'tmpl.typ', languages: ['nl', 'en'] };

  it('returns null when languages match', () => {
    expect(checkLanguageCompatibility(entry, 'nl')).toBeNull();
  });

  it('returns warning message on mismatch', () => {
    const msg = checkLanguageCompatibility(entry, 'fr');
    expect(msg).toContain('DUO');
    expect(msg).toContain('nl, en');
    expect(msg).toContain('fr');
  });

  it('returns null when template has no language declaration', () => {
    const noLang: TemplateEntry = { name: 'Plain', path: 'p.typ', languages: [] };
    expect(checkLanguageCompatibility(noLang, 'fr')).toBeNull();
  });
});

describe('resolveDefaultTemplate', () => {
  it('returns null (built-in) when defaultTemplate is built-in', () => {
    expect(resolveDefaultTemplate(SETTINGS_BASE)).toBeNull();
  });

  it('returns the matching TemplateEntry when found', () => {
    const tmpl: TemplateEntry = { name: 'DUO', path: 'tmpl.typ', languages: [] };
    const settings = { ...SETTINGS_BASE, templates: [tmpl], defaultTemplate: 'DUO' };
    expect(resolveDefaultTemplate(settings)).toBe(tmpl);
  });

  it('returns null when defaultTemplate name is not in list', () => {
    const settings = { ...SETTINGS_BASE, defaultTemplate: 'Missing' };
    expect(resolveDefaultTemplate(settings)).toBeNull();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
npm test -- tests/template.test.ts
```

Expected: FAIL — `Cannot find module '../src/template'`

- [ ] **Step 3: Create src/template.ts**

```typescript
import type { TemplateEntry, Omd2TypstSettings } from './settings';

/** Parse the `// omd2typst-languages: nl, en` comment from a .typ file's content. */
export function parseTemplateLanguages(typContent: string): string[] {
  const match = typContent.match(/\/\/\s*omd2typst-languages:\s*(.+)/);
  if (!match) return [];
  return match[1].split(',').map(l => l.trim()).filter(Boolean);
}

/**
 * Check whether a template's declared languages include the note's language.
 * Returns a human-readable warning string on mismatch, or null if compatible.
 */
export function checkLanguageCompatibility(
  template: TemplateEntry,
  noteLanguage: string,
): string | null {
  if (template.languages.length === 0) return null;
  if (template.languages.includes(noteLanguage)) return null;
  return (
    `Template '${template.name}' supports ${template.languages.join(', ')} — ` +
    `note language is '${noteLanguage}'.`
  );
}

/** Returns the TemplateEntry to use, or null to signal the built-in template. */
export function resolveDefaultTemplate(settings: Omd2TypstSettings): TemplateEntry | null {
  if (settings.defaultTemplate === 'built-in') return null;
  return settings.templates.find(t => t.name === settings.defaultTemplate) ?? null;
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
npm test -- tests/template.test.ts
```

Expected: PASS — 7 tests.

- [ ] **Step 5: Commit**

```bash
git add src/template.ts tests/template.test.ts
git commit -m "feat: add template language parsing and resolution"
```

---

## Task 5: Frontmatter logic

**Files:**
- Create: `src/frontmatter.ts`
- Create: `tests/frontmatter.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `tests/frontmatter.test.ts`:

```typescript
import {
  parseFrontmatter,
  mergeFrontmatter,
  buildFrontmatterBlock,
} from '../src/frontmatter';

describe('parseFrontmatter', () => {
  it('returns null when no frontmatter present', () => {
    expect(parseFrontmatter('# Hello\nNo frontmatter here.')).toBeNull();
  });

  it('parses keys from a frontmatter block', () => {
    const note = '---\ntitle: My Doc\nauthor: Alice\n---\n# Body';
    const result = parseFrontmatter(note);
    expect(result?.keys).toEqual(['title', 'author']);
  });

  it('handles frontmatter with empty values', () => {
    const note = '---\ntitle:\nauthor:\n---\n';
    const result = parseFrontmatter(note);
    expect(result?.keys).toEqual(['title', 'author']);
  });
});

describe('mergeFrontmatter', () => {
  it('prepends missing keys above existing frontmatter', () => {
    const note = '---\nauthor: Alice\n---\n# Body';
    const result = mergeFrontmatter(note, ['title', 'author', 'date']);
    expect(result).toMatch(/^---\ntitle:\ndate:\nauthor: Alice\n---/);
  });

  it('does not duplicate keys already present', () => {
    const note = '---\ntitle: My Doc\n---\n# Body';
    const result = mergeFrontmatter(note, ['title', 'author']);
    const matches = result.match(/title/g)!;
    expect(matches).toHaveLength(1);
    expect(result).toContain('author:');
  });

  it('inserts full block when note has no frontmatter', () => {
    const note = '# Just a heading';
    const result = mergeFrontmatter(note, ['title', 'author']);
    expect(result).toMatch(/^---\ntitle:\nauthor:\n---\n/);
    expect(result).toContain('# Just a heading');
  });
});

describe('buildFrontmatterBlock', () => {
  it('converts YAML lines string to key array', () => {
    const yaml = 'title:\nauthor:\ndate:';
    expect(buildFrontmatterBlock(yaml)).toEqual(['title', 'author', 'date']);
  });

  it('ignores blank lines and comments', () => {
    const yaml = 'title:\n\n# a comment\nauthor:';
    expect(buildFrontmatterBlock(yaml)).toEqual(['title', 'author']);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
npm test -- tests/frontmatter.test.ts
```

Expected: FAIL — `Cannot find module '../src/frontmatter'`

- [ ] **Step 3: Create src/frontmatter.ts**

```typescript
const FM_DELIM = '---';

interface ParsedFrontmatter {
  keys: string[];
  /** Byte offset of the opening '---\n' (0). */
  openEnd: number;
  /** Byte offset just after the closing '---'. */
  closeEnd: number;
}

/** Extract the list of keys from a note's YAML frontmatter block. */
export function parseFrontmatter(content: string): ParsedFrontmatter | null {
  const trimmed = content.trimStart();
  if (!trimmed.startsWith(FM_DELIM)) return null;
  const afterOpen = trimmed.slice(3);
  const closeIdx = findFrontmatterClose(afterOpen);
  if (closeIdx === null) return null;

  const yamlBlock = afterOpen.slice(0, closeIdx);
  const keys = yamlBlock
    .split('\n')
    .map(l => l.split(':')[0].trim())
    .filter(k => k.length > 0 && !k.startsWith('#'));

  const openEnd  = 4; // '---\n'
  const closeEnd = 4 + closeIdx + 4; // '---\n' + yaml + '\n---'
  return { keys, openEnd, closeEnd };
}

/**
 * Merge template keys into the note's frontmatter.
 * Missing keys are prepended above existing keys; existing keys are untouched.
 * If the note has no frontmatter, a full block is inserted at the top.
 */
export function mergeFrontmatter(noteContent: string, templateKeys: string[]): string {
  const parsed = parseFrontmatter(noteContent);
  if (!parsed) {
    const block = `${FM_DELIM}\n${templateKeys.map(k => `${k}:`).join('\n')}\n${FM_DELIM}\n`;
    return block + noteContent;
  }

  const existingKeys = new Set(parsed.keys);
  const missingKeys = templateKeys.filter(k => !existingKeys.has(k));
  if (missingKeys.length === 0) return noteContent;

  // Insert missing keys immediately after the opening ---
  const insertLines = missingKeys.map(k => `${k}:`).join('\n') + '\n';
  return (
    `${FM_DELIM}\n` +
    insertLines +
    noteContent.slice(parsed.openEnd, parsed.closeEnd) +
    noteContent.slice(parsed.closeEnd)
  );
}

/** Convert the inline YAML-lines setting string to a list of key names. */
export function buildFrontmatterBlock(yamlLines: string): string[] {
  return yamlLines
    .split('\n')
    .map(l => l.split(':')[0].trim())
    .filter(k => k.length > 0 && !k.startsWith('#'));
}

function findFrontmatterClose(s: string): number | null {
  let from = 0;
  while (true) {
    const idx = s.indexOf('\n---', from);
    if (idx === -1) return null;
    const after = idx + 4;
    if (after >= s.length || s[after] === '\n' || s[after] === '\r') return idx;
    from = idx + 1;
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
npm test -- tests/frontmatter.test.ts
```

Expected: PASS — 8 tests.

- [ ] **Step 5: Commit**

```bash
git add src/frontmatter.ts tests/frontmatter.test.ts
git commit -m "feat: add frontmatter parse, merge, and key-extraction logic"
```

---

## Task 6: Output path resolution

**Files:**
- Create: `src/output.ts`
- Create: `tests/output.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `tests/output.test.ts`:

```typescript
import { resolveOutputPath } from '../src/output';
import type { Omd2TypstSettings } from '../src/settings';

const BASE: Omd2TypstSettings = {
  templates: [], defaultTemplate: 'built-in',
  defaultOutputFormat: 'pdf', outputMode: 'same-folder', outputFolder: 'exports',
  defaultLanguage: 'en', frontmatterTemplateMode: 'inline',
  frontmatterInline: '', frontmatterFilePath: '',
};

describe('resolveOutputPath (same-folder)', () => {
  it('replaces .md extension with .pdf', () => {
    const result = resolveOutputPath('notes/report.md', 'pdf', BASE);
    expect(result).toBe('notes/report.pdf');
  });

  it('replaces .md extension with .typ', () => {
    const result = resolveOutputPath('notes/report.md', 'typ', BASE);
    expect(result).toBe('notes/report.typ');
  });

  it('handles a note at vault root', () => {
    const result = resolveOutputPath('note.md', 'pdf', BASE);
    expect(result).toBe('note.pdf');
  });
});

describe('resolveOutputPath (fixed-folder)', () => {
  const settings = { ...BASE, outputMode: 'fixed-folder' as const, outputFolder: 'exports' };

  it('places output in the configured folder', () => {
    const result = resolveOutputPath('notes/deep/report.md', 'pdf', settings);
    expect(result).toBe('exports/report.pdf');
  });

  it('strips path separators from the folder setting', () => {
    const s = { ...settings, outputFolder: 'exports/' };
    expect(resolveOutputPath('note.md', 'pdf', s)).toBe('exports/note.pdf');
  });
});

describe('resolveOutputPath (ask)', () => {
  it('returns null to signal the caller should open a file picker', () => {
    const settings = { ...BASE, outputMode: 'ask' as const };
    expect(resolveOutputPath('note.md', 'pdf', settings)).toBeNull();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
npm test -- tests/output.test.ts
```

Expected: FAIL — `Cannot find module '../src/output'`

- [ ] **Step 3: Create src/output.ts**

```typescript
import type { Omd2TypstSettings, OutputFormat } from './settings';

/**
 * Resolve the vault-relative output path for an export.
 * Returns null when outputMode is 'ask' — the caller opens a file picker.
 */
export function resolveOutputPath(
  notePath: string,
  format: OutputFormat,
  settings: Omd2TypstSettings,
): string | null {
  const ext = `.${format}`;
  const basename = notePath.replace(/\.md$/, '');
  const filename = basename.split('/').pop()! + ext;

  switch (settings.outputMode) {
    case 'same-folder':
      return basename + ext;

    case 'fixed-folder': {
      const folder = settings.outputFolder.replace(/\/+$/, '');
      return `${folder}/${filename}`;
    }

    case 'ask':
      return null;
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
npm test -- tests/output.test.ts
```

Expected: PASS — 6 tests.

- [ ] **Step 5: Commit**

```bash
git add src/output.ts tests/output.test.ts
git commit -m "feat: add output path resolution for all three output modes"
```

---

## Task 7: WASM wrappers

**Files:**
- Create: `src/wasm/omd2typst.ts`
- Create: `src/wasm/typst.ts`

These files wrap the WASM modules with lazy initialisation. They are not unit-tested directly — they are covered by the exporter integration test in Task 8.

- [ ] **Step 1: Build the omd2typst WASM**

From the plugin repo root:

```bash
git submodule update --init
npm run build:wasm
```

Expected: `✓ omd2typst WASM written to src/wasm/omd2typst-pkg`
The directory `src/wasm/omd2typst-pkg/` now contains `omd2typst_bg.wasm`, `omd2typst.js`, and `omd2typst.d.ts`.

- [ ] **Step 2: Create src/wasm/omd2typst.ts**

```typescript
import init, { render_to_typst, get_builtin_template } from './omd2typst-pkg/omd2typst';

let initialised = false;

async function ensureInit(): Promise<void> {
  if (initialised) return;
  await init();
  initialised = true;
}

/**
 * Convert Markdown to a Typst source string.
 * Pass the full content of a .typ template as templateSrc, or null for the built-in.
 */
export async function renderToTypst(
  markdown: string,
  templateSrc: string | null,
): Promise<string> {
  await ensureInit();
  return render_to_typst(markdown, templateSrc ?? undefined);
}

/** Return the built-in Typst template source. */
export async function getBuiltinTemplate(): Promise<string> {
  await ensureInit();
  return get_builtin_template();
}
```

- [ ] **Step 3: Inspect the typst-ts package to confirm the compiler API**

```bash
cat node_modules/@myriaddreamin/typst-ts-web-compiler/pkg/typst_ts_web_compiler.d.ts | head -80
```

Read the output to confirm the initialisation and compilation method signatures. The next step uses the documented API; adjust method names if the installed version differs.

- [ ] **Step 4: Create src/wasm/typst.ts**

```typescript
import { createTypstCompiler } from '@myriaddreamin/typst-ts-web-compiler';

let compiler: Awaited<ReturnType<typeof createTypstCompiler>> | null = null;
let wasmUrl: string | null = null;

/**
 * Must be called once on plugin load before any PDF compilation.
 * pluginDir — the vault-relative path to the plugin folder
 *   e.g. (plugin as any).manifest.dir  →  '.obsidian/plugins/omd2typst'
 * getResourcePath — app.vault.adapter.getResourcePath
 */
export function setTypstWasmPath(resourcePath: string): void {
  wasmUrl = resourcePath;
}

async function ensureCompiler(): Promise<typeof compiler> {
  if (compiler) return compiler;
  if (!wasmUrl) throw new Error('Typst WASM path not configured. Call setTypstWasmPath() first.');
  compiler = createTypstCompiler();
  await compiler.init({ getModule: () => fetch(wasmUrl!) });
  return compiler;
}

/** Compile a Typst source string to PDF bytes. */
export async function compileToPdf(typstSrc: string): Promise<Uint8Array> {
  const c = await ensureCompiler();
  c!.addSource('/main.typ', typstSrc);
  const pdf = await c!.pdf({ mainFilePath: '/main.typ' });
  if (!pdf) throw new Error('Typst compilation returned no output.');
  return pdf;
}
```

- [ ] **Step 5: Commit**

```bash
git add src/wasm/
git commit -m "feat: add lazy-init WASM wrappers for omd2typst and typst compiler"
```

---

## Task 8: Exporter pipeline

**Files:**
- Create: `src/exporter.ts`
- Create: `tests/exporter.test.ts`

- [ ] **Step 1: Write the failing integration test**

Create `tests/exporter.test.ts`:

```typescript
import { exportNote } from '../src/exporter';
import { TFile, Notice } from 'obsidian';
import type { Omd2TypstSettings } from '../src/settings';

// Mock the WASM wrappers
jest.mock('../src/wasm/omd2typst', () => ({
  renderToTypst: jest.fn().mockResolvedValue('#heading[Hello]'),
}));
jest.mock('../src/wasm/typst', () => ({
  compileToPdf: jest.fn().mockResolvedValue(new Uint8Array([0x25, 0x50, 0x44, 0x46])),
}));

const SETTINGS: Omd2TypstSettings = {
  templates: [], defaultTemplate: 'built-in',
  defaultOutputFormat: 'pdf', outputMode: 'same-folder', outputFolder: 'exports',
  defaultLanguage: 'en', frontmatterTemplateMode: 'inline',
  frontmatterInline: '', frontmatterFilePath: '',
};

function makeApp(noteContent: string) {
  const writtenFiles: Record<string, string | Uint8Array> = {};
  return {
    vault: {
      read: jest.fn().mockResolvedValue(noteContent),
      create: jest.fn().mockImplementation(async (path: string, data: any) => {
        writtenFiles[path] = data;
      }),
      createBinary: jest.fn().mockImplementation(async (path: string, data: Uint8Array) => {
        writtenFiles[path] = data;
      }),
      getAbstractFileByPath: jest.fn().mockReturnValue(null),
      createFolder: jest.fn().mockResolvedValue(undefined),
      adapter: {
        getResourcePath: jest.fn().mockReturnValue('http://localhost/typst.wasm'),
      },
    },
    _written: writtenFiles,
  };
}

describe('exportNote', () => {
  it('writes a .typ file when format is typ', async () => {
    const app = makeApp('# Hello') as any;
    const file = new TFile('notes/hello.md');
    await exportNote(file, 'typ', null, SETTINGS, app);
    expect(app._written['notes/hello.typ']).toBe('#heading[Hello]');
  });

  it('writes a .pdf file when format is pdf', async () => {
    const app = makeApp('# Hello') as any;
    const file = new TFile('notes/hello.md');
    await exportNote(file, 'pdf', null, SETTINGS, app);
    const pdf = app._written['notes/hello.pdf'] as Uint8Array;
    expect(pdf[0]).toBe(0x25); // %PDF magic bytes
  });

  it('warns when template language does not match note language', async () => {
    const template = { name: 'DUO', path: 'tmpl.typ', languages: ['nl'] };
    const settings = { ...SETTINGS, templates: [template], defaultTemplate: 'DUO' };
    const app = makeApp('---\nlanguage: fr\n---\n# Hello') as any;
    const mockTemplateFile = { path: 'tmpl.typ' };
    app.vault.getAbstractFileByPath = jest.fn().mockImplementation((p: string) => {
      return p === 'tmpl.typ' ? mockTemplateFile : null;
    });
    app.vault.read = jest.fn().mockImplementation(async (f: any) => {
      if (f?.path === 'tmpl.typ') return '// omd2typst-languages: nl';
      return '---\nlanguage: fr\n---\n# Hello';
    });
    const noticeMock = jest.mocked(Notice);
    noticeMock.mockClear();
    const file = new TFile('notes/hello.md');
    await exportNote(file, 'typ', template, settings, app);
    expect(noticeMock).toHaveBeenCalledWith(expect.stringContaining('DUO'));
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
npm test -- tests/exporter.test.ts
```

Expected: FAIL — `Cannot find module '../src/exporter'`

- [ ] **Step 3: Create src/exporter.ts**

```typescript
import { TFile, Notice } from 'obsidian';
import type { App } from 'obsidian';
import { renderToTypst } from './wasm/omd2typst';
import { compileToPdf } from './wasm/typst';
import { resolveOutputPath } from './output';
import { parseFrontmatter } from './frontmatter';
import { checkLanguageCompatibility } from './template';
import type { OutputFormat, Omd2TypstSettings, TemplateEntry } from './settings';

/**
 * Run the full export pipeline for a single note file.
 * template — the TemplateEntry to use, or null for the built-in template.
 */
export async function exportNote(
  file: TFile,
  format: OutputFormat,
  template: TemplateEntry | null,
  settings: Omd2TypstSettings,
  app: App,
): Promise<void> {
  const noteContent = await app.vault.read(file);

  // Resolve language for validation
  const fm = parseFrontmatter(noteContent);
  const noteLanguage = extractLanguage(noteContent, fm, settings.defaultLanguage);

  // Language compatibility warning (non-blocking)
  if (template) {
    const templateContent = await readTemplateFile(template.path, app);
    if (templateContent !== null) {
      const warning = checkLanguageCompatibility(template, noteLanguage);
      if (warning) new Notice(warning);
    }
  }

  // Load template source
  const templateSrc = template ? await readTemplateFile(template.path, app) : null;

  // MD → Typst
  const typstSrc = await renderToTypst(noteContent, templateSrc);

  // Resolve output path
  const outPath = resolveOutputPath(file.path, format, settings);
  if (outPath === null) return; // 'ask' mode not yet triggered here — handled in main.ts

  await ensureParentFolder(outPath, app);

  if (format === 'typ') {
    await writeText(outPath, typstSrc, app);
  } else {
    let pdfBytes: Uint8Array;
    try {
      pdfBytes = await compileToPdf(typstSrc);
    } catch (e) {
      // Preserve the intermediate .typ so the user can inspect what Typst rejected.
      const typFallback = outPath.replace(/\.pdf$/, '.typ');
      await writeText(typFallback, typstSrc, app);
      throw new Error(`Typst compilation failed (source saved to ${typFallback}): ${(e as Error).message}`);
    }
    await writeBinary(outPath, pdfBytes, app);
  }
}

function extractLanguage(
  content: string,
  fm: { keys: string[] } | null,
  defaultLang: string,
): string {
  if (!fm) return defaultLang;
  const line = content.split('\n').find(l => l.trimStart().startsWith('language:'));
  if (!line) return defaultLang;
  return line.split(':')[1]?.trim() || defaultLang;
}

async function readTemplateFile(path: string, app: App): Promise<string | null> {
  try {
    const f = app.vault.getAbstractFileByPath(path);
    if (!f) return null;
    // Duck-typed read: works with both the real Obsidian TFile and test mocks.
    return await app.vault.read(f as TFile);
  } catch {
    return null;
  }
}

async function ensureParentFolder(filePath: string, app: App): Promise<void> {
  const parts = filePath.split('/');
  if (parts.length <= 1) return;
  const folder = parts.slice(0, -1).join('/');
  if (!app.vault.getAbstractFileByPath(folder)) {
    await app.vault.createFolder(folder);
  }
}

async function writeText(path: string, content: string, app: App): Promise<void> {
  const existing = app.vault.getAbstractFileByPath(path);
  if (existing && existing instanceof TFile) {
    await app.vault.modify(existing, content);
  } else {
    await app.vault.create(path, content);
  }
}

async function writeBinary(path: string, data: Uint8Array, app: App): Promise<void> {
  const existing = app.vault.getAbstractFileByPath(path);
  if (existing && existing instanceof TFile) {
    await app.vault.modifyBinary(existing, data.buffer);
  } else {
    await app.vault.createBinary(path, data);
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
npm test -- tests/exporter.test.ts
```

Expected: PASS — 3 tests.

- [ ] **Step 5: Run the full test suite**

```bash
npm test
```

Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/exporter.ts tests/exporter.test.ts
git commit -m "feat: add exporter pipeline (MD → Typst → PDF)"
```

---

## Task 9: Plugin main — commands and context menus

**Files:**
- Create: `src/main.ts`

- [ ] **Step 1: Create src/main.ts**

```typescript
import { Plugin, TFile, Notice, normalizePath } from 'obsidian';
import { Omd2TypstSettings, DEFAULT_SETTINGS, TemplateEntry } from './settings';
import { Omd2TypstSettingTab } from './settings';
import { exportNote } from './exporter';
import { resolveDefaultTemplate } from './template';
import { mergeFrontmatter, buildFrontmatterBlock } from './frontmatter';
import { getBuiltinTemplate } from './wasm/omd2typst';
import { setTypstWasmPath } from './wasm/typst';

export default class Omd2TypstPlugin extends Plugin {
  settings!: Omd2TypstSettings;

  async onload(): Promise<void> {
    await this.loadSettings();
    this.addSettingTab(new Omd2TypstSettingTab(this.app, this));

    // Configure the Typst WASM path for runtime loading
    const pluginDir = (this as any).manifest.dir as string;
    const wasmPath = this.app.vault.adapter.getResourcePath(
      normalizePath(`${pluginDir}/wasm-runtime/typst_compiler.wasm`)
    );
    setTypstWasmPath(wasmPath);

    this.registerCommands();
    this.registerContextMenus();
  }

  private registerCommands(): void {
    this.addCommand({
      id: 'export-pdf',
      name: 'Export as PDF',
      checkCallback: (checking) => {
        const file = this.activeMarkdownFile();
        if (!file) return false;
        if (!checking) this.runExport(file, 'pdf');
        return true;
      },
    });

    this.addCommand({
      id: 'export-typ',
      name: 'Export as Typst source',
      checkCallback: (checking) => {
        const file = this.activeMarkdownFile();
        if (!file) return false;
        if (!checking) this.runExport(file, 'typ');
        return true;
      },
    });

    this.addCommand({
      id: 'insert-frontmatter',
      name: 'Insert omd2typst frontmatter',
      checkCallback: (checking) => {
        const file = this.activeMarkdownFile();
        if (!file) return false;
        if (!checking) this.runInsertFrontmatter(file);
        return true;
      },
    });

    this.addCommand({
      id: 'export-builtin-template',
      name: 'Export built-in template',
      callback: () => this.runExportBuiltinTemplate(),
    });
  }

  private registerContextMenus(): void {
    this.registerEvent(
      this.app.workspace.on('file-menu', (menu, abstractFile) => {
        if (!(abstractFile instanceof TFile) || abstractFile.extension !== 'md') return;
        menu.addItem(item =>
          item.setTitle('Export as PDF (omd2typst)')
            .setIcon('file-pdf')
            .onClick(() => this.runExport(abstractFile, 'pdf'))
        );
        menu.addItem(item =>
          item.setTitle('Export as Typst source (omd2typst)')
            .setIcon('file-type')
            .onClick(() => this.runExport(abstractFile, 'typ'))
        );
      })
    );
  }

  private async runExport(file: TFile, format: 'typ' | 'pdf'): Promise<void> {
    const template = resolveDefaultTemplate(this.settings);
    try {
      await exportNote(file, format, template, this.settings, this.app);
      new Notice(`Exported ${file.basename}.${format}`);
    } catch (e) {
      new Notice(`Export failed: ${(e as Error).message}`);
      console.error('[omd2typst] export error', e);
    }
  }

  private async runInsertFrontmatter(file: TFile): Promise<void> {
    const keys = buildFrontmatterBlock(this.settings.frontmatterInline);
    const content = await this.app.vault.read(file);
    const merged = mergeFrontmatter(content, keys);
    await this.app.vault.modify(file, merged);
  }

  private async runExportBuiltinTemplate(): Promise<void> {
    try {
      const src = await getBuiltinTemplate();
      const path = 'omd2typst-template.typ';
      const existing = this.app.vault.getAbstractFileByPath(path);
      if (existing instanceof TFile) {
        await this.app.vault.modify(existing, src);
      } else {
        await this.app.vault.create(path, src);
      }
      new Notice(`Built-in template exported to ${path}`);
    } catch (e) {
      new Notice(`Failed to export template: ${(e as Error).message}`);
    }
  }

  private activeMarkdownFile(): TFile | null {
    const file = this.app.workspace.getActiveFile();
    if (file instanceof TFile && file.extension === 'md') return file;
    return null;
  }

  async loadSettings(): Promise<void> {
    this.settings = Object.assign({}, DEFAULT_SETTINGS, await this.loadData());
  }

  async saveSettings(): Promise<void> {
    await this.saveData(this.settings);
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/main.ts
git commit -m "feat: register all commands and context menus in plugin main"
```

> **Note:** `main.ts` imports `Omd2TypstSettingTab` from `./settings`, which is not defined until Task 10. TypeScript will compile both files together at build time so this is fine, but `npm run build` will fail until Task 10 is complete. Task 11 is the first full-build verification step.

---

## Task 10: Settings tab UI

**Files:**
- Modify: `src/settings.ts` (append the SettingTab class)

- [ ] **Step 1: Append imports and Omd2TypstSettingTab to src/settings.ts**

Add the following imports at the top of `src/settings.ts` (after the existing type declarations):

```typescript
import { App, PluginSettingTab, Setting, TFile } from 'obsidian';
import type Omd2TypstPlugin from './main';
import { parseTemplateLanguages } from './template';
```

Then append the SettingTab class at the end of `src/settings.ts`:

```typescript
import { App, PluginSettingTab, Setting, TFile } from 'obsidian';
import type Omd2TypstPlugin from './main';
import { parseTemplateLanguages } from './template';

export class Omd2TypstSettingTab extends PluginSettingTab {
  constructor(app: App, private plugin: Omd2TypstPlugin) {
    super(app, plugin);
  }

  display(): void {
    const { containerEl } = this;
    containerEl.empty();

    containerEl.createEl('h2', { text: 'omd2typst Settings' });

    // --- Default output format ---
    new Setting(containerEl)
      .setName('Default output format')
      .setDesc('Format used when exporting via the command palette default action.')
      .addDropdown(drop => drop
        .addOption('pdf', 'PDF')
        .addOption('typ', 'Typst source (.typ)')
        .setValue(this.plugin.settings.defaultOutputFormat)
        .onChange(async v => {
          this.plugin.settings.defaultOutputFormat = v as OutputFormat;
          await this.plugin.saveSettings();
        })
      );

    // --- Output location ---
    new Setting(containerEl)
      .setName('Output location')
      .addDropdown(drop => drop
        .addOption('same-folder', 'Same folder as note')
        .addOption('fixed-folder', 'Fixed folder')
        .addOption('ask', 'Ask every time')
        .setValue(this.plugin.settings.outputMode)
        .onChange(async v => {
          this.plugin.settings.outputMode = v as OutputMode;
          await this.plugin.saveSettings();
          this.display(); // re-render to show/hide folder field
        })
      );

    if (this.plugin.settings.outputMode === 'fixed-folder') {
      new Setting(containerEl)
        .setName('Output folder')
        .setDesc('Vault-relative path, e.g. exports')
        .addText(text => text
          .setValue(this.plugin.settings.outputFolder)
          .onChange(async v => {
            this.plugin.settings.outputFolder = v.trim();
            await this.plugin.saveSettings();
          })
        );
    }

    // --- Default language ---
    new Setting(containerEl)
      .setName('Default language')
      .setDesc('Applied when the note has no language: frontmatter key. The built-in template defaults to English.')
      .addDropdown(drop => drop
        .addOption('en', 'English (en)')
        .addOption('nl', 'Nederlands (nl)')
        .setValue(this.plugin.settings.defaultLanguage)
        .onChange(async v => {
          this.plugin.settings.defaultLanguage = v;
          await this.plugin.saveSettings();
        })
      );

    // --- Templates ---
    containerEl.createEl('h3', { text: 'Templates' });
    new Setting(containerEl)
      .setName('Default template')
      .setDesc('Used for right-click exports and as the pre-selected option in palette exports.')
      .addDropdown(drop => {
        drop.addOption('built-in', '(Built-in)');
        this.plugin.settings.templates.forEach(t => {
          const langLabel = t.languages.length > 0 ? ` [${t.languages.join(', ')}]` : '';
          drop.addOption(t.name, `${t.name}${langLabel}`);
        });
        drop.setValue(this.plugin.settings.defaultTemplate)
          .onChange(async v => {
            this.plugin.settings.defaultTemplate = v;
            await this.plugin.saveSettings();
          });
        return drop;
      });

    this.plugin.settings.templates.forEach((tmpl, idx) => {
      const row = new Setting(containerEl)
        .setName(tmpl.name)
        .setDesc(tmpl.path + (tmpl.languages.length ? ` · languages: ${tmpl.languages.join(', ')}` : ''));
      row.addButton(btn => btn.setButtonText('Remove').setWarning().onClick(async () => {
        this.plugin.settings.templates.splice(idx, 1);
        await this.plugin.saveSettings();
        this.display();
      }));
    });

    new Setting(containerEl)
      .setName('Add template')
      .addText(name => name.setPlaceholder('Name (e.g. DUO)').onChange(() => {}))
      .addText(path => path.setPlaceholder('Path (e.g. templates/duo.typ)').onChange(() => {}))
      .addButton(btn => btn.setButtonText('Add').onClick(async () => {
        const inputs = containerEl.querySelectorAll<HTMLInputElement>('input[type=text]');
        const name = inputs[inputs.length - 2]?.value.trim();
        const path = inputs[inputs.length - 1]?.value.trim();
        if (!name || !path) return;
        const file = this.app.vault.getAbstractFileByPath(path);
        const languages = file instanceof TFile
          ? parseTemplateLanguages(await this.app.vault.read(file))
          : [];
        this.plugin.settings.templates.push({ name, path, languages });
        await this.plugin.saveSettings();
        this.display();
      }));

    // --- Frontmatter template ---
    containerEl.createEl('h3', { text: 'Frontmatter template' });
    new Setting(containerEl)
      .setName('Template source')
      .addDropdown(drop => drop
        .addOption('inline', 'Inline editor')
        .addOption('file', 'Template file')
        .setValue(this.plugin.settings.frontmatterTemplateMode)
        .onChange(async v => {
          this.plugin.settings.frontmatterTemplateMode = v as FrontmatterTemplateMode;
          await this.plugin.saveSettings();
          this.display();
        })
      );

    if (this.plugin.settings.frontmatterTemplateMode === 'inline') {
      new Setting(containerEl)
        .setName('YAML keys (one per line, colon optional)')
        .addTextArea(ta => ta
          .setValue(this.plugin.settings.frontmatterInline)
          .onChange(async v => {
            this.plugin.settings.frontmatterInline = v;
            await this.plugin.saveSettings();
          })
        );
    } else {
      new Setting(containerEl)
        .setName('Template file path')
        .setDesc('Vault-relative path to a .md file whose frontmatter is used as the template.')
        .addText(t => t
          .setValue(this.plugin.settings.frontmatterFilePath)
          .onChange(async v => {
            this.plugin.settings.frontmatterFilePath = v.trim();
            await this.plugin.saveSettings();
          })
        );
    }
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/settings.ts
git commit -m "feat: add settings tab UI (templates, language, output, frontmatter)"
```

---

## Task 11: End-to-end build verification

- [ ] **Step 1: Run the full test suite**

```bash
npm test
```

Expected: All tests pass.

- [ ] **Step 2: Build the plugin**

```bash
npm run build
```

Expected: `main.js` created in the repo root, no errors. The `wasm-runtime/` directory contains `typst_compiler.wasm`.

- [ ] **Step 3: Install into a test vault manually**

Copy the plugin to your Obsidian vault's plugins folder:

```bash
VAULT=~/path/to/your/vault
mkdir -p "$VAULT/.obsidian/plugins/omd2typst"
cp main.js manifest.json "$VAULT/.obsidian/plugins/omd2typst/"
cp -r wasm-runtime "$VAULT/.obsidian/plugins/omd2typst/"
```

- [ ] **Step 4: Enable the plugin in Obsidian**

Open Obsidian → Settings → Community Plugins → enable "omd2typst". Confirm no error appears in the developer console (View → Toggle Developer Tools → Console tab).

- [ ] **Step 5: Smoke test — Insert frontmatter**

Open a blank note. Run Command Palette → "Insert omd2typst frontmatter". Verify all expected keys appear at the top of the note.

- [ ] **Step 6: Smoke test — Export as Typst source**

Open a note with some content. Run Command Palette → "Export as Typst source". Verify a `.typ` file appears in the expected location and contains valid Typst markup.

- [ ] **Step 7: Smoke test — Export as PDF**

With the same note, run Command Palette → "Export as PDF". Verify a `.pdf` file is created and opens correctly.

- [ ] **Step 8: Smoke test — Export built-in template**

Run Command Palette → "Export built-in template". Verify `omd2typst-template.typ` appears in the vault root and is valid Typst.

- [ ] **Step 9: Smoke test — Right-click export**

Right-click a `.md` file in the file explorer. Verify "Export as PDF (omd2typst)" and "Export as Typst source (omd2typst)" appear and work.

- [ ] **Step 10: Smoke test — Language warning**

Add `language: fr` to a note's frontmatter. Add a template with `// omd2typst-languages: nl, en`. Set it as the default. Export the note. Verify a warning notification appears mentioning the language mismatch.

- [ ] **Step 11: Final commit**

```bash
git add .
git commit -m "chore: verify end-to-end build and smoke tests pass"
```

---

## Summary of commits

| Task | Commit message |
|---|---|
| 1 | `feat: add WASM API (render_to_typst, get_builtin_template) via wasm-bindgen` |
| 2 | `chore: scaffold obsidian-omd2typst plugin repo` |
| 3 | `feat: add settings model and Jest mocks` |
| 4 | `feat: add template language parsing and resolution` |
| 5 | `feat: add frontmatter parse, merge, and key-extraction logic` |
| 6 | `feat: add output path resolution for all three output modes` |
| 7 | `feat: add lazy-init WASM wrappers for omd2typst and typst compiler` |
| 8 | `feat: add exporter pipeline (MD → Typst → PDF)` |
| 9 | `feat: register all commands and context menus in plugin main` |
| 10 | `feat: add settings tab UI (templates, language, output, frontmatter)` |
| 11 | `chore: verify end-to-end build and smoke tests pass` |
