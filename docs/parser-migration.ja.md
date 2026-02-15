# パーサ移行計画（Regex -> Scanner/AST）

Issue: `#57`

## 目的

抽出ロジックを Regex 依存から Scanner/AST 中心へ段階的に移行し、正確性と保守性を高める。

## 現状

- 今回対応済み:
  - コメント抽出パスの Regex 依存を撤廃
    - `t('key')`
    - `t('key', 'default')`
    - `t('key', { ...options })`
  - `defaultValue/ns/context/count/ordinal` のオプション解析をスキャナ化
- まだ Regex を使う箇所:
  - `.vue/.svelte` の script/template/style ブロック分割
  - 一部の補助処理

## 移行方針

1. 先に抽出の主要ホットパス（コメント抽出）を置換。
2. 既存の unit/integration テストで挙動互換を維持。
3. 残りを段階置換しつつ `npm run benchmark` で性能回帰を監視。

## 補足

- 低リスクで前進するための段階移行であり、主要抽出パスから順に Regex 依存を縮小する。
