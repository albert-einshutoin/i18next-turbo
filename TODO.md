# i18next-turbo TODO リスト

このドキュメントは、i18next-turboの実装状況と今後のタスクを整理したものです。

## 📊 実装状況サマリー

- ✅ **完了**: Phase 2の一部、Phase 3のほぼ全て
- ⚠️ **部分的実装**: Phase 2の高度な機能
- ✅ **接続完了**: TypeScript型生成、デッドキー検知（CLIから呼び出し可能）
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

### Task 1.3: 実装済み機能のCLI接続（Wiring）【完了】✅

#### 1.3.1: TypeScript型生成コマンドの追加
- [x] `src/main.rs` の `Commands` Enum に `Typegen` バリアントを追加
- [x] `typegen` サブコマンドを実装
  - `--output` オプション（型定義ファイルの出力先）
  - `--default-locale` オプション（デフォルトロケール）
- [x] `src/typegen.rs` の `generate_types()` 関数を呼び出す処理を追加
- [ ] 設定ファイルから `types` セクションを読み込む
- [x] `extract` コマンド実行時に自動的に型生成するオプション（`--generate-types`）を追加

#### 1.3.2: デッドキー検知コマンドの追加
- [x] `src/main.rs` の `Commands` Enum に `Check` または `Cleanup` バリアントを追加
- [x] `check` または `cleanup` サブコマンドを実装
  - `--remove` オプション（未使用キーを削除するかどうか）
  - `--dry-run` オプション（削除前にプレビュー）
- [x] `src/cleanup.rs` の `find_dead_keys()` と `remove_dead_keys()` 関数を呼び出す処理を追加
- [x] 検出されたデッドキーのレポート表示
- [ ] 削除実行時の確認プロンプト（`--remove` が指定されている場合）

#### 達成基準
- [x] `i18next-turbo typegen` コマンドが動作する
- [x] `i18next-turbo check` コマンドが動作する
- [x] `i18next-turbo extract --generate-types` で抽出と型生成が同時に実行される
- [ ] READMEに記載されている機能が実際に使える状態になる

---

## ⚛️ Phase 2: i18next 完全互換 (v1.0.0 目標)

### Task 2.1: `<Trans>` コンポーネントの完全対応 ✅（主要機能完了）

#### 2.1.1: 子要素（Children）からのキー抽出 ✅
- [x] `JSXElement` の子ノードを訪問する Visitor を実装
- [x] `JSXText` ノードからテキストを抽出
- [x] `i18nKey` がない場合、子要素のテキストをキーとして使用
- [x] HTML タグ（`<strong>`, `<br>` など）を保持する処理
- [x] 補間構文（`{{name}}`）の処理

#### 2.1.2: `ns` 属性の抽出 ✅
- [x] `JSXOpeningElement` から `ns` 属性を抽出
- [x] 名前空間を `ExtractedKey` に設定
- [x] テストケースの追加

#### 2.1.3: `count` 属性の抽出 ✅
- [x] `JSXOpeningElement` から `count` 属性を抽出
- [x] 複数形キー（`_one`, `_other`）を生成
- [ ] `count` と `context` の組み合わせに対応

#### 2.1.4: `context` 属性の抽出
- [ ] `JSXOpeningElement` から `context` 属性を抽出
- [ ] コンテキスト付きキー（`key_context`）を生成
- [ ] 動的なコンテキスト値（三項演算子など）の解析

#### 達成基準
- [x] `<Trans>Hello</Trans>` から `Hello` がキーとして抽出される
- [x] `<Trans ns="common">content</Trans>` が `common` 名前空間に保存される
- [x] `<Trans count={5}>item</Trans>` から `item_one`, `item_other` が生成される

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

#### 2.3.6: ネストされた翻訳（Nested Translations）のサポート
- [ ] `$t(key)` パターンの検出（文字列内のネストされた翻訳）
- [ ] `nestingPrefix` と `nestingSuffix` の設定サポート（デフォルト: `$t(` と `)`）
- [ ] `nestingOptionsSeparator` の設定サポート（デフォルト: `,`）
- [ ] 文字列内の `$t(key, { options })` パターンの解析
- [ ] ネストされたキーから複数形やコンテキストを抽出
- [ ] デフォルト値内のネストされた翻訳の抽出

#### 2.3.7: returnObjects のサポート
- [ ] `t('key', { returnObjects: true })` の検出
- [ ] 構造化コンテンツ（オブジェクト）の保持
- [ ] `objectKeys` セットの管理
- [ ] オブジェクトキーの子要素を自動的に保持するパターン生成（`key.*`）

#### 2.3.8: テンプレートリテラル（Template Literals）のサポート ✅
- [x] `t(\`key\`)` パターンの検出（バッククォートで囲まれた文字列）
- [x] `Expr::Tpl` (Template Literal) ノードの処理を追加
- [x] 変数が埋め込まれていないテンプレートリテラル（静的文字列）の抽出
  - `t(\`hello\`)` → `hello` として抽出
- [x] 変数が埋め込まれているテンプレートリテラルの警告またはスキップ
  - `t(\`hello_${name}\`)` → スキップ（動的キーは抽出不可）
- [x] `Lit::Str` と `TemplateLiteral` の両方をサポートする統一的な処理
- [x] テストケースの追加

#### 達成基準
- [ ] `const { t } = useTranslation('common', { keyPrefix: 'user' }); t('name')` が `common:user.name` として抽出される
- [ ] `const t = getFixedT('en', 'ns', 'prefix'); t('key')` が `ns:prefix.key` として抽出される
- [ ] `t($ => $.user.profile)` が `user.profile` として抽出される
- [ ] `t('You have $t(item_count, {"count": {{count}} })')` から `item_count_one`, `item_count_other` が抽出される
- [ ] `t('countries', { returnObjects: true })` で既存の `countries` オブジェクトが保持される

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

#### 2.4.4: ヒューリスティック設定検出
- [ ] プロジェクト構造の自動検出機能
- [ ] 一般的な翻訳ファイルの場所を検索（`locales/en/*.json`, `public/locales/en/*.json` など）
- [ ] 検出された構造から設定を自動生成
- [ ] `status` や `lint` コマンドで設定ファイルなしでも動作

#### 達成基準
- [ ] ユーザーが既存の JS 設定ファイルをそのまま使える
- [ ] TypeScript 設定ファイルも読み込める
- [ ] 設定の検証とエラーメッセージ
- [ ] 設定ファイルなしで `status` コマンドが動作する

---

## 🚀 Phase 3: 圧倒的差別化 (v2.0.0 目標)

### Task 3.1: 追加コマンドの実装

#### 3.1.1: `status` コマンド ✅（基本実装完了）
- [x] 翻訳完了率の計算（キー数ベース）
- [x] ロケール別のサマリー表示
- [x] 詳細なキー別レポート（`status [locale]`）
- [ ] 名前空間フィルタ（`--namespace` オプション）
- [ ] プログレスバーの表示
- [ ] 非ゼロ終了コード（翻訳が不完全な場合）

#### 3.1.2: `sync` コマンド ✅
- [x] プライマリ言語ファイルの読み込み
- [x] セカンダリ言語ファイルとの比較
- [x] 不足キーの追加（デフォルト値で）
- [x] 未使用キーの削除（`--remove-unused` オプション）
- [x] 変更されたファイルの報告
- [x] `--dry-run` オプション

#### 3.1.3: `lint` コマンド ✅
- [x] ハードコードされた文字列の検出
- [x] JSX テキストノードの解析
- [x] JSX 属性の解析（`title`, `alt`, `placeholder`, `aria-label` など）
- [x] 無視ルールの設定（`ignoredTags`: script, style, code, pre）
- [x] エラーレポートの表示
- [x] `--fail-on-error` オプション（CI用）
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

#### 3.2.2: 設定ファイルの拡張（基本オプション）
- [ ] `preservePatterns`: 動的キーのパターン保持（glob パターン配列）
- [ ] `preserveContextVariants`: コンテキスト変種の保持
- [ ] `generateBasePluralForms`: ベース複数形の生成制御
- [ ] `disablePlurals`: 複数形の完全無効化
- [ ] `extractFromComments`: コメントからの抽出
- [ ] `removeUnusedKeys`: 未使用キーの削除（デフォルト: `true`）
- [ ] `ignore`: 抽出対象から除外するファイルパターン（glob 配列）

#### 3.2.3: セパレータと補間の設定 ✅（部分実装）
- [x] `keySeparator`: キーのセパレータ（デフォルト: `'.'`）
- [x] `nsSeparator`: 名前空間セパレータ（デフォルト: `':'`）
- [x] `contextSeparator`: コンテキストセパレータ（デフォルト: `'_'`）
- [x] `pluralSeparator`: 複数形セパレータ（デフォルト: `'_'`）
- [ ] `keySeparator: false` でフラットキーサポート
- [ ] `nsSeparator: false` で無効化
- [ ] `interpolationPrefix`: 補間プレフィックス（デフォルト: `'{{'`）
- [ ] `interpolationSuffix`: 補間サフィックス（デフォルト: `'}}'`）
- [ ] `nestingPrefix`: ネスト翻訳プレフィックス（デフォルト: `'$t('`）
- [ ] `nestingSuffix`: ネスト翻訳サフィックス（デフォルト: `')'`）
- [ ] `nestingOptionsSeparator`: ネスト翻訳オプションセパレータ（デフォルト: `','`）

#### 3.2.4: 言語とデフォルト値の設定
- [ ] `primaryLanguage`: プライマリ言語の指定（デフォルト: `locales[0]`）
- [ ] `secondaryLanguages`: セカンダリ言語の配列（自動計算も可能）
- [ ] `defaultValue`: デフォルト値の設定
  - 文字列形式: `''` または `'TODO'`
  - 関数形式: `(key, namespace, language, value) => string`
- [ ] `defaultNS`: デフォルト名前空間（デフォルト: `'translation'`、`false` で名前空間なし）

#### 3.2.5: ソートとフォーマット設定
- [ ] `sort`: キーのソート設定
  - ブール値: `true`（アルファベット順）または `false`（ソートなし）
  - 関数形式: `(a: ExtractedKey, b: ExtractedKey) => number`
- [ ] `indentation`: JSON のインデント
  - 数値形式: `2`（スペース数）
  - 文字列形式: `'\t'`（タブ）または `'  '`（スペース）

#### 3.2.6: Trans コンポーネント設定
- [ ] `transKeepBasicHtmlNodesFor`: Trans コンポーネントで保持する HTML タグ（デフォルト: `['br', 'strong', 'i']`）
- [ ] `transComponents`: 抽出対象の Trans コンポーネント名（デフォルト: `['Trans']`）

#### 3.2.7: 出力パスの関数形式サポート
- [ ] `output`: 関数形式のサポート
  - 文字列形式: `'locales/{{language}}/{{namespace}}.json'`
  - 関数形式: `(language: string, namespace?: string) => string`

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

#### 3.3.3: JavaScript ファイル出力
- [ ] `outputFormat: 'js'` または `'js-esm'`: ES Module 形式（`export default`）
- [ ] `outputFormat: 'js-cjs'`: CommonJS 形式（`module.exports`）
- [ ] プロジェクトのモジュールシステムに応じた自動選択

#### 3.3.4: 名前空間のマージ
- [ ] `mergeNamespaces: true` オプション
- [ ] 全名前空間を1ファイルに統合
- [ ] 出力パスの調整（`{{namespace}}` プレースホルダーなし）
- [ ] 既存ファイルの構造検出（名前空間付き vs フラット）

---

### Task 3.4: コメントからの抽出

#### 3.4.1: コメントパターンの検出
- [ ] `// t('key', 'default')` パターンの検出
- [ ] `/* t('key') */` パターンの検出
- [ ] オブジェクト構文の解析: `// t('key', { defaultValue: '...', ns: '...' })`
- [ ] 複数行コメントのサポート
- [ ] コメント内の複数形パターンの検出
- [ ] コメント内のコンテキストパターンの検出

#### 3.4.2: スコープ解決
- [ ] コメント内の `useTranslation` 参照の解決
- [ ] `keyPrefix` の適用
- [ ] 名前空間の解決

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

#### 3.5.3: Locize 設定オプション
- [ ] `locize.projectId`: プロジェクト ID
- [ ] `locize.apiKey`: API キー（環境変数推奨）
- [ ] `locize.version`: バージョン（デフォルト: `'latest'`）
- [ ] `locize.updateValues`: 既存翻訳値の更新
- [ ] `locize.sourceLanguageOnly`: ソース言語のみ同期
- [ ] `locize.compareModificationTime`: 変更時刻の比較
- [ ] `locize.cdnType`: CDN タイプ（`'standard'` または `'pro'`）
- [ ] `locize.dryRun`: プレビューモード

---

### Task 3.6: TypeScript 型生成の拡張

#### 3.6.1: 型生成設定の詳細
- [ ] `types.input`: 型生成元の翻訳ファイルパターン
- [ ] `types.output`: メインの型定義ファイルパス
- [ ] `types.resourcesFile`: リソースインターフェースファイルのパス
- [ ] `types.enableSelector`: セレクター API の有効化（`true`, `false`, `'optimize'`）
- [ ] `types.indentation`: 型定義ファイルのインデント

#### 3.6.2: セレクター API の型生成
- [ ] `enableSelector: true` の場合の型生成
- [ ] `enableSelector: 'optimize'` の場合の最適化された型生成
- [ ] 型安全なキー選択のサポート

#### 3.6.3: マージされた名前空間の型生成
- [ ] `mergeNamespaces: true` の場合の型生成
- [ ] 複数名前空間を含むファイルの型生成

---

### Task 3.7: Lint 設定の詳細

#### 3.7.1: Lint 設定オプション
- [ ] `lint.ignoredAttributes`: 無視する JSX 属性名のリスト
- [ ] `lint.ignoredTags`: 無視する JSX タグ名のリスト
- [ ] `lint.acceptedAttributes`: リント対象の JSX 属性名のリスト（ホワイトリスト）
- [ ] `lint.acceptedTags`: リント対象の JSX タグ名のリスト（ホワイトリスト）
- [ ] `lint.ignore`: リント対象から除外するファイルパターン

#### 3.7.2: リントロジックの実装
- [ ] デフォルトの推奨属性リスト（`alt`, `title`, `placeholder`, `aria-label` など）
- [ ] デフォルトの推奨タグリスト（`p`, `span`, `div`, `button`, `label` など）
- [ ] ホワイトリストとブラックリストの優先順位
- [ ] Trans コンポーネント内のコンテンツの無視

---

### Task 3.8: プラグインシステム

#### 3.8.1: プラグイン API の設計
- [ ] `Plugin` インターフェースの定義
- [ ] プラグインのライフサイクルフック
  - `setup`: 初期化
  - `onLoad`: ファイル読み込み前の変換
  - `onVisitNode`: AST ノード訪問時の処理
  - `onEnd`: 抽出完了後の処理
  - `afterSync`: 同期完了後の処理

#### 3.8.2: プラグインの実装例
- [ ] HTML ファイル用プラグインの例
- [ ] Handlebars テンプレート用プラグインの例
- [ ] カスタム抽出パターン用プラグインの例

#### 3.8.3: プラグインの読み込み
- [ ] 設定ファイルからのプラグイン読み込み
- [ ] プラグインのエラーハンドリング

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

### 未実装の重要な機能（i18next-cli との比較）
- ✅ **実装済み機能のCLI接続**（typegen、check、status コマンド追加完了）
- ✅ テンプレートリテラル（`t(\`key\`)`）のサポート
- ❌ ネストされた翻訳（`$t(...)` パターン）
- ❌ `returnObjects` のサポート
- ❌ フラットキー（`keySeparator: false`）
- ❌ セパレータの設定（`nsSeparator`, `contextSeparator`, `pluralSeparator`）
- ❌ 補間構文の設定（`interpolationPrefix`, `interpolationSuffix`）
- ❌ ネスト翻訳の設定（`nestingPrefix`, `nestingSuffix`, `nestingOptionsSeparator`）
- ❌ プライマリ/セカンダリ言語の設定
- ❌ `defaultValue` の関数形式
- ❌ `sort` の関数形式
- ❌ `indentation` の文字列形式
- ❌ `output` の関数形式
- ❌ `defaultNS: false` のサポート
- ❌ `transKeepBasicHtmlNodesFor` の設定
- ❌ プラグインシステム
- ❌ ヒューリスティック設定検出
- ❌ JavaScript ファイル出力（`js`, `js-esm`, `js-cjs`）
- ❌ 型生成の詳細設定（`enableSelector`, `resourcesFile`）
- ❌ Lint 設定の詳細（`acceptedAttributes`, `acceptedTags`）

### 技術的負債
- [ ] エラーハンドリングの改善
- [ ] ログレベルの設定
- [ ] パフォーマンス最適化
- [ ] メモリ使用量の最適化

---

## 🎯 優先度マトリックス

### P0 (最優先 - 即座に実装)
1. ~~**Task 1.3: 実装済み機能のCLI接続（Wiring）**~~ ✅ **完了**
2. Task 1.1: napi-rs の導入
3. Task 1.2: CI/CD の構築
4. Task 2.4: JS/TS 設定ファイルの読み込み

### P1 (高優先度 - Phase 2 完了のため)
5. Task 2.1: `<Trans>` コンポーネントの完全対応
6. Task 2.2: 言語別複数形カテゴリの生成
7. Task 2.3.1: `useTranslation` hook のスコープ解決
8. Task 2.3.2: `getFixedT` のサポート
9. ~~Task 2.3.8: テンプレートリテラルのサポート~~ ✅ **完了**

### P2 (中優先度 - 差別化機能)
8. ~~Task 3.1.1: `status` コマンド~~ ✅ **完了**
9. ~~Task 3.1.2: `sync` コマンド~~ ✅ **完了**
10. ~~Task 3.1.3: `lint` コマンド~~ ✅ **完了**
11. Task 3.2: 高度な設定オプション

### P3 (低優先度 - 拡張機能)
12. Task 3.1.4: `rename-key` コマンド
13. Task 3.1.5: `init` コマンド
14. Task 3.1.6: `migrate-config` コマンド
15. Task 3.3: 出力フォーマットの多様化
16. Task 3.4: コメントからの抽出
17. Task 3.5: Locize 統合
18. Task 2.3.6: ネストされた翻訳のサポート
19. Task 2.3.7: returnObjects のサポート
20. Task 3.2.3-3.2.7: 詳細な設定オプション
21. Task 3.6: TypeScript 型生成の拡張
22. Task 3.7: Lint 設定の詳細
23. Task 3.8: プラグインシステム
24. Task 2.4.4: ヒューリスティック設定検出

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

## 🔍 セルフレビュー結果（i18next-cli との比較）

### 評価レポートに基づく追加タスク（2025-01-XX）

以下のタスクが評価レポートの指摘に基づいて追加されました：

1. ~~**Task 1.3: 実装済み機能のCLI接続（Wiring）**~~ ✅ **完了**
   - `typegen.rs` と `cleanup.rs` がCLIから呼び出し可能に
   - `Commands` Enum に `Typegen`、`Check`、`Status` バリアントを追加
   - `extract --generate-types` オプションも追加

2. ~~**Task 2.3.8: テンプレートリテラルのサポート**~~ ✅ **完了**
   - `t(\`key\`)` パターンの検出を実装
   - `Expr::Tpl` ノードの処理を追加
   - 静的テンプレートリテラルの抽出とテスト追加

### 以前に追加された重要なタスク

以下のタスクが以前のセルフレビューで追加されました：

1. **Task 2.3.6: ネストされた翻訳のサポート**
   - `$t(...)` パターンの検出と抽出
   - 文字列内のネストされた翻訳の処理

2. **Task 2.3.7: returnObjects のサポート**
   - 構造化コンテンツの保持
   - オブジェクトキーの自動保持

3. **Task 2.4.4: ヒューリスティック設定検出**
   - 設定ファイルなしでの動作
   - プロジェクト構造の自動検出

4. **Task 3.2.3-3.2.7: 詳細な設定オプション**
   - セパレータと補間の設定
   - 言語とデフォルト値の設定
   - ソートとフォーマット設定
   - Trans コンポーネント設定
   - 出力パスの関数形式

5. **Task 3.3.3: JavaScript ファイル出力**
   - ES Module と CommonJS のサポート

6. **Task 3.6: TypeScript 型生成の拡張**
   - セレクター API の型生成
   - マージされた名前空間の型生成

7. **Task 3.7: Lint 設定の詳細**
   - ホワイトリスト/ブラックリストのサポート
   - デフォルト推奨リスト

8. **Task 3.8: プラグインシステム**
   - 拡張可能なアーキテクチャ

### 確認事項

- ✅ 全ての主要な i18next-cli 機能が TODO に含まれている
- ✅ 設定オプションの詳細が網羅されている
- ✅ 出力フォーマットの多様性が考慮されている
- ✅ プラグインシステムが計画に含まれている
- ✅ ヒューリスティック設定検出が考慮されている

---

最終更新: 2025-01-XX

