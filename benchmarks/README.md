# Benchmarks

## Fixtures

### `fixtures/large` — 1 file, 5000 keys

- **src/app.ts**: 1 ファイルに `t('translate_target1')` … `t('translate_target5000')` を 5000 件
- **locales/en/translation.json**: `translate_targetK` → `translated_targetK` のフラットな JSON
- **i18next-turbo.json**: i18next-turbo 用 extract 設定
- **i18next.config.cjs**: i18next-cli 用 extract 設定（同一内容）

再生成（件数指定可能、省略時 5000）:

```bash
node scripts/generate-bench-fixtures.mjs [count]
```

例: 10000 件で再生成

```bash
node scripts/generate-bench-fixtures.mjs 10000
```

## i18next-cli と i18next-turbo の比較ベンチマーク

同一フィクスチャで両 CLI の `extract` を実行し、平均・最小・最大時間と倍率を表示します。

**前提**

- i18next-turbo: `cargo build --release` 済み、または `i18next-turbo` が PATH にあること
- i18next-cli: `npx i18next-cli` で実行（npm のパッケージを使用）

**実行（リポジトリルートで）:**

```powershell
node scripts/run-benchmark.mjs [runs]
```

`runs` は 1 ツールあたりの計測回数（省略時 3）。ウォームアップ 1 回のあと、指定回数だけ計測して avg/min/max を出し、最後に「i18next-turbo is ~X.XXx faster」を表示します。

例:

```powershell
node scripts/run-benchmark.mjs 5
```

## 手動で個別に計測する場合

フィクスチャディレクトリで `extract` を実行し、所要時間を計測します。

**i18next-turbo:**

```powershell
cd benchmarks/fixtures/large
Measure-Command { i18next-turbo --config i18next-turbo.json extract }
```

**i18next-cli:**

```powershell
cd benchmarks/fixtures/large
Measure-Command { npx i18next-cli extract --config i18next.config.cjs }
```

**複数回計測（例: hyperfine）:**

```bash
cd benchmarks/fixtures/large
hyperfine -w 1 "i18next-turbo --config i18next-turbo.json extract" "npx i18next-cli extract --config i18next.config.cjs"
```
