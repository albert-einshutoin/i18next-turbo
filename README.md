# i18next-turbo

[![npm version](https://img.shields.io/npm/v/i18next-turbo.svg)](https://www.npmjs.com/package/i18next-turbo)
[![npm downloads](https://img.shields.io/npm/dm/i18next-turbo.svg)](https://www.npmjs.com/package/i18next-turbo)
[![Node.js](https://img.shields.io/node/v/i18next-turbo.svg)](https://www.npmjs.com/package/i18next-turbo)
[![CI](https://github.com/albert-einshutoin/i18next-turbo/actions/workflows/ci.yml/badge.svg)](https://github.com/albert-einshutoin/i18next-turbo/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/albert-einshutoin/i18next-turbo/graph/badge.svg)](https://codecov.io/gh/albert-einshutoin/i18next-turbo)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-DEA584?logo=rust)](https://www.rust-lang.org/)

Fast i18next key extraction for modern TypeScript/JavaScript codebases.

`i18next-turbo` is a Rust + SWC based extractor compatible with i18next-style workflows. It is designed for fast CI runs and low-latency watch mode.
Latest stable release: `v0.5.3`.

## Install

```bash
npm install --save-dev i18next-turbo
```

The package resolves a platform-specific binary through `optionalDependencies`.
For details (platform mapping, fallback behavior, troubleshooting), see [docs/installation.md](docs/installation.md).

## Quick Start

1. Initialize config:

```bash
i18next-turbo init
```

2. Extract keys once:

```bash
i18next-turbo extract
```

3. Run watch mode during development:

```bash
i18next-turbo watch
```

## Minimal Config

Create `i18next-turbo.json` in your project root:

```json
{
  "input": ["src/**/*.{ts,tsx,js,jsx}"],
  "output": "locales",
  "locales": ["en", "ja"],
  "defaultNamespace": "translation",
  "functions": ["t", "i18n.t"]
}
```

`output` is the locale root directory. File naming and extension are derived from `outputFormat`.

## Format Contract

- Supported locale output formats: `json`, `json5`, `js`/`js-esm`, `js-cjs`, `ts`
- Guaranteed consistent handling across `extract`, `status`, `check`, `sync`, and `rename-key`
- `locize` currently supports `json` and `json5` only
- The Node API returns structured objects on both native-addon and CLI-fallback paths

## Compatibility Policy

- First-class target: i18next-style key extraction workflows for TS/JS codebases
- Config auto-discovery supports `i18next-turbo.json` plus selected `i18next-parser` / `i18next.config.*` shapes
- This is not a drop-in implementation of every `i18next-parser` plugin or config edge case
- Watch mode, plugin hooks, and runtime transforms are project features, not compatibility guarantees with other tools

## CLI Commands

- `extract`: extract keys and sync locale files
- `watch`: watch files and sync continuously
- `sync`: sync from extracted cache/results
- `check`: detect/remove dead keys
- `lint`: detect hardcoded user-facing strings
- `status`: show translation progress
- `typegen`: generate TypeScript resource types
- `rename-key`: rename translation keys safely
- `migrate-config`: migrate config from i18next-parser style

## Docs

- [Installation and Binary Resolution](docs/installation.md)
- [API Reference](docs/api.md)
- [Compatibility and Format Guarantees](docs/compatibility.md)
- [Plugin Runtime Design](docs/plugin-runtime.md)
- [Parser Migration Plan](docs/parser-migration.md)
- [Migration Guide](docs/migration-guide.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Usage Examples](docs/usage-examples.md)
- [Performance Testing](docs/performance-testing.md)
- [Changelog](CHANGELOG.md)

## Contributing

- [Contributing Guide](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security Policy](SECURITY.md)
