# プラグインランタイム設計（Issue #59）

## 目的

- Rustバイナリ配布時に JS プラグインがどう動くかを明確化する。
- プラグイン失敗時の安全な挙動を定義する。
- プラグイン有無の性能差を定量化する。

## 実行モデル

- 抽出/同期のコア処理は Rust バイナリで実行。
- 動的 JS プラグインは Node ラッパー（`bin/cli.js`）で実行。
- 方式は「単一 Node プロセス + 子 Rust プロセス」。
  - Node が `onLoad` 前処理と `onVisitNode`/`afterSync` 後処理を担当。
  - Rust はプラグイン非依存で、JSON設定を stdin から受け取る。

Rust コアの決定性を保つため、Rust への JS ランタイム埋め込みは行わない。

## watchの対応範囲

- Nodeラッパーの`watch`はpluginなし（`plugins`未指定または空配列）で使用する。
- 解決された`plugins`（トップレベルまたは`extract.plugins`）が空でない場合、
  `watch does not support plugins`と診断し、終了コード1で拒否する。
  pluginモジュールの読込、factory・hook、Rust起動より前に拒否し、翻訳ファイルを変更しない。
  `pluginErrorMode: continue`でも、この組み合わせは継続しない。
- JS/TS設定、明示指定のJSON設定、自動検出した設定に同じ制限を適用する。
  inline設定（`--config-json JSON`・`--config-json=JSON`・`--config-stdin`）も、
  設定ファイルの有無によらず同じ検証を通す。優先順はstdin、inline JSON、ファイル設定。
  watchの設定stdinは一度だけ読み、検証済み設定をRustへ渡す。他のコマンドはstdinを
  そのまま引き継ぎ、help/versionでは消費しない。
  設定自体の評価はplugin判定に必要であり、設定内の任意のJS実行を防ぐものではない。
- `-cFILE`・`-c=FILE`・`-vcFILE`も同じ設定解決を通す。
  空または非オブジェクトの明示設定は処理前に終了コード1で拒否し、既定設定へフォールバックしない。
- watchの全明示設定をNode側の共通入口で解決する。JS/TSは既存loaderを使い、
  それ以外は`.conf`・拡張子なしを含め、拡張子によらずJSONとして読み込む。
  読込・JSON解析・オブジェクト検証に失敗した場合は、Rustや既定設定へフォールバックせず拒否する。
- pluginが必要な場合は`extract`を実行する。単発抽出の`onLoad`・AST通知・終了hookは維持する。
- pluginなしwatchの増分抽出と既存翻訳の保持、未解決値がある場合のprune拒否は維持する。
  純RustバイナリはJS pluginを評価しないため、plugin付きwatchの代替にはならない。

## フック境界

- `setup(context)`:
  前処理・Rust実行前。
- `onLoad({ filePath, relativePath, source, ... })`:
  Rust解析前のソース前処理。
- `onVisitNode(node)`:
  Rust ASTイベント中継と翻訳ツリー走査イベントを受け取る。
- `onEnd(context)`:
  Rust実行後・ランタイム変換後。
- `afterSync(context)`:
  同期後の最終フック。

## 失敗ポリシー

`pluginErrorMode` でプラグイン load/hook 失敗時の挙動を制御する。

- `continue`（デフォルト）:
  warning を出して処理継続。
- `fail-fast`:
  即時エラー終了（非ゼロ終了コード）。

設定場所:

- トップレベル: `pluginErrorMode`
- extract配下: `extract.pluginErrorMode`

両方ある場合は `extract.pluginErrorMode` を優先。

## Node なし環境での互換性

- JS/TS 設定ロードとプラグイン実行には Node ラッパーが必要。
- 純 Rust バイナリ実行では JS プラグインは評価されない。
- 推奨運用:
  - プラグインが必要な場合は Node ラッパー経由で実行。
  - 最小・決定的な運用が必要な場合は JSON 設定 + Rust のみを使用。

## ベンチマーク

- 基本比較:
  `npm run benchmark`（turbo vs i18next-cli）
- プラグインオーバーヘッド:
  `npm run benchmark:plugin`（同一fixtureで plugin off/on 比較）

計測指標:

- 平均/最小/最大プロセス時間
- オーバーヘッド比率 `((withPlugin.avg - withoutPlugin.avg) / withoutPlugin.avg)`

しきい値判定:

- `node scripts/run-plugin-benchmark.mjs 5 --max-overhead 0.20`

## セキュリティ境界

- プラグインは Node プロセス権限で実行される。
- 追加サンドボックスはデフォルトで提供しない。
- サードパーティプラグインは信頼コードとして扱うこと。
