# i18next-cli と i18next-turbo のテスト・カバレッジ比較

調査日: 2026-02-10（更新）

## 1. 概要

| 項目 | i18next-cli | i18next-turbo |
|------|-------------|---------------|
| 言語 | TypeScript (Node.js) | Rust |
| テストフレームワーク | Vitest | cargo test (標準) |
| テストファイル数 | 57 | 2 (integration) + 各モジュール内 unit |
| テスト数 | 542 (1 timeout 失敗あり) | 140 (lib) + 6 (bin) + 27 (integration) |
| カバレッジ計測 | あり (v8) | CI では未実施 |
| カバレッジ値 | **Stmts 84.59%**, Branch 76.27%, Funcs 89.32%, Lines 86.83% | **Regions 75.09% / Functions 76.09% / Lines 75.23%**（`cargo llvm-cov --summary-only`） |

---

## 2. i18next-cli のカバレッジ詳細

- **対象**: `src/**/*`（除外: `src/types.ts`, `src/index.ts`, `src/extractor/index.ts`）
- **計測**: `npm run test:coverage` → `vitest run --coverage`（@vitest/coverage-v8）

### カバレッジ by モジュール（抜粋）

| ファイル/ディレクトリ | Stmts | Branch | Funcs | Lines |
|----------------------|-------|--------|-------|-------|
| **All files** | 84.59 | 76.27 | 89.32 | 86.83 |
| src/cli.ts | 55.63 | 48.93 | 60.71 | 57.25 |
| src/config.ts | 82.71 | 78.12 | 85.71 | 82.05 |
| src/extractor.ts | 0 | 0 | 0 | 0 (除外扱いの再エクスポート) |
| src/linter.ts | 71.55 | 66.39 | 88.46 | 73.6 |
| src/syncer.ts | 92.72 | 78.37 | 100 | 92.3 |
| src/status.ts | 94.8 | 75.26 | 92.3 | 95.89 |
| src/types-generator.ts | 91.78 | 83.33 | 83.33 | 90.9 |
| src/extractor/core/* | 89.52 | 77.84 | 96.29 | 93.54 |
| src/extractor/parsers/* | 84.77 | 76.64 | 93.95 | 87.13 |
| src/utils/* | 92.85 | 89.53 | 85 | 92.68 |

---

## 3. i18next-turbo のテスト構成

### 3.1 単体テスト（`src/**/*.rs` 内 `#[cfg(test)] mod tests`）

- **extractor.rs**: 約 60+ テスト（t(), Trans, useTranslation, コメント抽出, 複数形, コンテキスト, Vue/Svelte など）
- **json_sync.rs**: 約 28 テスト（JSON スタイル検出, マージ, 削除, シリアライズ, JS/TS ロケール出力など）
- **config.rs**: 9 テスト（plural, namespace, types 設定など）
- **lint.rs**: 7 テスト（ハードコード文字列, 属性, 無視パターンなど）
- **typegen.rs**: 5 テスト（TS 生成, pascalCase, enableSelector など）
- **cleanup.rs**: 3 テスト（ネストキー削除）
- **commands/locize.rs**: 19 テスト（設定分岐・フォーマット・namespace 解決を追加）
- **commands/init.rs**: 2 テスト
- **commands/rename_key.rs**: 2 テスト
- **fs.rs**, **watcher.rs**, **main.rs**: watcher/main のテストを拡張（各 3+）

合計: **約 135 個**の unit test（lib）

### 3.2 統合テスト（`tests/*.rs`）

**cli_integration.rs（17 テスト）**

| テスト名 | 保証していること |
|----------|------------------|
| extract_creates_locale_file | extract でロケール JSON が生成される |
| extract_generates_types_file | extract --generate-types で型ファイル生成 |
| extract_fail_on_warnings_returns_error | 動的キー等で --fail-on-warnings 時にエラー |
| extract_writes_json5_when_configured | outputFormat: json5 で .json5 出力 |
| extract_writes_js_module_when_requested | outputFormat: js で .js モジュール出力 |
| extract_writes_ts_module_when_requested | outputFormat: ts で .ts モジュール出力 |
| sync_adds_missing_keys_to_secondary_locale | sync で他ロケールにキーが追加される |
| sync_remove_unused_respects_dry_run | sync --remove-unused の dry-run と本実行 |
| status_fail_on_incomplete_returns_error | status --fail-on-incomplete で未翻訳時にエラー |
| typegen_command_generates_file | typegen コマンドで型ファイル生成 |
| check_dry_run_reports_dead_keys | check --dry-run でデッドキー報告 |
| lint_fail_on_error_returns_non_zero | ハードコード文字列で lint --fail-on-error が非ゼロ |
| lint_passes_when_only_translated_text_exists | 翻訳のみの場合は lint 成功 |
| migrate_dry_run_prints_preview_for_existing_turbo_config | migrate --dry-run でプレビュー表示 |
| extract_fails_on_invalid_utf8_source | 不正UTF-8入力で extract が非ゼロ終了し読み込みエラーを返す |
| extract_fails_on_unreadable_source_file (unix) | 権限不足で読み取り不可ファイル時に非ゼロ終了する |
| extract_handles_large_input_without_panicking | 大規模入力（3,000キー+深いネスト）でもパニックせず完走する |

**extraction_patterns.rs（6 テスト）**

- t()/i18n.t()/Trans/useTranslation/comment など主要パターンの抽出回帰

**node_wrapper_integration.rs（6 テスト、Node と cosmiconfig/jiti がある場合のみ）**

- node_wrapper_supports_default_value_and_sort_function_forms
- node_wrapper_plugin_onload_transforms_source_and_aftersync_runs
- node_wrapper_plugin_onvisitnode_receives_rust_ast_events
- node_wrapper_merge_namespaces_without_namespace_placeholder_writes_single_file
- node_wrapper_plugin_errors_continue_by_default
- node_wrapper_plugin_errors_fail_fast_when_configured

---

## 4. 機能別「同様の作業が保証されているか」対応表

| 機能 | i18next-cli のテスト | i18next-turbo のテスト | 同様の保証度 |
|------|----------------------|-------------------------|--------------|
| **extract（基本）** | extractor.runExtractor.*, extractor.extract, extractor.findKeys, extractor.getTranslations, extractor.t, extractor.Trans, extractor.*.test.ts 多数 | extractor::tests::* (60+), extract_creates_locale_file 等 | ◎ 両方で強くカバー |
| **extract（出力形式）** | extractor.runExtractor.formats, extractor.runExtractor.json5 | extract_writes_json5_when_configured, extract_writes_js_module_*, extract_writes_ts_module_* | ◎ 同等のフォーマットを検証 |
| **extract（コメント）** | extractor.extractFromComments, extractor.comment-parser | extractor::tests::test_extract_from_comment_* | ◎ コメント抽出を両方で検証 |
| **extract（defaultValue）** | extractor.runExtractor.defaultValue, runExtractor-primary-defaults | extractor::tests::test_default_value_* | ◎ 同等 |
| **sync** | syncer.test, syncer.formats.test | json_sync::tests::*, sync_adds_missing_keys_*, sync_remove_unused_* | ◎ 同期・削除を両方で検証 |
| **status** | status.test, status.formats.test | status_fail_on_incomplete_returns_error, config/plural 等 | ○  CLI の方が詳細（複数ロケール・複数形など） |
| **typegen** | types-generator*.test.ts | typegen::tests::*, typegen_command_generates_file | ◎ 型生成を両方で検証 |
| **check（デッドキー）** | syncer の remove-unused 関連 | check_dry_run_reports_dead_keys, cleanup::tests | ○  turbo は check コマンドで明示的にテスト |
| **lint** | linter.test（32 tests） | lint::tests::*, lint_fail_on_error_*, lint_passes_* | ◎ ハードコード検出・属性等を両方で検証 |
| **migrate** | migrator.test | migrate_dry_run_prints_preview_* | ○ 両方でマイグレーションを検証（CLI の方が多様な可能性） |
| **rename-key** | rename-key.test（30 tests） | commands::rename_key::tests::*, (integration にはなし) | ○  CLI の方がケース数が多い |
| **init** | init.test | commands::init::tests | ○ 両方で検証 |
| **config** | config.test, config.ensureConfig.test | config::tests::* | ◎ 設定解釈を両方で検証 |
| **locize** | locize.test, locize-output-function | commands::locize::tests | ○ 両方で検証（CLI は E2E 的） |
| **plugin** | plugin.*.test.ts（Vue, Svelte, Astro, Handlebars, HTML, TypeScript, afterSync 等） | node_wrapper の 5 テスト（defaultValue/sort, onload/aftersync, onvisitnode, error mode） | △  CLI はプラグイン多様、turbo は Node ラッパー経由の一部のみ |
| **CLI 入口** | cli.test, cli.config.test | main.rs の tests（3 件すべて通過） | ○ 基本分岐は検証済み |

---

## 5. 結論と推奨

### 同様に保証されている部分（◎）

- **extract**: キー抽出、t/Trans/useTranslation、コメント、defaultValue、出力形式（json/json5/js/ts）は両プロジェクトでテストされている。
- **sync**: キー追加・削除・フォーマットは両方でカバーされている。
- **typegen**: 型生成は両方でテストあり。
- **lint**: ハードコード検出とオプションは両方で検証されている。
- **config**: 設定の解釈は両方で単体テストがある。

### CLI の方が厚い部分（○）

- **status**: 複数ロケール・複数形・namespace フィルタなどは i18next-cli の方がテスト数・シナリオが豊富。
- **rename-key**: i18next-cli は 30 テストで多様なケースを検証；turbo は rename_key の unit のみで integration はなし。
- **migrate / init / locize**: いずれも CLI の方が E2E 的なテストが多く、turbo は必要最小限。

### turbo で不足しがちな部分（△）

- **プラグイン**: turbo は Node ラッパー経由の 5 テスト（エラーモード含む）を持つが、Vue/Svelte/Astro/Handlebars/HTML 等の個別プラグイン群は CLI 側が依然厚い。
- **カバレッジ運用**: turbo は `cargo llvm-cov` で計測可能。現在は手動計測ベースのため、CI 常時収集は未導入。
- **CLI 入口**: main.rs テストの失敗は解消済み。

### 正常系/異常系のカバレッジ方針（Issue #61）

- 正常系: 既存コマンドの主要ユースケース（extract/sync/status/typegen/check/lint/migrate）を integration で維持。
- 異常系: 入力破損（不正UTF-8）、I/O失敗（権限/読み取り不可）、極端入力（巨大ファイル/深いネスト）を最小1ケース以上追加。
- 失敗契約: 異常系は「非ゼロ終了コード + stderr のエラー文言」を必ず検証。
- CI運用: 異常系テストを通常の `cargo test --all` に含め、常時実行する。

### 数値まとめ

- **i18next-cli**: 約 **86% Lines カバレッジ**、542 テスト、57 ファイル。extract/sync/status/typegen/lint/config などは高いカバレッジでカバーされている。
- **i18next-turbo**: **単体 140 + bin 6 + 統合 27**。`cargo llvm-cov` 実測で **Regions 75.09% / Functions 76.09% / Lines 75.23%**。extract/sync/status/typegen/lint/check/migrate の主要フローは integration で保証され、異常系（不正UTF-8・権限・巨大入力）も回帰検知対象に追加済み。

全体として、**コア機能（extract, sync, typegen, lint, config）については両方で同様の動作がテストで保証されている**。status/rename-key は CLI の方が厚く、turbo は plugin 拡張周辺（多言語テンプレート専用プラグイン群）でまだ差がある。
