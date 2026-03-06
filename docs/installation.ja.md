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

`lib/index.js` が利用する native Node addon をビルドする場合:

```bash
cargo build --release --features napi
```

## CI 運用の推奨

- npm 依存キャッシュと Rust ターゲットキャッシュを分離する。
- `package.json` と `package-lock.json` の整合性を保つ。
- 必要に応じて `--config` で設定ファイルパスを明示して実行する。
- `cargo test` に加えて `cargo test --features napi` も検証する。
