# i18next-turbo TODO List

This document organizes the implementation status and future tasks for i18next-turbo.

## 📊 Implementation Summary

- ✅ **Done**: Most of Phase 2 (full Trans support, useTranslation/getFixedT/selector scope resolution)
- ✅ **Done**: Phase 3 main commands (status, sync, lint, check, typegen, init, rename-key)
- ✅ **Done**: Nested translations (configurable nestingPrefix/suffix/separator), flat keys, extraction from comments
- ✅ **Done**: Technical improvements (tempfile atomic writes, key conflict reporting, glob streaming, NFC optimization, indent detection/preservation, FileSystem trait integration)
- ✅ **Done**: Robustness (error message matrix display, no Silent Failure, preserve existing JSON style, mockable FS abstraction)
- ✅ **Done**: Language-specific plural categories (ICU-based) and ordinal plural key generation
- ✅ **Done**: returnObjects protection (`key.*` marker for child key preservation)
- ✅ **Done**: Phase 1 (npm distribution + CI/CD + release/publish workflow)

---

## 🎯 i18next-turbo Scope Redefinition (2026-02-10)

Full feature copy of i18next-cli is not the goal. `i18next-turbo` positions "fast extraction + safe sync + production CLI" as its core value.

### 100% Definition (Turbo Core v1)

The following 25 items define **100%** for Turbo Core v1:

1. `t('...')` / `i18n.t('...')` extraction
2. namespace / keySeparator / flat key support
3. `<Trans>` extraction for `i18nKey/ns/defaults/children`
4. `<Trans>` `count/context` (including dynamic context)
5. `useTranslation` scope resolution (including `useTranslationNames`)
6. `getFixedT` scope resolution
7. selector API (`t($ => $.a.b)`) extraction
8. template literal (static) extraction
9. nested translation extraction (configurable prefix/suffix/separator)
10. nested translation options (`count/context/ordinal`) reflected
11. Language-specific cardinal plurals (ICU)
12. ordinal plurals
13. `returnObjects: true` preservation marker (`key.*`)
14. Comment extraction and disable comments
15. ignore patterns
16. preservePatterns + removeUnusedKeys
17. JSON sync preserving existing translations (non-destructive)
18. Conflict detection and warnings
19. outputFormat (json/json5/js/ts)
20. Indent/style preservation + atomic write
21. `extract` / `watch`
22. `status` / `check` / `sync`
23. `lint` / `rename-key`
24. `typegen` / `init` / `migrate`
25. JS/TS config loading (Node wrapper + Rust)

### Current Implementation Rate (Turbo Core v1)

- **25 / 25 = 100%**
- Incomplete (Core v1 basis): none

### Items Not Included in Core v1 (for comparison)

The following are "i18next-cli compatibility extensions" and are not counted toward Turbo Core v1:

1. Plugin system
2. Heuristic auto config detection (running without config file)
3. `output` / `defaultValue` function form
4. `mergeNamespaces`
5. Advanced Locize option set
6. Typegen selector optimize mode

---

## 🚀 Phase 1: Distribution & Foundation (v0.5.0 target)

### Task 1.1: napi-rs integration and hybrid setup

#### 1.1.1: Cargo.toml updates
- [x] Add `napi` crate (with version)
- [x] Add `napi-derive` crate
- [x] Add `[lib]` section with `crate-type = ["cdylib", "rlib"]`
- [x] Add `napi-build` to `[build-dependencies]`

#### 1.1.2: Node.js API in src/lib.rs
- [x] Export functions with `#[napi]` macro
- [x] Make `extract()` callable from Node.js
- [x] Make `watch()` callable from Node.js
- [x] Function to convert config object to Rust `Config` (via JSON string)
- [x] Map errors to `napi::Error`

#### 1.1.3: package.json
- [x] Create `package.json`
- [x] Set `name`, `version`, `description`, `license`
- [x] Set `bin` for CLI entry point
- [x] Set `main` for Node.js API entry point
- [x] OS-specific binaries in `optionalDependencies`
  - `i18next-turbo-darwin-x64`, `i18next-turbo-darwin-arm64`
  - `i18next-turbo-win32-x64`, `i18next-turbo-win32-ia32`
  - `i18next-turbo-linux-x64`, `i18next-turbo-linux-arm64`
- [x] Add `postinstall` script for binary download

#### 1.1.4: Node.js wrapper
- [x] Create `bin/cli.js` (wrapper that invokes Rust binary)
- [x] Create `lib/index.js` (Node.js API entry point)
- [x] Implement NAPI calls (`extract`, `watch`)
- [x] Implement JS/TS config loading
  - Load `i18next-parser.config.js`
  - Load `i18next.config.ts` (via `jiti` or `ts-node`)
  - Convert config object to JSON string and pass to Rust binary

#### 1.1.5: Build scripts
- [x] Create `build.rs` (using napi-build)
- [x] Cross-compilation setup
- [x] Binary packaging script

#### Acceptance criteria
- [x] `npm install .` succeeds locally
- [x] `node -e "require('./').extract(...)"` works
- [x] `npx i18next-turbo extract` works

---

### Task 1.2: CI/CD (GitHub Actions)

#### 1.2.1: GitHub Actions workflow
- [x] Create `.github/workflows/ci.yml`
- [x] Matrix strategy for OS builds
  - `windows-latest`
  - `macos-latest` (x64, arm64)
  - `ubuntu-latest` (x64, arm64)
- [x] Rust toolchain setup
- [x] Run `cargo build --release` on each OS
- [x] Archive build artifacts

#### 1.2.2: Release workflow
- [x] Create `.github/workflows/release.yml`
- [x] Trigger on tag push
- [x] Build for all OS
- [x] Upload binaries to GitHub Releases
- [x] npm publish
  - `NPM_TOKEN` secret
  - Run `npm publish`

#### 1.2.3: npm package config
- [x] Add `files` to `package.json`
- [x] Create `.npmignore`
- [x] Version automation

#### Acceptance criteria
- [x] Binaries listed on GitHub Releases for each OS
- [x] Package published to npm registry
- [x] `npm install i18next-turbo` works

### Task 1.3: CLI wiring for implemented features ✅

#### 1.3.1: TypeScript typegen command
- [x] Add `Typegen` variant to `Commands` enum in `src/main.rs`
- [x] Implement `typegen` subcommand
  - `--output` (output path for type definition file)
  - `--default-locale` option
- [x] Call `generate_types()` from `src/typegen.rs`
- [x] Read `types` section from config
- [x] Add `--generate-types` option to run typegen when running `extract`

#### 1.3.2: Dead key detection command ✅
- [x] Add `Check` or `Cleanup` variant to `Commands` enum in `src/main.rs`
- [x] Implement `check` or `cleanup` subcommand
  - `--remove` (whether to remove unused keys)
  - `--dry-run` (preview before removal)
- [x] Call `find_dead_keys()` and `purge_dead_keys()` from `src/cleanup.rs`
- [x] Report detected dead keys
- [x] Confirmation prompt when removing (when `--remove` is set)

#### Acceptance criteria
- [x] `i18next-turbo typegen` works
- [x] `i18next-turbo check` works
- [x] `i18next-turbo extract --generate-types` runs extraction and typegen together
- [x] Features documented in README are usable

---

## ⚛️ Phase 2: Full i18next compatibility (v1.0.0 target)

### Task 2.1: Full `<Trans>` support ✅

#### 2.1.1: Key extraction from children ✅
- [x] Visitor to traverse `JSXElement` children
- [x] Extract text from `JSXText` nodes
- [x] Use children text as key when `i18nKey` is absent
- [x] Preserve HTML tags (`<strong>`, `<br>`, etc.)
- [x] Handle interpolation (`{{name}}`)

#### 2.1.2: `ns` attribute extraction ✅
- [x] Extract `ns` from `JSXOpeningElement`
- [x] Set namespace on `ExtractedKey`
- [x] Add test cases

#### 2.1.3: `count` attribute extraction ✅
- [x] Extract `count` from `JSXOpeningElement`
- [x] Generate plural keys (`_one`, `_other`)
- [x] Support combination of `count` and `context`

#### 2.1.4: `context` attribute extraction ✅
- [x] Extract `context` from `JSXOpeningElement`
- [x] Generate context keys (`key_context`)
- [x] Parse dynamic context (e.g. ternary)

#### Acceptance criteria
- [x] `<Trans>Hello</Trans>` extracts `Hello` as key
- [x] `<Trans ns="common">content</Trans>` stored in `common` namespace
- [x] `<Trans count={5}>item</Trans>` yields `item_one`, `item_other`
- [x] `<Trans context="male">friend</Trans>` yields `friend_male`

---

### Task 2.2: Plurals and context (full support)

#### 2.2.1: Language-specific plural categories
- [x] Implement `Intl.PluralRules`-equivalent in Rust
  - Use `icu_plurals` or `intl_pluralrules` crate, or custom (CLDR)
- [x] Get plural categories for all configured languages
  - `zero`, `one`, `two`, `few`, `many`, `other`
- [x] Generate keys per language category
- [x] Use base key for single-category languages (`other` only)

#### 2.2.2: Ordinal plurals
- [x] Detect `ordinal` plural type
- [x] Generate keys like `key_ordinal_one`, `key_ordinal_other`
- [x] Enable/disable via config (`ordinal: true`)

#### 2.2.3: Context and plural combination ✅
- [x] Handle both `context` and `count`
- [x] Generate `key_context_one`, `key_context_other`
- [x] Control base plural key generation (`generateBasePluralForms`)

- [x] `t('apple', { count: 5 })` yields basic plural keys (`_one`, `_other`)
- [x] `t('apple', { count: 5 })` yields language-specific categories (ICU)
- [x] Japanese (`other` only) yields only `apple`
- [x] Russian yields `apple_one`, `apple_few`, `apple_many`, `apple_other`

---

### Task 2.3: Advanced extraction patterns

#### 2.3.1: `useTranslation` hook scope resolution ✅
- [x] ScopeManager-like logic
- [x] Parse `useTranslation('ns', { keyPrefix: 'user' })`
- [x] Track variable scope
- [x] Apply `keyPrefix` logic
- [x] Array destructuring: `const [t] = useTranslation()`
- [x] Object destructuring: `const { t } = useTranslation()`
- [x] Alias: `const { t: translate } = useTranslation()`

#### 2.3.2: `getFixedT` support ✅
- [x] Detect `i18next.getFixedT()` calls
- [x] Extract namespace and keyPrefix from arguments
- [x] Attach scope to variable
- [x] Handle `const t = getFixedT('en', 'ns', 'prefix')`

#### 2.3.3: Selector API support
- [x] Detect `t($ => $.key.path)` pattern
- [x] Extract key path from arrow function argument
- [x] Type-safe key selection (with typegen)

#### 2.3.4: Function alias tracking
- [x] Detect aliases like `const translate = t`
- [x] Track alias calls
- [x] Inherit scope

#### 2.3.5: Dynamic context resolution
- [x] Parse ternary: `context: isMale ? 'male' : 'female'`
- [x] Enumerate possible values and generate multiple keys
- [x] Warn when unresolvable

#### 2.3.6: Nested translations ✅
- [x] Detect `$t(key)` (nested in strings)
- [x] Configurable `nestingPrefix` / `nestingSuffix` (default `$t(`, `)`)
- [x] Configurable `nestingOptionsSeparator` (default `,`)
- [x] Parse `$t(key, { options })` in strings
- [x] Extract plurals/context from nested keys
- [x] Extract nested in default values
- [x] Extract from Trans `defaults` prop

#### 2.3.7: returnObjects support
- [x] Detect `t('key', { returnObjects: true })`
- [x] Preserve structured (object) content
- [x] Manage objectKeys set (`key.*` marker)
- [x] Generate pattern to keep object children (`key.*`)

#### 2.3.8: Template literals ✅
- [x] Detect `t(\`key\`)` (backtick strings)
- [x] Handle `Expr::Tpl` (Template Literal)
- [x] Extract static template literals (no variables)
  - `t(\`hello\`)` → extract `hello`
- [x] Warn or skip when variables are embedded
  - `t(\`hello_${name}\`)` → skip (dynamic key)
- [x] Unified handling for `Lit::Str` and `TemplateLiteral`
- [x] Add test cases

#### Acceptance criteria
- [x] `const { t } = useTranslation('common', { keyPrefix: 'user' }); t('name')` → `common:user.name`
- [x] `const t = getFixedT('en', 'ns', 'prefix'); t('key')` → `ns:prefix.key`
- [x] `t($ => $.user.profile)` → `user.profile`
- [x] `t('You have $t(item_count, {"count": {{count}} })')` → `item_count_one`, `item_count_other`
- [x] `t('countries', { returnObjects: true })` keeps existing `countries` object

---

### Task 2.4: JS/TS config (Interop)

#### 2.4.1: Config loading in Node.js wrapper
- [x] Detect config in `bin/cli.js`
  - `i18next-turbo.json`
  - `i18next-parser.config.js`, `i18next-parser.config.ts`
  - `i18next.config.ts`, `i18next.config.js`
- [x] Load JS/TS via `require()` or `jiti`
- [x] Convert config to JSON string
- [x] Pass JSON string to Rust binary

#### 2.4.2: JSON parse on Rust side
- [x] CLI argument to accept JSON string
- [x] Parse with `serde_json`
- [x] Map to existing `Config` struct

#### 2.4.3: Config compatibility
- [x] Support `i18next-parser.config.js` shape
- [x] Property mapping (e.g. `$LOCALE` → `{{language}}`)
- [x] Default values

#### 2.4.4: Heuristic config detection
- [x] Full project-structure auto-detection
- [x] Search common locale paths (`locales/en/*.json`, `public/locales/en/*.json`, etc.)
- [x] Generate config from detected structure
- [x] `status` and `lint` work without config file

#### Acceptance criteria
- [x] Users can use existing JS config as-is
- [x] TypeScript config loadable
- [x] Config validation and error messages
- [x] `status` works without config file

---

## 🚀 Phase 3: Differentiation (v2.0.0 target)

### Task 3.1: Additional commands

#### 3.1.1: `status` ✅ (basic implementation done)
- [x] Translation completion (key-based)
- [x] Per-locale summary
- [x] Key-level report (`status [locale]`)
- [x] Namespace filter (`--namespace`)
- [x] Progress bar
- [x] Non-zero exit when incomplete

#### 3.1.2: `sync` ✅
- [x] Load primary language file
- [x] Compare with secondary language files
- [x] Add missing keys (with default value)
- [x] Remove unused keys (`--remove-unused`)
- [x] Report changed files
- [x] `--dry-run`

#### 3.1.3: `lint` ✅
- [x] Detect hardcoded strings
- [x] Parse JSX text nodes
- [x] Parse JSX attributes (`title`, `alt`, `placeholder`, `aria-label`, etc.)
- [x] Ignore rules (`ignoredTags`: script, style, code, pre)
- [x] Error report
- [x] `--fail-on-error` (CI)
- [x] Watch mode

#### 3.1.4: `rename-key` ✅
- [x] Find keys in source files
- [x] Regex-based key replacement (source)
- [x] Rename keys in translation files
- [x] Conflict detection
- [x] Dry-run
- [x] Change report
- [x] `--locales-only` (translation files only)

#### 3.1.5: `init` ✅
- [x] CLI options for config values
- [x] Auto-generate i18next-turbo.json
- [x] Auto-create locale directories
- [x] Next-step guidance
- [x] Interactive config wizard (optional)
- [x] Project structure auto-detection

#### 3.1.6: `migrate-config`
- [x] Detect legacy config
- [x] Conversion logic
- [x] Migrate to new format
- [x] Warning messages

---

### Task 3.2: Advanced config options

#### 3.2.1: Extract command options
- [x] `--sync-primary`: sync primary with default values from code
- [x] `--sync-all`: sync all locales
- [x] `--dry-run`: preview without writing ✅
- [x] `--ci`: non-zero exit when files updated ✅

#### 3.2.2: Config file extension (basic options)
- [x] `preservePatterns`: preserve dynamic key patterns (glob array) ✅
- [x] `preserveContextVariants`: preserve context variants
- [x] `generateBasePluralForms`: control base plural generation ✅
- [x] `disablePlurals`: disable plurals ✅
- [x] `extractFromComments`: extract from comments ✅
- [x] `removeUnusedKeys`: remove unused (default `true`) ✅
- [x] `ignore`: file patterns to exclude (glob array) ✅

#### 3.2.3: Separators and interpolation ✅
- [x] `keySeparator` (default `'.'`)
- [x] `nsSeparator` (default `':'`)
- [x] `contextSeparator` (default `'_'`)
- [x] `pluralSeparator` (default `'_'`)
- [x] `keySeparator: ""` for flat keys
- [x] `nsSeparator: false` to disable
- [x] `interpolationPrefix` (default `'{{'`) ✅
- [x] `interpolationSuffix` (default `'}}'`) ✅
- [x] `nestingPrefix` (default `'$t('`) ✅
- [x] `nestingSuffix` (default `')'`) ✅
- [x] `nestingOptionsSeparator` (default `','`) ✅

#### 3.2.4: Language and default value
- [x] `primaryLanguage` (default `locales[0]`)
- [x] `secondaryLanguages` (auto or explicit)
- [x] `defaultValue` (partial)
  - String: `''` ✅ (`ExtractedKey.default_value` used)
  - Function: `(key, namespace, language, value) => string` - [x] implemented (Node wrapper post-process)
- [x] `defaultNS` (default `'translation'` ✅, `false` for no namespace ✅)

#### 3.2.5: Sort and format
- [x] `sort`: key sort (alphabetical via `sort_keys_alphabetically`)
  - Boolean `true` ✅
  - Function `(a, b) => number` - [x] implemented (Node wrapper post-process)
- [x] `indentation`: JSON indent
  - Number (e.g. `2`) ✅
  - String `'\t'` or `'  '` ✅

#### 3.2.6: Trans component config
- [x] `transKeepBasicHtmlNodesFor`: HTML tags to keep in Trans (default `['br', 'strong', 'i']`)
- [x] `transComponents`: Trans component names to extract (default `['Trans']`) ✅

#### 3.2.7: Output path as function
- [x] `output`: function form
  - String: `'locales/{{language}}/{{namespace}}.json'`
  - Function: `(language: string, namespace?: string) => string`

---

### Task 3.3: Output format variety

#### 3.3.1: JSON5
- [x] Integrate JSON5 parser (`json5` crate)
- [x] Preserve comments
- [x] Preserve trailing commas
- [x] Preserve number format

#### 3.3.2: TypeScript output ✅
- [x] `outputFormat: 'ts'`
- [x] Generate `export default { ... } as const`
- [x] Type safety

#### 3.3.3: JavaScript output ✅
- [x] `outputFormat: 'js'` or `'js-esm'`: ES Module (`export default`)
- [x] `outputFormat: 'js-cjs'`: CommonJS (`module.exports`)
- [x] Auto-select by project module system

#### 3.3.4: Namespace merge
- [x] `mergeNamespaces: true`
- [x] Merge all namespaces into one file per language
- [x] Output path adjustment (no `{{namespace}}` placeholder)
- [x] Detect existing file structure (namespaced vs flat)

---

### Task 3.4: Comment extraction ✅

#### 3.4.1: Comment pattern detection ✅
- [x] `// t('key', 'default')`
- [x] `/* t('key') */`
- [x] Object form: `// t('key', { defaultValue: '...', ns: '...' })`
- [x] Multi-line comments
- [x] Backtick: `// t(\`key\`)`
- [x] Plural in comments
- [x] Context in comments

#### 3.4.2: Scope resolution
- [x] Resolve `useTranslation` refs in comments
- [x] Apply `keyPrefix`
- [x] Resolve namespace

#### 3.4.3: Config option
- [x] `extractFromComments: true/false`
- [x] Enabled by default

---

### Task 3.5: Locize integration (optional)

#### 3.5.1: Locize CLI integration
- [x] Check `locize-cli` dependency
- [x] Implement `locize-sync`
- [x] Implement `locize-download`
- [x] Implement `locize-migrate`

#### 3.5.2: Credentials
- [x] Interactive credential setup
- [x] Read from env
- [x] Save to config file

#### 3.5.3: Locize config options
- [x] `locize.projectId`
- [x] `locize.apiKey` (env recommended)
- [x] `locize.version` (default `'latest'`)
- [x] `locize.updateValues`
- [x] `locize.sourceLanguageOnly`
- [x] `locize.compareModificationTime`
- [x] `locize.cdnType` (`'standard'` or `'pro'`)
- [x] `locize.dryRun`

---

### Task 3.6: TypeScript typegen extension

#### 3.6.1: Typegen config
- [x] `types.input`: source locale file pattern
- [x] `types.output`: main type definition path
- [x] `types.resourcesFile`: resources interface path
- [x] `types.enableSelector`: selector API (`true`, `false`, `'optimize'`)
- [x] `types.indentation`: indent for type file

#### 3.6.2: Selector API typegen
- [x] Typegen when `enableSelector: true`
- [x] Optimized typegen when `enableSelector: 'optimize'`
- [x] Type-safe key selection

#### 3.6.3: Merged namespace typegen
- [x] Typegen when `mergeNamespaces: true`
- [x] Typegen for files with multiple namespaces

---

### Task 3.7: Lint config details

#### 3.7.1: Lint config options
- [x] `lint.ignoredAttributes`: JSX attributes to ignore
- [x] `lint.ignoredTags`: JSX tags to ignore
- [x] `lint.acceptedAttributes`: allowlist of attributes
- [x] `lint.acceptedTags`: allowlist of tags
- [x] `lint.ignore`: file patterns to exclude

#### 3.7.2: Lint logic
- [x] Default recommended attributes (`alt`, `title`, `placeholder`, `aria-label`, etc.)
- [x] Default recommended tags (`p`, `span`, `div`, `button`, `label`, etc.)
- [x] Allowlist/denylist precedence
- [x] Ignore content inside Trans

---

### Task 3.8: Plugin system

#### 3.8.1: Plugin API design
- [x] Define `Plugin` interface
- [x] Plugin lifecycle hooks (setup / onLoad / onVisitNode / onEnd / afterSync)
  - `setup`: init
  - `onLoad`: transform before file parse (planned)
  - `onVisitNode`: on AST node visit (planned)
  - `onEnd`: after extraction
  - `afterSync`: after sync

#### 3.8.2: Plugin examples
- [x] HTML plugin example
- [x] Handlebars plugin example
- [x] Custom extraction pattern example

#### 3.8.3: Plugin loading
- [x] Load plugins from config
- [x] Plugin error handling

---

## 🧪 Testing and QA

### Task 4.1: Test coverage
- [x] Unit tests per extraction pattern
- [x] Integration tests
- [x] Edge case tests
- [x] Performance tests

### Task 4.2: Documentation
- [x] API documentation
- [x] Usage examples
- [x] Migration guide
- [x] Troubleshooting guide

---

## 📝 Notes

### Implemented features
- ✅ Basic `t()` extraction
- ✅ `i18n.t()` extraction
- ✅ Full `<Trans>` support (`i18nKey`, `ns`, `count`, `context`, `defaults`, `children`)
- ✅ Namespace support
- ✅ Basic plurals (`_one`, `_other`)
- ✅ Context support (string literals)
- ✅ Context + plural (`key_context_one`, `key_context_other`)
- ✅ Magic comment (`i18next-extract-disable`)
- ✅ JSON sync (preserve existing)
- ✅ Watch mode
- ✅ TypeScript typegen
- ✅ Dead key detection and removal
- ✅ `useTranslation` scope resolution (`keyPrefix`)
- ✅ `getFixedT` support
- ✅ Nested translations (`$t(...)`)
- ✅ Template literals (`t(\`key\`)`, static only)
- ✅ Comment extraction (including backtick)
- ✅ Flat keys (`keySeparator: ""`)

### Not implemented vs i18next-cli
- ✅ **CLI wiring** (typegen, check, status) done
- ✅ Template literals (`t(\`key\`)`)
- ✅ Nested translations (`$t(...)`)
- ✅ `returnObjects` (`key.*`)
- ✅ Flat keys (`keySeparator: ""`)
- ✅ Separators (`nsSeparator`, `contextSeparator`, `pluralSeparator`)
- ✅ Interpolation (`interpolationPrefix`, `interpolationSuffix`)
- ✅ Nesting (`nestingPrefix`, `nestingSuffix`, `nestingOptionsSeparator`)
- ✅ `primaryLanguage`
- ✅ `secondaryLanguages`
- ✅ `defaultValue` function form
- ✅ `sort` function form
- ✅ `indentation` (number/string)
- ✅ `output` function form
- ✅ `defaultNS: false`
- ✅ `transKeepBasicHtmlNodesFor`
- ✅ Plugin system (setup / onLoad / onVisitNode / onEnd / afterSync)
- ✅ Heuristic config detection
- ✅ JS output (`js`, `js-esm`, `js-cjs`)
- ✅ Typegen details (`enableSelector`)
- ✅ Lint details (`acceptedAttributes`, `acceptedTags`)

### Technical debt (resolved)
- [x] Error handling (lock-free, depth-limited recursion)
- [x] Log level config
- [x] Performance (BufReader/BufWriter, early dedup)
- [x] Memory (fold/reduce dedup)
- [x] File locking (fs2)
- [x] Regex safety (init tests, comments)
- [x] Atomic writes (tempfile)
- [x] Key conflict reporting (no Silent Failure, KeyConflict type)
- [x] Glob streaming (par_bridge, O(1) memory)
- [x] NFC optimization (is_nfc_quick)
- [x] Error format (file:line:col, IDE-friendly)
- [x] JSON indent detection/preservation
- [x] FileSystem trait (open_locked, atomic_write, mockable)

---

## 🎯 Priority matrix

### P0 (highest – implement first)
1. ~~**Task 1.3: CLI wiring**~~ ✅ **Done**
2. Task 1.1: napi-rs integration
3. Task 1.2: CI/CD
4. Task 2.4: JS/TS config loading

### P1 (high – for Phase 2 completion)
5. ~~Task 2.1: Full `<Trans>`~~ ✅ **Done**
6. Task 2.2: Language-specific plural categories
7. ~~Task 2.3.1: `useTranslation` scope resolution~~ ✅ **Done**
8. ~~Task 2.3.2: `getFixedT`~~ ✅ **Done**
9. ~~Task 2.3.8: Template literals~~ ✅ **Done**

### P2 (medium – differentiation)
8. ~~Task 3.1.1: `status`~~ ✅ **Done**
9. ~~Task 3.1.2: `sync`~~ ✅ **Done**
10. ~~Task 3.1.3: `lint`~~ ✅ **Done**
11. Task 3.2: Advanced config options

### P3 (low – extensions)
12. ~~Task 3.1.4: `rename-key`~~ ✅ **Done**
13. ~~Task 3.1.5: `init`~~ ✅ **Done**
14. Task 3.1.6: `migrate-config`
15. Task 3.3: Output format variety
16. ~~Task 3.4: Comment extraction~~ ✅ **Done**
17. Task 3.5: Locize integration
18. ~~Task 2.3.6: Nested translations~~ ✅ **Done**
19. Task 2.3.7: returnObjects support
20. ~~Task 3.2.3: Flat keys (keySeparator)~~ ✅ **Done**
21. Task 3.2.4–3.2.7: Detailed config options
22. Task 3.6: TypeScript typegen extension
23. Task 3.7: Lint config details
24. Task 3.8: Plugin system
25. Task 2.4.4: Heuristic config detection

---

## 📅 Milestones

### v0.5.0 (Phase 1 complete)
- [x] Distributable as npm package
- [x] CI/CD working
- [x] Basic Node.js API

### v1.0.0 (Phase 2 complete)
- [x] Full i18next compatibility
- [x] Easy migration from existing tools
- [x] Zero extraction gaps

### v2.0.0 (Phase 3 complete)
- [x] Differentiation features
- [x] Better DX
- [x] Ecosystem integration

---

## 🔍 Self-review (vs i18next-cli)

### Tasks added from evaluation report (2025-01-XX)

1. ~~**Task 1.3: CLI wiring**~~ ✅ **Done**
   - `typegen.rs` and `cleanup.rs` callable from CLI
   - Added `Typegen`, `Check`, `Status` to `Commands` enum
   - Added `extract --generate-types` option

2. ~~**Task 2.3.8: Template literals**~~ ✅ **Done**
   - Implemented `t(\`key\`)` detection
   - Added `Expr::Tpl` handling
   - Static template literal extraction and tests

### Previously added tasks

1. **Task 2.3.6: Nested translations**
   - Detect and extract `$t(...)`
   - Handle nested translations in strings

2. **Task 2.3.7: returnObjects**
   - Preserve structured content
   - Auto-keep object keys

3. **Task 2.4.4: Heuristic config detection**
   - Run without config file
   - Auto-detect project structure

4. **Task 3.2.3–3.2.7: Detailed config options**
   - Separators and interpolation
   - Language and default value
   - Sort and format
   - Trans component config
   - Output path as function

5. **Task 3.3.3: JavaScript output**
   - ES Module and CommonJS

6. **Task 3.6: TypeScript typegen extension**
   - Selector API typegen
   - Merged namespace typegen

7. **Task 3.7: Lint config details**
   - Allowlist/denylist
   - Default recommended lists

8. **Task 3.8: Plugin system**
   - Extensible architecture

### Checklist

- ✅ All major i18next-cli features are in this TODO
- ✅ Config options are covered
- ✅ Output format variety is considered
- ✅ Plugin system is planned
- ✅ Heuristic config detection is considered

---

Last updated: 2026-02-12 (v0.5.1 release assets verified on GitHub Releases, v2.0 milestone checklist synchronized).
