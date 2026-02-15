# MVP計画（Issue優先順位ベース）

現在のOpen Issueに付与した `priority:*` ラベルを基準に、実装順序を定義します。

## 優先度ポリシー

- `priority:P0`: リリース品質のブロッカー、またはアーキテクチャ基盤。
- `priority:P1`: 基盤安定後に着手する高価値改善。
- `priority:P2`: バックログ、重複、または統合候補。

## 現在のOpen Issue（優先度順）

### P0（最優先）

1. [#58](https://github.com/albert-einshutoin/i18next-turbo/issues/58) ネットワーク依存（Locize等）のコア分離。
2. [#60](https://github.com/albert-einshutoin/i18next-turbo/issues/60) `reqwest`/Locize依存のfeature化（ミニマルコアを既定化）。
3. [#35](https://github.com/albert-einshutoin/i18next-turbo/issues/35) ライブラリ経路の `anyhow` を厳格Error Enumへ移行。
4. [#61](https://github.com/albert-einshutoin/i18next-turbo/issues/61) 破壊系テスト拡充（壊れたUnicode・巨大入力・権限/資源障害）。

### P1（次優先）

1. [#57](https://github.com/albert-einshutoin/i18next-turbo/issues/57) Regex依存抽出からスキャナ/AST中心へ移行。
2. [#59](https://github.com/albert-einshutoin/i18next-turbo/issues/59) プラグイン実行モデルとオーバーヘッドの定義。
3. [#42](https://github.com/albert-einshutoin/i18next-turbo/issues/42) SWC依存見直しによるバイナリサイズ最適化。
4. [#40](https://github.com/albert-einshutoin/i18next-turbo/issues/40) JSON整形スタイル維持によるDiffノイズ削減。

### P2（バックログ/統合候補）

1. [#37](https://github.com/albert-einshutoin/i18next-turbo/issues/37) ビルドサイズ/コンパイル時間削減（#42/#60と重複領域）。
2. [#31](https://github.com/albert-einshutoin/i18next-turbo/issues/31) ビルドサイズ/コンパイル時間削減（旧系統の重複）。

## 実行ウェーブ

### Wave 1: アーキテクチャ基盤
- #58 と #60 をセットで実施。
- 成果: core-onlyビルドとオプション連携の境界確立。

### Wave 2: 信頼性とエラー契約
- #35 と #61 を実施。
- 成果: 型付きエラーと異常系耐性の強化。

### Wave 3: 抽出エンジンと拡張モデル
- #57 と #59 を実施。
- 成果: 抽出コアの決定性向上とプラグイン境界の明文化。

### Wave 4: サイズ/UXの仕上げ
- #42 と #40 を実施。
- 成果: 配布サイズ縮小とレビューしやすい低ノイズDiff。

## 補足

- 優先度はGitHub Issueラベル（`priority:P0/P1/P2`）として管理。
- 優先度の再調整は、各Wave完了時または重大インシデント時に実施。
