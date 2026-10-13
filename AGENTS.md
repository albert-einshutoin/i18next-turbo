# Repository workflow

日本語で回答する。既存の未コミット・未追跡ファイルを保護する。

## GitHub Flow

長期ブランチは `main` のみ。最新の `main` から短期作業ブランチを作り、
`main` 向けPRで検証・レビュー後にマージする。マージ済み作業ブランチは削除する。
`develop` は使わない。未統合コミットは統合またはアーカイブ参照で保全してから整理する。
commit・push・merge・releaseはユーザーが依頼した工程だけで行う。

## Verification

変更箇所に対応するテストと静的レビューを先に行い、最後に必要な全体検証を行う。
下記は実在するコマンドであり、`ci` / `ci-pr` の別runnerは設けていない。

| 範囲 | コマンド |
|---|---|
| Rust全体 | `cargo test --all-targets --locked` |
| 全feature | `cargo test --all-targets --all-features --locked` |
| Clippy | `cargo clippy --all-targets --all-features --locked -- -D warnings` |
| Format / JS syntax | `cargo fmt -- --check` / `node --check bin/cli.js` |
| Release build | `cargo build --release --locked` |
| Node wrapper dependencies | `npm install --ignore-scripts --omit=optional --no-audit --no-fund --package-lock=false` |
| Native library | `cargo build --lib --all-features --locked` |

Native integration tests require the built library to be installed as a `.node`
file in the target directory. Mac/arm64 setup and pinned extraction comparison
are documented in [the evaluation](benchmarks/evaluation-20261013/README.md).

Hosted CI runs for pull requests and pushes to `main`. Its existing benchmark,
Node smoke, lint/tests and cross-platform build jobs provide separate hosted evidence.
テストには使い捨てのローカル入力を使う。実サービスへのLocize接続、公開、release実行は別工程とする。
