# API Documentation

## Node.js API (`lib/index.js`)

### `extract(config, options?)`
- Purpose: extract translation keys and sync locale files.
- Returns: `Promise<object>`.
- Result shape:
  - `success`
  - `files_processed`
  - `unique_keys`
  - `keys_added`
  - `keys_removed`
  - `updated_files`
  - `warnings`
  - `extraction_warnings`
  - `conflicts`
  - `message`

### `lint(config, options?)`
- Purpose: detect hardcoded user-facing strings.
- Returns: `Promise<object>` with `files_checked` and `issues`.

### `check(config, options?)`
- Purpose: detect dead keys and optionally remove them.
- Returns: `Promise<object>` with `dead_keys` and `removed_count`.

### `watch(config, options?)`
- Purpose: run continuous extraction.
- Returns: `Promise<void>` (long-running).

## Node API Contract

- `lib/index.js` prefers the native addon when available.
- If the native addon is unavailable, it falls back to the CLI wrapper.
- Both paths return the same structured result shape for `extract`, `lint`, and `check`.
- `watch` is intentionally long-running and does not return a summary object.

## Locale Format Coverage

- `extract`, `status`, `check`, `sync`, and `rename-key` share the same locale format handling.
- Supported locale file formats: `json`, `json5`, `js`/`js-esm`, `js-cjs`, `ts`.
- `locize` is limited to `json` and `json5`.

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

## Plugin Runtime Error Policy

Set `pluginErrorMode` to control hook failure behavior.

```js
module.exports = {
  pluginErrorMode: 'continue', // or 'fail-fast'
  extract: {
    pluginErrorMode: 'fail-fast' // extract scope takes precedence
  }
};
```

- `continue` (default): log warning and continue.
- `fail-fast`: exit with non-zero code on plugin load/hook error.

For detailed examples, see:
- `examples/plugins/html-plugin.js`
- `examples/plugins/handlebars-plugin.js`
- `examples/plugins/custom-extraction-plugin.js`
