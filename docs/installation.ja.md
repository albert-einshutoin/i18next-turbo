# インストールとバイナリ解決

## 推奨インストール

```bash
npm install --save-dev i18next-turbo
```

`i18next-turbo` は `optionalDependencies` を使って OS 別パッケージを解決します。
`bin/cli.js` は次の順で実行バイナリを探索します。

1. `CARGO_BIN_EXE_i18next_turbo`（Cargo テスト/開発環境）
2. `I18NEXT_TURBO_BINARY`（手動上書き）
3. ローカルビルド（`target/debug`, `target/release`）
4. インストール済み OS 別パッケージ

## OS 別パッケージ

- `i18next-turbo-darwin-arm64`
- `i18next-turbo-darwin-x64`
- `i18next-turbo-linux-x64-gnu`
- `i18next-turbo-linux-x64-musl`
- `i18next-turbo-win32-x64-msvc`

Linux x64 は実行環境を判定して `-musl` または `-gnu` を選択します。

## 開発時フォールバック

OS 別パッケージが利用できない場合でも、ローカルビルドで実行できます。

```bash
cargo build --release
```

その後に:

```bash
i18next-turbo extract
```

## CI 運用の推奨

- npm 依存キャッシュと Rust ターゲットキャッシュを分離する。
- `package.json` と `package-lock.json` の整合性を保つ。
- 必要に応じて `--config` で設定ファイルパスを明示して実行する。

## 既知のレジストリ制約（2026-02-14 時点）

執筆時点では `i18next-turbo-win32-x64-msvc` の publish が npm のスパム判定（`403`）で失敗する可能性があります。

この場合の対応:

1. [npm support](https://www.npmjs.com/support) に問い合わせる。
2. `i18next-turbo-win32-x64-msvc` のパッケージ名審査解除を依頼する。
3. 承認後に release publish を再実行する。

それまで Windows ではソースビルド（`cargo build --release`）での利用を推奨します。
