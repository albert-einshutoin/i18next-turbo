# パーサ移行計画（Regex -> Scanner/AST）

Issue: `#57`

## 目的

抽出ロジックを Regex 依存から Scanner/AST 中心へ段階的に移行し、正確性と保守性を高める。

## 現状

- 対応完了:
  - コメント抽出パスの Regex 依存を撤廃
    - `t('key')`
    - `t('key', 'default')`
    - `t('key', { ...options })`
  - `defaultValue/ns/context/count/ordinal` のオプション解析をスキャナ化
  - `.vue/.svelte` の script/template/style ブロック分割もスキャナ化
  - extractor パスにおける `regex` クレート依存を `Cargo.toml` から削除

## 結果

- unit/integration テストで挙動互換を確認済み。
- 性能回帰チェックとして `npm run benchmark` を継続運用する。
