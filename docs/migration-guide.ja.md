# マイグレーションガイド

## i18next-parser / i18next-cli から

1. 既存の入力グロブとロケールディレクトリを維持。
2. `i18next-turbo.json` を作成、または JS/TS 設定を Node ラッパーで利用。
3. 主要オプションを対応付け。
   - `defaultNS` -> `defaultNamespace`
   - `keySeparator` / `nsSeparator` の `false` は空文字に変換
   - `mergeNamespaces` は対応済み
4. 事前確認。
   - `i18next-turbo status`
   - `i18next-turbo check --dry-run`
5. 抽出実行。
   - `i18next-turbo extract`

## 既存の単一マージファイルを使う場合

`all.json` など既存ファイル名を維持するには:

```json
{
  "mergeNamespaces": true,
  "mergedNamespaceFilename": "all"
}
```

未指定時は、既存の単一ファイル構造を優先的に自動検出します。
