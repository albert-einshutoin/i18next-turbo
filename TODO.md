# i18next-turbo TODO リスト

このドキュメントは、i18next-turboの実装状況と今後のタスクを整理したものです。

## 📊 実装状況サマリー

- ✅ **完了**: Phase 2の一部、Phase 3のほぼ全て
- ⚠️ **部分的実装**: Phase 2の高度な機能
- ❌ **未実装**: Phase 1全体、Phase 2の一部、Phase 3以降の追加機能

---

## 🚀 Phase 1: 配布と基盤整備 (v0.5.0 目標)

### Task 1.1: napi-rs の導入とハイブリッド構成化

#### 1.1.1: Cargo.toml の更新
- [ ] `napi` クレートを追加（バージョン指定）
- [ ] `napi-derive` クレートを追加
- [ ] `[lib]` セクションを追加して `crate-type = ["cdylib", "rlib"]` を設定
- [ ] `[build-dependencies]` に `napi-build` を追加

#### 1.1.2: src/lib.rs の Node.js API 実装
- [ ] `#[napi]` マクロを使用した関数エクスポート
- [ ] `extract()` 関数を Node.js から呼び出し可能にする
- [ ] `watch()` 関数を Node.js から呼び出し可能にする
- [ ] 設定オブジェクトを Rust の `Config` 構造体に変換する関数
- [ ] エラーハンドリングを `napi::Error` に変換

#### 1.1.3: package.json の作成
- [ ] `package.json` ファイルを作成
- [ ] `name`, `version`, `description`, `license` を設定
- [ ] `bin` フィールドで CLI エントリーポイントを指定
- [ ] `main` フィールドで Node.js API エントリーポイントを指定
- [ ] `optionalDependencies` で OS 別バイナリを管理
  - `i18next-turbo-darwin-x64`, `i18next-turbo-darwin-arm64`
  - `i18next-turbo-win32-x64`, `i18next-turbo-win32-ia32`
  - `i18next-turbo-linux-x64`, `i18next-turbo-linux-arm64`
- [ ] `scripts` に `postinstall` スクリプトを追加（バイナリダウンロード用）

#### 1.1.4: Node.js ラッパーの作成
- [ ] `bin/cli.js` を作成（Rust バイナリを呼び出すラッパー）
- [ ] `lib/index.js` を作成（Node.js API のエントリーポイント）
- [ ] JS/TS 設定ファイルの読み込み処理を実装
  - `i18next-parser.config.js` の読み込み
  - `i18next.config.ts` の読み込み（`jiti` または `ts-node` を使用）
  - 設定オブジェクトを JSON 文字列に変換して Rust バイナリに渡す

#### 1.1.5: ビルドスクリプトの作成
- [ ] `build.rs` を作成（napi-build を使用）
- [ ] クロスコンパイル用の設定
- [ ] バイナリのパッケージングスクリプト

#### 達成基準
- [ ] ローカルで `npm install .` が成功する
- [ ] `node -e "require('./').extract(...)"` が動作する
- [ ] `npx i18next-turbo extract` が動作する

---

### Task 1.2: CI/CD (GitHub Actions) の構築

#### 1.2.1: GitHub Actions ワークフローの作成
- [ ] `.github/workflows/ci.yml` を作成
- [ ] マトリックス戦略で OS 別ビルドを設定
  - `windows-latest`
  - `macos-latest` (x64, arm64)
  - `ubuntu-latest` (x64, arm64)
- [ ] Rust ツールチェーンのセットアップ
- [ ] 各 OS で `cargo build --release` を実行
- [ ] ビルド成果物をアッカイブ

#### 1.2.2: リリースワークフローの作成
- [ ] `.github/workflows/release.yml` を作成
- [ ] タグプッシュ時にトリガー
- [ ] 全 OS 向けにビルド
- [ ] GitHub Releases にバイナリをアップロード
- [ ] npm への公開処理
  - `NPM_TOKEN` シークレットの設定
  - `npm publish` の実行

#### 1.2.3: npm パッケージの設定
- [ ] `package.json` に `files` フィールドを追加
- [ ] `.npmignore` を作成
- [ ] バージョン管理の自動化

#### 達成基準
- [ ] GitHub の Releases ページに各 OS 用のバイナリが並ぶ
- [ ] npm レジストリにパッケージが公開される
- [ ] `npm install i18next-turbo` でインストールできる

---

## ⚛️ Phase 2: i18next 完全互換 (v1.0.0 目標)

### Task 2.1: `<Trans>` コンポーネントの完全対応

#### 2.1.1: 子要素（Children）からのキー抽出
- [ ] `JSXElement` の子ノードを訪問する Visitor を実装
- [ ] `JSXText` ノードからテキストを抽出
- [ ] `i18nKey` がない場合、子要素のテキストをキーとして使用
- [ ] HTML タグ（`<strong>`, `<br>` など）を保持する処理
- [ ] 補間構文（`{{name}}`）の処理

#### 2.1.2: `ns` 属性の抽出
- [ ] `JSXOpeningElement` から `ns` 属性を抽出
- [ ] 名前空間を `ExtractedKey` に設定
- [ ] テストケースの追加

#### 2.1.3: `count` 属性の抽出
- [ ] `JSXOpeningElement` から `count` 属性を抽出
- [ ] 複数形キー（`_one`, `_other`）を生成
- [ ] `count` と `context` の組み合わせに対応

#### 2.1.4: `context` 属性の抽出
- [ ] `JSXOpeningElement` から `context` 属性を抽出
- [ ] コンテキスト付きキー（`key_context`）を生成
- [ ] 動的なコンテキスト値（三項演算子など）の解析

#### 達成基準
- [ ] `<Trans>Hello</Trans>` から `Hello` がキーとして抽出される
- [ ] `<Trans ns="common">content</Trans>` が `common` 名前空間に保存される
- [ ] `<Trans count={5}>item</Trans>` から `item_one`, `item_other` が生成される

---

### Task 2.2: 複数形 (Plurals) と Context の完全対応

#### 2.2.1: 言語別複数形カテゴリの生成
- [ ] Rust で `Intl.PluralRules` 相当の機能を実装
  - `icu_plurals` クレートまたは `intl_pluralrules` クレートを使用
  - または独自実装（CLDR データベースを使用）
- [ ] 設定された全言語の複数形カテゴリを取得
  - `zero`, `one`, `two`, `few`, `many`, `other`
- [ ] 各言語のカテゴリに基づいてキーを生成
- [ ] 単一カテゴリ（`other` のみ）の言語ではベースキーを使用

#### 2.2.2: Ordinal 複数形の対応
- [ ] `ordinal` タイプの複数形を検出
- [ ] `key_ordinal_one`, `key_ordinal_other` などのキーを生成
- [ ] 設定オプションで Ordinal を有効/無効化

#### 2.2.3: コンテキストと複数形の組み合わせ
- [ ] `context` と `count` の両方が存在する場合の処理
- [ ] `key_context_one`, `key_context_other` の生成
- [ ] ベース複数形キーの生成制御（`generateBasePluralForms` オプション）

#### 達成基準
- [ ] `t('apple', { count: 5 })` で言語に応じた複数形カテゴリが生成される
- [ ] 日本語（`other` のみ）では `apple` のみが生成される
- [ ] ロシア語では `apple_one`, `apple_few`, `apple_many`, `apple_other` が生成される

---

### Task 2.3: 高度な抽出パターンの実装

#### 2.3.1: `useTranslation` hook のスコープ解決
- [ ] `ScopeManager` 相当の機能を実装
- [ ] `useTranslation('ns', { keyPrefix: 'user' })` の解析
- [ ] 変数スコープの追跡
- [ ] `keyPrefix` の適用ロジック
- [ ] 配列分割代入: `const [t] = useTranslation()`
- [ ] オブジェクト分割代入: `const { t } = useTranslation()`
- [ ] エイリアス: `const { t: translate } = useTranslation()`

#### 2.3.2: `getFixedT` のサポート
- [ ] `i18next.getFixedT()` 呼び出しの検出
- [ ] 引数から namespace と keyPrefix を抽出
- [ ] スコープ情報を変数に紐付け
- [ ] `const t = getFixedT('en', 'ns', 'prefix')` の処理

#### 2.3.3: セレクター API のサポート
- [ ] `t($ => $.key.path)` パターンの検出
- [ ] アロー関数の引数からキーパスを抽出
- [ ] 型安全なキー選択のサポート

#### 2.3.4: 関数のエイリアス追跡
- [ ] `const translate = t` のようなエイリアスの検出
- [ ] エイリアスされた関数呼び出しの追跡
- [ ] スコープ情報の継承

#### 2.3.5: 動的コンテキスト値の解決
- [ ] 三項演算子の解析: `context: isMale ? 'male' : 'female'`
- [ ] 可能な値を列挙して複数のキーを生成
- [ ] 解決不可能な場合は警告を出力

#### 達成基準
- [ ] `const { t } = useTranslation('common', { keyPrefix: 'user' }); t('name')` が `common:user.name` として抽出される
- [ ] `const t = getFixedT('en', 'ns', 'prefix'); t('key')` が `ns:prefix.key` として抽出される
- [ ] `t($ => $.user.profile)` が `user.profile` として抽出される

---

### Task 2.4: 設定ファイルの JS/TS 対応 (Interop)

#### 2.4.1: Node.js ラッパーでの設定読み込み
- [ ] `bin/cli.js` で設定ファイルを検出
  - `i18next-turbo.json`
  - `i18next-parser.config.js`
  - `i18next.config.ts`
  - `i18next.config.js`
- [ ] `require()` または `jiti` で JS/TS ファイルを読み込み
- [ ] 設定オブジェクトを JSON 文字列に変換
- [ ] Rust バイナリに JSON 文字列を引数として渡す

#### 2.4.2: Rust 側での JSON パース
- [ ] JSON 文字列を受け取る CLI 引数を追加
- [ ] `serde_json` で JSON をパース
- [ ] 既存の `Config` 構造体に変換

#### 2.4.3: 設定の互換性
- [ ] `i18next-parser.config.js` の形式に対応
- [ ] プロパティ名のマッピング（`$LOCALE` → `{{language}}` など）
- [ ] デフォルト値の設定

#### 達成基準
- [ ] ユーザーが既存の JS 設定ファイルをそのまま使える
- [ ] TypeScript 設定ファイルも読み込める
- [ ] 設定の検証とエラーメッセージ

---

## 🚀 Phase 3: 圧倒的差別化 (v2.0.0 目標)

### Task 3.1: 追加コマンドの実装

#### 3.1.1: `status` コマンド
- [ ] 翻訳完了率の計算
- [ ] ロケール別のサマリー表示
- [ ] 詳細なキー別レポート（`status [locale]`）
- [ ] 名前空間フィルタ（`--namespace` オプション）
- [ ] プログレスバーの表示
- [ ] 非ゼロ終了コード（翻訳が不完全な場合）

#### 3.1.2: `sync` コマンド
- [ ] プライマリ言語ファイルの読み込み
- [ ] セカンダリ言語ファイルとの比較
- [ ] 不足キーの追加（デフォルト値で）
- [ ] 未使用キーの削除（オプション）
- [ ] 変更されたファイルの報告

#### 3.1.3: `lint` コマンド
- [ ] ハードコードされた文字列の検出
- [ ] JSX テキストノードの解析
- [ ] JSX 属性の解析（`title`, `alt` など）
- [ ] 無視ルールの設定（`ignoredTags`, `ignoredAttributes`）
- [ ] エラーレポートの表示
- [ ] Watch モードのサポート

#### 3.1.4: `rename-key` コマンド
- [ ] ソースファイル内のキーを検索
- [ ] AST ベースでのキー置換
- [ ] 翻訳ファイル内のキーをリネーム
- [ ] コンフリクトの検出
- [ ] Dry-run モード
- [ ] 変更内容のレポート

#### 3.1.5: `init` コマンド
- [ ] 対話的な設定ウィザード
- [ ] プロジェクト構造の自動検出
- [ ] 設定ファイルの生成（TS/JS 選択可能）
- [ ] デフォルト値の提案

#### 3.1.6: `migrate-config` コマンド
- [ ] レガシー設定ファイルの検出
- [ ] 設定の変換ロジック
- [ ] 新しい形式への移行
- [ ] 警告メッセージの表示

---

### Task 3.2: 高度な設定オプション

#### 3.2.1: Extract コマンドのオプション
- [ ] `--sync-primary`: プライマリ言語のデフォルト値同期
- [ ] `--sync-all`: 全ロケールの同期
- [ ] `--dry-run`: ファイル変更なしのプレビュー
- [ ] `--ci`: CI モード（ファイル更新時に非ゼロ終了）

#### 3.2.2: 設定ファイルの拡張
- [ ] `preservePatterns`: 動的キーのパターン保持
- [ ] `preserveContextVariants`: コンテキスト変種の保持
- [ ] `generateBasePluralForms`: ベース複数形の生成制御
- [ ] `disablePlurals`: 複数形の完全無効化
- [ ] `extractFromComments`: コメントからの抽出

---

### Task 3.3: 出力フォーマットの多様化

#### 3.3.1: JSON5 サポート
- [ ] JSON5 パーサーの統合（`serde_json5` または類似）
- [ ] コメントの保持
- [ ] 末尾カンマの保持
- [ ] 数値形式の保持

#### 3.3.2: TypeScript ファイル出力
- [ ] `outputFormat: 'ts'` オプション
- [ ] `export default { ... } as const` 形式の生成
- [ ] 型安全性の確保

#### 3.3.3: 名前空間のマージ
- [ ] `mergeNamespaces: true` オプション
- [ ] 全名前空間を1ファイルに統合
- [ ] 出力パスの調整（`{{namespace}}` プレースホルダーなし）

---

### Task 3.4: コメントからの抽出

#### 3.4.1: コメントパターンの検出
- [ ] `// t('key', 'default')` パターンの検出
- [ ] `/* t('key') */` パターンの検出
- [ ] オブジェクト構文の解析: `// t('key', { defaultValue: '...', ns: '...' })`

#### 3.4.2: スコープ解決
- [ ] コメント内の `useTranslation` 参照の解決
- [ ] `keyPrefix` の適用

#### 3.4.3: 設定オプション
- [ ] `extractFromComments: true/false` オプション
- [ ] デフォルトで有効化

---

### Task 3.5: Locize 統合（オプション）

#### 3.5.1: Locize CLI の統合
- [ ] `locize-cli` の依存関係チェック
- [ ] `locize-sync` コマンドの実装
- [ ] `locize-download` コマンドの実装
- [ ] `locize-migrate` コマンドの実装

#### 3.5.2: 認証情報の管理
- [ ] インタラクティブな認証情報設定
- [ ] 環境変数からの読み込み
- [ ] 設定ファイルへの保存

---

## 🧪 テストと品質保証

### Task 4.1: テストカバレッジの向上
- [ ] 各抽出パターンのユニットテスト
- [ ] 統合テストの追加
- [ ] エッジケースのテスト
- [ ] パフォーマンステスト

### Task 4.2: ドキュメント
- [ ] API ドキュメントの整備
- [ ] 使用例の追加
- [ ] マイグレーションガイド
- [ ] トラブルシューティングガイド

---

## 📝 メモ

### 実装済み機能
- ✅ 基本的な `t()` 関数の抽出
- ✅ `i18n.t()` 形式の抽出
- ✅ `<Trans>` コンポーネントの基本対応（`i18nKey`, `defaults`）
- ✅ 名前空間サポート
- ✅ 基本的な複数形サポート（`_one`, `_other`）
- ✅ コンテキストサポート（基本的な文字列リテラル）
- ✅ マジックコメント（`i18next-extract-disable`）
- ✅ JSON 同期（既存翻訳の保持）
- ✅ Watch モード
- ✅ TypeScript 型定義生成
- ✅ 未使用キーの検知と削除

### 技術的負債
- [ ] エラーハンドリングの改善
- [ ] ログレベルの設定
- [ ] パフォーマンス最適化
- [ ] メモリ使用量の最適化

---

## 🎯 優先度マトリックス

### P0 (最優先 - 即座に実装)
1. Task 1.1: napi-rs の導入
2. Task 1.2: CI/CD の構築
3. Task 2.4: JS/TS 設定ファイルの読み込み

### P1 (高優先度 - Phase 2 完了のため)
4. Task 2.1: `<Trans>` コンポーネントの完全対応
5. Task 2.2: 言語別複数形カテゴリの生成
6. Task 2.3.1: `useTranslation` hook のスコープ解決
7. Task 2.3.2: `getFixedT` のサポート

### P2 (中優先度 - 差別化機能)
8. Task 3.1.1: `status` コマンド
9. Task 3.1.2: `sync` コマンド
10. Task 3.1.3: `lint` コマンド
11. Task 3.2: 高度な設定オプション

### P3 (低優先度 - 拡張機能)
12. Task 3.1.4: `rename-key` コマンド
13. Task 3.1.5: `init` コマンド
14. Task 3.1.6: `migrate-config` コマンド
15. Task 3.3: 出力フォーマットの多様化
16. Task 3.4: コメントからの抽出
17. Task 3.5: Locize 統合

---

## 📅 マイルストーン

### v0.5.0 (Phase 1 完了)
- [ ] npm パッケージとして配布可能
- [ ] CI/CD が動作
- [ ] 基本的な Node.js API

### v1.0.0 (Phase 2 完了)
- [ ] i18next 完全互換
- [ ] 既存ツールからの移行が容易
- [ ] 抽出漏れゼロ

### v2.0.0 (Phase 3 完了)
- [ ] 差別化機能の実装
- [ ] 開発者体験の向上
- [ ] エコシステム統合

---

最終更新: 2025-01-XX

