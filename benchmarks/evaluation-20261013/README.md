# 抽出互換性・速度・全体検証（2026-10-13）

関数別名、`getFixedT`、有限動的キー、locale別の複数形、デフォルト値を修正し、i18next-cli **1.75.2**（`540b3beac7737ce066af47875be7f1dd360bd12c`）と比較した。

測定対象のコードは `3d61037`。結果は[results.json](results.json)、検証コマンドとsource/binary hashは[validation.json](validation.json)。この資料を追加するコミットは抽出器を変更しない。

## 抽出結果

比較単位は言語・namespace・leaf key・値。JSONの空白・順序は除外する。Rust直接とNodeラッパーの両経路で一致し、既存seed翻訳も保持した。

| fixture | 対象ファイル（別途ignore 1件） | 比較組数 | 欠落 / 余分 / 値違い |
|---|---:|---:|---:|
| feature-probes | 8 | 106 | 0 / 0 / 0 |
| static-300 | 300 | 7,956 | 0 / 0 / 0 |
| mixed-308 | 308 | 8,023 | 0 / 0 / 0 |
| static-1200 | 1,200 | 31,716 | 0 / 0 / 0 |

修正前のfeature-probesは28 / 12 / 7、mixed-308は31 / 12 / 7だった。修正前後でsource・両設定・seedのbytesが同一であることを確認した。機能probeの固定CLI出力は `tests/fixtures/cli-parity/expected.json` にも保存し、integration testで照合する。

## 速度

Apple M4 / arm64 / macOS / Node v24.2.0 / Satechi、warm OS filesystem cache。en/ja/ru、6 namespace、18 JSON。300/1200ファイルは80% TSXコンポーネントと20% TSユーティリティ、混在fixtureには機能probeを追加した。

各ツールwarmup 1回後、条件ごとに7回計測し順序をローテーション。起動・解析・同期・終了を含み、依存取得・build・npxは除く。初回同期はseedへ復元した出力に追加し、再抽出は同期済み出力を使う。cold-cache計測ではない。Nodeラッパーには同じrelease binaryを `I18NEXT_TURBO_BINARY` で指定する。

中央値ms（全samples、mean、min/maxはresults.json）：

| 条件 | CLI | Rust直接 | Nodeラッパー | 対CLI倍率（ラッパー） |
|---|---:|---:|---:|---:|
| 300・初回同期 | 524.95 | 37.32 | 66.88 | 7.85x |
| 300・再抽出 | 452.06 | 29.50 | 59.19 | 7.64x |
| 混在308・初回同期 | 514.39 | 36.41 | 65.73 | 7.83x |
| 混在308・再抽出 | 467.75 | 29.90 | 60.66 | 7.71x |
| 1200・初回同期 | 1310.20 | 111.32 | 138.63 | 9.45x |
| 1200・再抽出 | 1131.56 | 93.11 | 121.96 | 9.28x |

出力比較は各測定群末尾に行う。保存したtiming fixtureのcomparisonは最後の再抽出群であり、全サンプル個別のsnapshotではない。静的fixtureは独立した期待値とも照合した。

watchは1200ファイルで既存sourceへキーを追加し、3言語すべてに反映するまでを5回測定した。ready反映後1秒待機、300ms debounceと5ms pollを含む。CLI中央値 **856.78ms**、Rust直接 **321.59ms**（2.66x）。追加キーを含む結果は一致し、sourceは復元した。ラッパー経由watchは未計測。

## 全体検証

同じsource bytesに対するローカル検証。結果は後続のcommit整理時にhashを再照合した。

| コマンド | 結果 |
|---|---|
| `cargo test --all-targets --locked` | 191成功、0失敗、0ignored |
| `cargo test --all-targets --all-features --locked` | 207成功、0失敗、0ignored |
| `cargo test --features napi --test node_wrapper_integration --locked` | 11成功（上記のsubset） |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | 成功、`-A`なし |
| `cargo fmt -- --check` / `node --check bin/cli.js` / `git diff --check` | 成功 |
| `cargo build --release --locked` / `cargo build --lib --all-features --locked` | 成功 |

Node依存を導入し、実際にbuildしたnative addonをロードして検証した。snake_case result/options、nullable message、lint設定、strict拒否、check dry-runの非書込み、CRLF保持を確認。元のNode失敗3件とClippy指摘1件は解消し、全feature検証で見つかったlocize import/旧test引数も修正した。

Rust/N-API、JS formatter、測定scriptの専門レビューとCompanion Stop Review Gateは静的ALLOW。静的レビューはruntime/hosted CIの代替ではない。

## 再実行

測定scriptはこのMac条件用。Python **3.11以上**（測定時3.14.6）とNode、Rustを用意し、隣接した `i18next-cli` cloneを上記commitに固定してnpm依存導入と `npm run build` を済ませる。fixtureのアプリコードやLocize等の実サービスは実行しない。

```sh
# i18next-turboのルート
npm install --ignore-scripts --omit=optional --no-audit --no-fund --package-lock=false
cargo build --release --locked
python3 benchmarks/evaluation-20261013/run.py --phase all --runs 7
```

生成fixture・ログ・完全reportは専用の `target/evaluation-20261013/` 配下のみを更新する。runごとにreportを作り直し、選択phase・in_progress/failed/completedを記録する。`completed`は選択phaseの測定完了を意味し、互換性の判定は各caseのparityを参照する。保存失敗時も元の測定例外を保持し、古い完了結果へfallbackしない。

Nodeを含む全体検証は、このMac/arm64では次の準備後に上のtest/Clippyコマンドを使う：

```sh
cargo build --lib --all-features --locked
cp target/debug/libi18next_turbo.dylib target/debug/i18next-turbo.darwin-arm64.node
```

生成fixtureでの結果であり、実案件の全パターン、全面CLI互換、別OS/CPU、公開npm配布、liveサービスの動作保証ではない。大規模・頻繁な抽出/watchに絞った開発継続には実測上の価値がある。
