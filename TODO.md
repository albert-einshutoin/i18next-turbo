# i18next-turbo TODO List

This document tracks the implementation status and upcoming tasks for i18next-turbo.

## 📊 Implementation Summary

- ✅ **Done**: Most of Phase 2 (full Trans support, useTranslation/getFixedT/selector scope resolution)
- ✅ **Done**: Phase 3 main commands (status, sync, lint, check, typegen, init, rename-key)
- ✅ **Done**: Nested translations (configurable nestingPrefix/suffix/separator), flat keys, extraction from comments
- ✅ **Done**: Technical improvements (tempfile atomic writes, key conflict reporting, glob streaming, NFC optimization, indent detection/preservation, FileSystem trait integration)
- ✅ **Done**: Robustness (error message matrix display, no Silent Failure, preserve existing JSON style, mockable FS abstraction)
- ✅ **Done**: Language-specific plural categories (ICU-based) and ordinal plural key generation
- ✅ **Done**: returnObjects protection (`key.*` marker for child key preservation)
- ⚠️ **Partial**: Phase 1 (npm distribution base is ready; CI/CD not implemented)

---

## 🎯 i18next-turbo Scope (2026-02-10)

Full feature parity with i18next-cli is not the goal. `i18next-turbo` focuses on "fast extraction + safe sync + production CLI" as core value.

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
- Incomplete (Core v1): none

### Items Not Counted in Core v1 (for comparison)

These are "i18next-cli compatibility extensions" and are not part of Turbo Core v1:

1. Plugin system
2. Heuristic auto config detection (running without config file)
3. `output` / `defaultValue` function form
4. `mergeNamespaces`
5. Advanced Locize options
6. Typegen selector optimize mode

---

## 🚀 Phase 1: Distribution & Foundation (v0.5.0 target)

### Task 1.1: napi-rs integration and hybrid setup

#### 1.1.1: Cargo.toml updates
- [x] Add `napi` crate (with version)
- [x] Add `napi-derive` crate
- [x] Add `[lib]` with `crate-type = ["cdylib", "rlib"]`
- [x] Add `napi-build` to `[build-dependencies]`

#### 1.1.2: Node.js API in src/lib.rs
- [x] Export functions with `#[napi]` macro
- [x] Make `extract()` callable from Node.js
- [x] Make `watch()` callable from Node.js
- [x] Convert config object to Rust `Config` (via JSON string)
- [x] Map errors to `napi::Error`

#### 1.1.3: package.json
- [x] Create `package.json`
- [x] Set `name`, `version`, `description`, `license`
- [x] Set `bin` for CLI entry
- [x] Set `main` for Node.js API entry
- [x] OS-specific binaries in `optionalDependencies`
- [x] Add `postinstall` script for binary download

#### 1.1.4: Node.js wrapper
- [x] Create `bin/cli.js` (calls Rust binary)
- [x] Create `lib/index.js` (Node API entry)
- [x] Call NAPI functions (`extract`, `watch`)
- [x] Load JS/TS config (i18next-parser.config.js, i18next.config.ts via jiti/ts-node), pass as JSON to Rust

#### 1.1.5: Build scripts
- [x] Create `build.rs` (napi-build)
- [x] Cross-compile setup
- [x] Binary packaging script

#### Acceptance criteria
- [ ] `npm install .` succeeds locally
- [ ] `node -e "require('./').extract(...)"` works
- [ ] `npx i18next-turbo extract` works

---

### Task 1.2: CI/CD (GitHub Actions)

#### 1.2.1: GitHub Actions workflow
- [x] Create `.github/workflows/ci.yml`
- [x] Matrix for OS (windows, macos x64/arm64, ubuntu x64/arm64)
- [x] Rust toolchain setup, `cargo build --release`, archive artifacts

#### 1.2.2: Release workflow
- [x] Create `.github/workflows/release.yml`
- [x] Trigger on tag push, build all OS, upload to GitHub Releases, npm publish with NPM_TOKEN

#### 1.2.3: npm package config
- [x] `files` in package.json, .npmignore, version automation

#### Acceptance criteria
- [ ] Binaries on GitHub Releases
- [ ] Package on npm registry
- [ ] `npm install i18next-turbo` works

### Task 1.3: CLI wiring for implemented features ✅

- [x] Typegen and Check commands, `extract --generate-types`, README features usable.

---

## ⚛️ Phase 2: Full i18next compatibility (v1.0.0 target)

### Task 2.1: Full `<Trans>` support ✅

- [x] Children/key extraction, `ns`, `count`, `context` (including dynamic), tests.

### Task 2.2: Plurals and context

- [x] ICU plural categories, ordinal plurals, context+plural combination, generateBasePluralForms.

### Task 2.3: Advanced extraction patterns

- [x] useTranslation/getFixedT/selector/alias, dynamic context, nested translations, returnObjects, template literals.

### Task 2.4: JS/TS config (Interop)

- [x] Config detection and loading (Node), JSON parse in Rust, heuristic detection for status/lint/init without config.

---

## 🚀 Phase 3: Differentiation (v2.0.0 target)

### Task 3.1: Additional commands

- [x] status, sync, lint, rename-key, init, migrate-config (with noted optional items).

### Task 3.2: Advanced config options

- [x] Extract flags, preservePatterns, separators, interpolation, nesting, primaryLanguage, sort/indentation, Trans settings, output function form.

### Task 3.3: Output formats

- [x] JSON5, TS/JS output, mergeNamespaces (with remaining sub-items).

### Task 3.4: Comment extraction ✅

- [x] Comment patterns, scope resolution, extractFromComments option.

### Task 3.5: Locize integration (optional)

- [x] locize-cli integration, env-based auth, locize options.

### Task 3.6: TypeScript typegen extension

- [x] types.* options, selector typegen, merged namespaces.

### Task 3.7: Lint config

- [x] ignoredAttributes/Tags, acceptedAttributes/Tags, ignore, default recommendations.

### Task 3.8: Plugin system

- [ ] Plugin API, lifecycle hooks, examples, loading from config.

---

## 🧪 Testing and QA

- [ ] Unit tests per extraction pattern, integration tests, edge cases, performance tests.
- [ ] API docs, examples, migration guide, troubleshooting.

---

## 📝 Notes

### Implemented

- t()/i18n.t(), Trans (i18nKey, ns, count, context, defaults, children), namespaces, plurals, context, magic comments, JSON sync, watch, typegen, dead key check, useTranslation/getFixedT, nested $t(), template literals, comment extraction, flat keys, returnObjects (key.*), separators, output function, defaultNS: false, transKeepBasicHtmlNodesFor, JS output, types.enableSelector, lint acceptedAttributes/Tags.

### Not implemented (vs i18next-cli)

- defaultValue/sort function form, plugin system.

### Technical debt (resolved)

- Error handling, logging, performance, memory, file locking, regex safety, atomic writes, key conflict reporting, glob streaming, NFC, error format, indent preservation, FileSystem trait.

---

## 🎯 Priority matrix

- P0: Task 1.1 napi-rs, 1.2 CI/CD, 2.4 JS/TS config.
- P1–P3: Remaining tasks as in Japanese TODO; many already done.

---

## 📅 Milestones

- v0.5.0: npm distributable, CI/CD, Node API.
- v1.0.0: i18next compatibility, migration, zero extraction gaps.
- v2.0.0: Differentiation, DX, ecosystem.

---

Last updated: 2026-02-10 (comment scope resolution tests, lint watch, init auto-detect, locize advanced options, JSON5 style preservation updates).
