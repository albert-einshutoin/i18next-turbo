# i18next-turbo

Fast i18next key extraction for modern TypeScript/JavaScript codebases.

`i18next-turbo` is a Rust + SWC based extractor compatible with i18next-style workflows. It is designed for fast CI runs and low-latency watch mode.

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
  "output": "locales/$LOCALE/$NAMESPACE.json",
  "locales": ["en", "ja"],
  "defaultNamespace": "translation",
  "functions": ["t", "i18n.t"]
}
```

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
- [Migration Guide](docs/migration-guide.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Usage Examples](docs/usage-examples.md)
- [Performance Testing](docs/performance-testing.md)

## Contributing

- [Contributing Guide](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security Policy](SECURITY.md)
