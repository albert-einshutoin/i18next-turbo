# API ドキュメント

## Node.js API (`lib/index.js`)

### `extract(config, options?)`
- 目的: 翻訳キー抽出とロケール同期。
- 戻り値: `Promise<object>`（ネイティブアドオン結果）。

### `lint(config, options?)`
- 目的: ハードコード文字列検出。
- 戻り値: `Promise<object>`。

### `check(config, options?)`
- 目的: 未使用キー検出（必要に応じて削除）。
- 戻り値: `Promise<object>`。

### `watch(config, options?)`
- 目的: 継続抽出。
- 戻り値: `Promise<void>`（長時間実行）。

## CLI コマンド

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

## プラグインフック（Nodeラッパー）

- `setup(context)`
- `onLoad({ filePath, relativePath, source, ... })`
- `onVisitNode(node)`
- `onEnd(context)`
- `afterSync(context)`
