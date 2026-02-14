# i18next-turbo

モダンな TypeScript/JavaScript コードベース向けの高速 i18next キー抽出ツールです。

`i18next-turbo` は Rust + SWC ベースの抽出器で、i18next ワークフローとの互換性を重視して設計されています。CI の高速化と、開発時の低遅延な watch 実行を目的としています。

## インストール

```bash
npm install --save-dev i18next-turbo
```

パッケージ本体は `optionalDependencies` を使って OS/アーキテクチャ別バイナリを解決します。
詳細（対応表、フォールバック、トラブル対応）は [docs/installation.ja.md](docs/installation.ja.md) を参照してください。

## クイックスタート

1. 設定ファイルを初期化:

```bash
i18next-turbo init
```

2. 1 回だけ抽出:

```bash
i18next-turbo extract
```

3. 開発中は watch 実行:

```bash
i18next-turbo watch
```

## 最小設定

プロジェクトルートに `i18next-turbo.json` を作成:

```json
{
  "input": ["src/**/*.{ts,tsx,js,jsx}"],
  "output": "locales/$LOCALE/$NAMESPACE.json",
  "locales": ["en", "ja"],
  "defaultNamespace": "translation",
  "functions": ["t", "i18n.t"]
}
```

## CLI コマンド

- `extract`: キー抽出とロケール同期
- `watch`: 変更監視しながら継続同期
- `sync`: 抽出結果から同期のみ実行
- `check`: 未使用キー検出/削除
- `lint`: ハードコード文字列検出
- `status`: 翻訳進捗表示
- `typegen`: TypeScript 型生成
- `rename-key`: 翻訳キーの安全なリネーム
- `migrate-config`: i18next-parser 形式設定の移行

## ドキュメント

- [インストールとバイナリ解決](docs/installation.ja.md)
- [API リファレンス](docs/api.ja.md)
- [移行ガイド](docs/migration-guide.ja.md)
- [トラブルシューティング](docs/troubleshooting.ja.md)
- [使用例](docs/usage-examples.ja.md)
- [パフォーマンステスト](docs/performance-testing.ja.md)

## コントリビュート

- [Contributing Guide](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security Policy](SECURITY.md)
