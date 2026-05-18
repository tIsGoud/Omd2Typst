# Obsidian Plugin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `obsidian-omd2typst`, an official Obsidian community plugin that exports notes to Typst source or PDF using one WASM module (omd2typst for MD→Typst) and the user's installed `typst` binary for PDF compilation.

**Architecture:** The existing `omd2typst` Rust repo gains a `src/lib.rs` WASM entry point compiled via `wasm-pack`; a new `obsidian-omd2typst` TypeScript repo consumes it as a git submodule. The omd2typst WASM is loaded at runtime via fetch; PDF compilation shells out to the system `typst` CLI via Node.js `child_process` (available in Electron/Obsidian). No typst compiler is bundled in the plugin.

**Tech Stack:** Rust + wasm-bindgen + wasm-pack; TypeScript + Obsidian API; esbuild; Node.js `child_process` + `fs`; Jest + ts-jest.

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
| `esbuild.config.mjs` | Create | Bundle src/ → main.js; copy omd2typst WASM to wasm-runtime/ |
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
| `src/typst-cli.ts` | Create | findTypstBinary, checkTypstInstalled, compileToPdfViaCli |
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
    "jest": "^29.5.0",
    "obsidian": "latest",
    "ts-jest": "^29.5.0",
    "typescript": "^5.4.0"
  },
  "dependencies": {}
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
import builtins from 'builtin-modules';
import { copyFile, mkdir } from 'fs/promises';
import process from 'process';

const prod = process.argv[2] === 'production';

// Copy the omd2typst WASM to wasm-runtime/ so it can be loaded at runtime
// via fetch() (async instantiation avoids Chrome's 4 KB sync-compile limit).
await mkdir('wasm-runtime', { recursive: true });
await copyFile(
  'src/wasm/omd2typst-pkg/omd2typst_wasm_bg.wasm',
  'wasm-runtime/omd2typst_bg.wasm',
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
  target: 'es2022',
  sourcemap: prod ? false : 'inline',
  treeShaking: true,
  outfile: 'main.js',
  minify: prod,
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

## Task 7: WASM wrapper and typst CLI helper

**Files:**
- Create: `src/wasm/omd2typst.ts`
- Create: `src/typst-cli.ts`

The WASM wrapper handles MD→Typst with lazy init. The CLI helper shells out to the system `typst` binary for PDF compilation. Neither is unit-tested directly — both are covered by the exporter integration test in Task 8.

- [ ] **Step 1: Build the omd2typst WASM**

From the plugin repo root:

```bash
git submodule update --init
npm run build:wasm
```

Expected: `✓ omd2typst WASM written to src/wasm/omd2typst-pkg`
The directory `src/wasm/omd2typst-pkg/` now contains `omd2typst_wasm_bg.wasm`, the JS glue, and `.d.ts` bindings.

- [ ] **Step 2: Create src/wasm/omd2typst.ts**

```typescript
import {
  __wbg_set_wasm,
  __wbindgen_init_externref_table,
  render_to_typst,
  get_builtin_template,
} from './omd2typst-pkg/omd2typst_wasm_bg.js';

let initialised = false;
let wasmPath: string | null = null;

export function setOmd2TypstWasmPath(resourcePath: string): void {
  wasmPath = resourcePath;
}

async function ensureInit(): Promise<void> {
  if (initialised) return;
  if (!wasmPath) throw new Error('omd2typst WASM path not configured.');
  const response = await fetch(wasmPath);
  const buffer = await response.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(buffer, {
    './omd2typst_wasm_bg.js': { __wbindgen_init_externref_table },
  });
  __wbg_set_wasm(instance.exports);
  (instance.exports.__wbindgen_start as CallableFunction)();
  initialised = true;
}

/**
 * Convert Markdown to a Typst source string.
 * templateSrc — vault-relative import path (e.g. "/typst/template.typ"), or null for built-in.
 * The renderer emits `#import "<path>": template, callout` in the output.
 */
export async function renderToTypst(
  markdown: string,
  templateSrc: string | null,
): Promise<string> {
  await ensureInit();
  return render_to_typst(markdown, templateSrc ?? undefined);
}

export async function getBuiltinTemplate(): Promise<string> {
  await ensureInit();
  return get_builtin_template();
}
```

- [ ] **Step 3: Create src/typst-cli.ts**

```typescript
// eslint-disable-next-line @typescript-eslint/no-require-imports
const nodeFs   = typeof require !== 'undefined' ? require('fs')   as typeof import('fs')   : null;
// eslint-disable-next-line @typescript-eslint/no-require-imports
const nodePath = typeof require !== 'undefined' ? require('path') as typeof import('path') : null;

export function findTypstBinary(): string | null {
  if (!nodeFs) return null;
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const cp = require('child_process') as typeof import('child_process');
  try { cp.execSync('typst --version', { stdio: 'pipe' }); return 'typst'; } catch { /* fall through */ }
  const candidates = [
    '/opt/homebrew/bin/typst',
    '/usr/local/bin/typst',
    '/usr/bin/typst',
    process.env.HOME ? `${process.env.HOME}/.cargo/bin/typst` : '',
  ];
  for (const p of candidates) {
    if (p && nodeFs.existsSync(p)) return p;
  }
  return null;
}

export function checkTypstInstalled(): string {
  if (!nodeFs) throw new Error('Not running in Electron/Node environment');
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const cp = require('child_process') as typeof import('child_process');
  const bin = findTypstBinary();
  if (!bin) throw new Error('typst not found. Install from https://typst.app or add to PATH.');
  return cp.execSync(`"${bin}" --version`, { stdio: 'pipe' }).toString().trim();
}

/**
 * Compile a .typ file on disk to PDF using the system typst CLI.
 * typPath   — vault-relative path to the .typ file
 * vaultBase — absolute vault root (app.vault.adapter as any).basePath
 * The --root flag makes vault-relative #import paths in the .typ resolve correctly.
 */
export async function compileToPdfViaCli(typPath: string, vaultBase: string): Promise<Uint8Array> {
  if (!nodeFs || !nodePath) throw new Error('CLI requires Electron/Node environment');
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const cp = require('child_process') as typeof import('child_process');
  const bin = findTypstBinary();
  if (!bin) throw new Error('typst CLI not found. Install from https://typst.app or add to PATH.');
  const realTypPath = nodePath.join(vaultBase, typPath);
  const realPdfPath = realTypPath.replace(/\.typ$/, '.__tmp.pdf');
  const cmd = [`"${bin}"`, 'compile', `"${realTypPath}"`, `"${realPdfPath}"`, `--root "${vaultBase}"`].join(' ');
  console.log('[omd2typst] CLI compile:', cmd);
  try {
    cp.execSync(cmd, { timeout: 120_000, stdio: 'pipe' });
    const buf = nodeFs.readFileSync(realPdfPath) as Buffer;
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
  } finally {
    try { nodeFs.unlinkSync(realPdfPath); } catch { /* best-effort */ }
  }
}
```

- [ ] **Step 4: Commit**

```bash
git add src/wasm/omd2typst.ts src/typst-cli.ts
git commit -m "feat: add omd2typst WASM wrapper and typst CLI helper"
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
import type { TFile } from 'obsidian';
const { TFile: TFileCtor } = require('obsidian') as { TFile: new (path: string) => TFile };
import type { Omd2TypstSettings, TemplateEntry } from '../src/settings';

jest.mock('../src/wasm/omd2typst', () => ({
  renderToTypst: jest.fn().mockResolvedValue('#heading[Hello]'),
}));
jest.mock('../src/typst-cli', () => ({
  compileToPdfViaCli: jest.fn().mockResolvedValue(new Uint8Array([0x25, 0x50, 0x44, 0x46])),
}));

const BASE_SETTINGS: Omd2TypstSettings = {
  templates: [], defaultTemplate: 'built-in',
  defaultOutputFormat: 'typ', outputMode: 'fixed-folder', outputFolder: 'exports',
  defaultLanguage: 'en', frontmatterTemplateMode: 'inline',
  frontmatterInline: '', frontmatterFilePath: '',
};

function makeTFile(path: string): TFile {
  const parts = path.split('/');
  const name = parts[parts.length - 1];
  const extension = name.includes('.') ? name.split('.').pop()! : '';
  const basename = name.replace(/\.[^.]+$/, '');
  const parent = parts.length > 1 ? { path: parts.slice(0, -1).join('/') } : null;
  return { path, name, extension, basename, parent } as unknown as TFile;
}

function makeApp() {
  return {
    vault: {
      read: jest.fn().mockResolvedValue('# Hello\n'),
      getAbstractFileByPath: jest.fn().mockReturnValue(null),
      adapter: {
        write: jest.fn().mockResolvedValue(undefined),
        remove: jest.fn().mockResolvedValue(undefined),
      },
    },
  };
}

describe('exportNote — Typst export', () => {
  it('writes a .typ file when format is typ', async () => {
    const file = makeTFile('notes/hello.md');
    const app = makeApp();
    await exportNote(file, 'typ', null, BASE_SETTINGS, app as any);
    expect(app.vault.adapter.write).toHaveBeenCalledWith('exports/hello.typ', '#heading[Hello]');
  });
});

describe('exportNote — PDF export', () => {
  it('writes .typ then .pdf and removes the intermediate .typ', async () => {
    const file = makeTFile('notes/hello.md');
    const app = makeApp();
    await exportNote(file, 'pdf', null, BASE_SETTINGS, app as any);
    const calls = (app.vault.adapter.write as jest.Mock).mock.calls;
    expect(calls[0][0]).toBe('exports/hello.typ');  // intermediate
    expect(calls[1][0]).toBe('exports/hello.pdf');  // final PDF
    expect(app.vault.adapter.remove).toHaveBeenCalledWith('exports/hello.typ');
  });
});

describe('exportNote — language mismatch', () => {
  it('emits a Notice on mismatch but still exports', async () => {
    const template: TemplateEntry = { name: 'DUO', path: 'templates/duo.typ', languages: ['nl'] };
    const templateFile = new TFileCtor('templates/duo.typ');
    const app = makeApp();
    (app.vault.read as jest.Mock).mockResolvedValueOnce('---\nlanguage: en\n---\n# Hello\n');
    (app.vault.getAbstractFileByPath as jest.Mock).mockReturnValue(templateFile);
    const noticeMock = jest.spyOn(require('obsidian'), 'Notice');
    await exportNote(file, 'typ', template, BASE_SETTINGS, app as any);
    expect(noticeMock).toHaveBeenCalled();
    expect(app.vault.adapter.write).toHaveBeenCalled();
    noticeMock.mockRestore();
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
import { Notice, TFile, App } from 'obsidian';
import type { OutputFormat, TemplateEntry, Omd2TypstSettings } from './settings';
import { checkLanguageCompatibility } from './template';
import { resolveOutputPath } from './output';
import { renderToTypst } from './wasm/omd2typst';
import { compileToPdfViaCli } from './typst-cli';

function extractFrontmatterValue(content: string, key: string): string | null {
  if (!content.startsWith('---')) return null;
  const afterOpen = content.slice(3);
  const closeIdx = afterOpen.indexOf('\n---');
  if (closeIdx === -1) return null;
  for (const line of afterOpen.slice(0, closeIdx).split('\n')) {
    const colonIdx = line.indexOf(':');
    if (colonIdx === -1) continue;
    if (line.slice(0, colonIdx).trim() === key) return line.slice(colonIdx + 1).trim() || null;
  }
  return null;
}

export async function exportNote(
  file: TFile,
  format: OutputFormat,
  template: TemplateEntry | null,
  settings: Omd2TypstSettings,
  app: App,
): Promise<void> {
  const markdown = await app.vault.read(file);

  // Resolve template path for #import (null → built-in).
  // Prefix with / → vault-root-relative; typst CLI resolves via --root <vaultBase>.
  let templatePath: string | null = null;
  if (template !== null && template.path) {
    const abstractFile = app.vault.getAbstractFileByPath(template.path);
    if (!(abstractFile instanceof TFile)) {
      throw new Error(`Template file not found or is a folder: '${template.path}'`);
    }
    templatePath = '/' + template.path;
  }

  // Language compatibility warning (non-blocking)
  if (template !== null) {
    const noteLanguage = extractFrontmatterValue(markdown, 'language') ?? settings.defaultLanguage;
    const warning = checkLanguageCompatibility(template, noteLanguage);
    if (warning !== null) new Notice(warning);
  }

  const typstSrc = await renderToTypst(markdown, templatePath);

  const outputPath = resolveOutputPath(file.path, format, settings);
  if (outputPath === null) throw new Error('ask-every-time output mode not yet implemented');

  if (format === 'typ') {
    await app.vault.adapter.write(outputPath, typstSrc);
  } else {
    // Write intermediate .typ so typst CLI can compile it with --root <vaultBase>.
    const typPath = outputPath.replace(/\.pdf$/, '.typ');
    await app.vault.adapter.write(typPath, typstSrc);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const vaultBase: string = (app.vault.adapter as any).basePath ?? '';
    const pdfBytes = await compileToPdfViaCli(typPath, vaultBase);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    await (app.vault.adapter.write as any)(outputPath, pdfBytes);
    await app.vault.adapter.remove(typPath);
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
git commit -m "feat: add exporter pipeline (MD → Typst → PDF via system typst CLI)"
```

---

## Task 9: Plugin main — commands and context menus

**Files:**
- Create: `src/main.ts`

- [ ] **Step 1: Create src/main.ts**

```typescript
import { Plugin, TFile, Notice, normalizePath } from 'obsidian';
import { Omd2TypstSettings, DEFAULT_SETTINGS, OutputFormat, Omd2TypstSettingTab } from './settings';
import { resolveDefaultTemplate } from './template';
import { mergeFrontmatter } from './frontmatter';
import { exportNote } from './exporter';
import { checkTypstInstalled } from './typst-cli';
import { setOmd2TypstWasmPath, getBuiltinTemplate } from './wasm/omd2typst';

export default class Omd2TypstPlugin extends Plugin {
  settings: Omd2TypstSettings = DEFAULT_SETTINGS;

  async onload() {
    await this.loadSettings();

    // Verify typst CLI is installed — required for PDF export.
    try {
      const version = checkTypstInstalled();
      console.log(`[omd2typst] Found ${version}`);
    } catch {
      new Notice('omd2typst: typst not found — PDF export will fail. Install typst from https://typst.app or add it to PATH.');
    }

    // Configure the omd2typst WASM path for runtime loading.
    const omd2typstWasmPath = (this.app.vault.adapter as any).getResourcePath(
      normalizePath(`${this.manifest.dir}/wasm-runtime/omd2typst_bg.wasm`)
    );
    setOmd2TypstWasmPath(omd2typstWasmPath);

    this.registerCommands();
    this.registerContextMenus();
    this.addSettingTab(new Omd2TypstSettingTab(this.app, this));
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

Expected: `main.js` created in the repo root (~12 KB), no errors. The `wasm-runtime/` directory contains `omd2typst_bg.wasm`.

- [ ] **Step 3: Install into a test vault manually**

Copy the plugin to your Obsidian vault's plugins folder:

```bash
VAULT=~/path/to/your/vault
mkdir -p "$VAULT/.obsidian/plugins/omd2typst"
cp main.js manifest.json "$VAULT/.obsidian/plugins/omd2typst/"
cp -r wasm-runtime "$VAULT/.obsidian/plugins/omd2typst/"
# Ensure typst is installed: typst --version
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
| 7 | `feat: add omd2typst WASM wrapper and typst CLI helper` |
| 8 | `feat: add exporter pipeline (MD → Typst → PDF via system typst CLI)` |
| 9 | `feat: register all commands and context menus in plugin main` |
| 10 | `feat: add settings tab UI (templates, language, output, frontmatter)` |
| 11 | `chore: verify end-to-end build and smoke tests pass` |
