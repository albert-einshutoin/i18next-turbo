# i18next-turbo へのコントリビュート

i18next-turbo への貢献に興味を持っていただきありがとうございます。このドキュメントでは、コントリビュートのガイドラインと手順を説明します。

## 行動規範

本プロジェクトは行動規範を採用しています。参加前に [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) を必ずお読みください。

## コントリビュート方法

### バグの報告

バグを発見した場合は、以下を含む Issue を立ててください：

- 明確で説明的なタイトル
- 再現手順
- 期待される動作
- 実際の動作
- 環境（OS、Rust バージョンなど）
- 関連するエラーメッセージやログ

### 機能の提案

機能提案を歓迎します。以下を含む Issue を立ててください：

- 機能の明確な説明
- ユースケースと例
- この機能が有用な理由

### プルリクエスト

1. **リポジトリをフォーク**し、`main` から feature ブランチを作成
2. **変更**を下記のコーディング規約に従って行う
3. **テスト**を新機能やバグ修正に追加
4. 必要に応じて**ドキュメント**を更新
5. **テストを実行**して通過することを確認
6. **プルリクエスト**を明確な説明とともに提出

## 開発環境

### 必要環境

- Rust 1.70 以降
- Cargo（Rust に同梱）

### ビルド

```bash
git clone https://github.com/your-username/i18next-turbo.git
cd i18next-turbo

cargo build
cargo test
cargo run -- extract
```

## コーディング規約

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) に従う
- `cargo fmt` でフォーマット、`cargo clippy` でチェック
- 意味のあるコミットメッセージを書く
- 関数は小さく単一責任に、複雑なロジックにはコメントを追加

## コミットメッセージ

[Conventional Commits](https://www.conventionalcommits.org/) 形式に従ってください。

## レビュー

- すべての PR には少なくとも 1 人のメンテナのレビューが必要です
- CI（テスト、リント、フォーマット）が通ること
- 承認後、メンテナがマージします

質問は Issue でどうぞ。i18next-turbo へのコントリビュートありがとうございます。
