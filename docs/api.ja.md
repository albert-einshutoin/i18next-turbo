# API ドキュメント

## Node.js API (`lib/index.js`)

### `extract(config, options?)`
- 目的: 翻訳キー抽出とロケール同期。
- 戻り値: `Promise<object>`。
- 結果フィールド:
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
- 目的: ハードコード文字列検出。
- 戻り値: `files_checked` と `issues` を持つ `Promise<object>`。

### `check(config, options?)`
- 目的: 未使用キー検出（必要に応じて削除）。
- 戻り値: `dead_keys` と `removed_count` を持つ `Promise<object>`。

### `watch(config, options?)`
- 目的: 継続抽出。
- 戻り値: `Promise<void>`（長時間実行）。

## Node API 契約

- `lib/index.js` は native addon があればそれを優先します。
- native addon が無い場合は CLI wrapper にフォールバックします。
- `extract` / `lint` / `check` はどちらの経路でも同じ構造化結果を返します。
- `watch` は長時間実行のため、要約結果オブジェクトは返しません。

## locale format の適用範囲

- `extract`, `status`, `check`, `sync`, `rename-key` は同じ locale format handling を共有します。
- 対応 locale 形式: `json`, `json5`, `js`/`js-esm`, `js-cjs`, `ts`
- `locize` は `json` と `json5` のみ対応です。

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

## プラグインエラーポリシー

`pluginErrorMode` でフック失敗時の挙動を指定できます。

```js
module.exports = {
  pluginErrorMode: 'continue', // または 'fail-fast'
  extract: {
    pluginErrorMode: 'fail-fast' // extract 配下が優先
  }
};
```

- `continue`（デフォルト）: warning 出力で継続。
- `fail-fast`: load/hook エラー時に非ゼロ終了。
