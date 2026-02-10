# API Documentation

## Node.js API (`lib/index.js`)

### `extract(config, options?)`
- Purpose: extract translation keys and sync locale files.
- Returns: `Promise<object>` (JSON-serializable result from native addon).

### `lint(config, options?)`
- Purpose: detect hardcoded user-facing strings.
- Returns: `Promise<object>`.

### `check(config, options?)`
- Purpose: detect dead keys and optionally remove them.
- Returns: `Promise<object>` with dead key details.

### `watch(config, options?)`
- Purpose: run continuous extraction.
- Returns: `Promise<void>` (long-running).

## CLI Commands

- `i18next-turbo extract`
- `i18next-turbo sync`
- `i18next-turbo lint`
- `i18next-turbo status`
- `i18next-turbo check`
- `i18next-turbo typegen`
- `i18next-turbo init`
- `i18next-turbo migrate-config`
- `i18next-turbo rename-key`
- `i18next-turbo watch`

## Plugin Hooks (Node wrapper)

- `setup(context)`
- `onLoad({ filePath, relativePath, source, ... })`
- `onVisitNode(node)`
- `onEnd(context)`
- `afterSync(context)`

For detailed examples, see:
- `examples/plugins/html-plugin.js`
- `examples/plugins/handlebars-plugin.js`
- `examples/plugins/custom-extraction-plugin.js`
