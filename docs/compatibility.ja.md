# 互換性とフォーマット保証

## 対応 locale 形式

`i18next-turbo` は次の locale 出力形式を扱います。

- `json`
- `json5`
- `js` / `js-esm`
- `js-cjs`
- `ts`

次のコマンドは同じ locale file discovery / parsing ルールを共有します。

- `extract`
- `status`
- `check`
- `sync`
- `rename-key`

## Node API の保証

- `lib/index.js` は native addon が存在する場合はそれを優先します。
- native addon が無い場合は CLI wrapper にフォールバックします。
- `extract`, `lint`, `check` はどちらの経路でも同じ契約の構造化結果を返します。

## 互換性の範囲

このプロジェクトは i18next 風の抽出ワークフローを対象にしており、あらゆるサードパーティ抽出器の完全再現は目的にしていません。

保証するもの:

- Rust + SWC による TS/JS ソース解析
- i18next 風キー抽出と locale 同期
- 上記コマンド間で共有される対応 locale format handling

保証しないもの:

- すべての `i18next-parser` 設定エッジケースの完全ドロップイン互換
- 他抽出器との完全な出力一致
- JS/TS module locale 形式に対する `locize` 対応

## コマンド別の制限

- `locize` は `json` と `json5` のみ対応です
- `watch` は対応入力向けに最適化していますが、他ツールチェーン互換レイヤーとしては位置づけていません
- plugin hook はプロジェクト拡張ポイントであり、他 plugin API との互換保証ではありません
