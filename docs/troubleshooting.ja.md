# トラブルシューティング

## キーが抽出されない

- `input` グロブが実ファイルに一致しているか確認。
- `ignore` が広すぎないか確認。
- `i18next-turbo status` で検出対象を確認。

## デッドキーが過剰に出る

- 名前空間利用が一貫しているか確認（`ns:key` と default namespace）。
- マージ出力の場合は `mergeNamespaces` を有効化。
- 必要に応じて `preservePatterns` を追加。

## Node ラッパーが設定を読めない

- TS設定には `jiti` が必要。
- `--config i18next.config.ts` で明示指定。

## プラグインフックでエラー

- フックエラーは warning として出力。
- `setup` / `onLoad` から段階的に導入。

## JSON5 の見た目が変わる

- コメントと末尾カンマは保持。
- 数値リテラル形式は同値の範囲で保持（`1e3`, `0x10` など）。
