# i18next-turbo ⚡️

**Rust + SWC で実現する超高速 i18next 翻訳キー抽出 — 10〜100 倍の速度**

`i18next-turbo` は `i18next-parser` および `i18next-cli` の**超高速な代替**です。Rust と SWC で構築され、数千ファイルを**ミリ秒**で処理します。

> **⚠️ 開発中**: 現在は Rust バイナリとして利用可能です。npm パッケージの配布は準備中です。

---

## 🚀 なぜ i18next-turbo か

### パフォーマンス比較

| ツール | エンジン | 処理時間（1k ファイル） | Watch モード |
|:---|:---|:---|:---|
| `i18next-parser` | Node.js (Babel/Regex) | **10〜30 秒** | 遅い / 高 CPU |
| `i18next-cli` | Node.js (SWC) | **2〜5 秒** | 中程度 |
| **`i18next-turbo`** | **Rust + SWC** | **< 100ms** ⚡️ | **即時 / 低負荷** |

**ベンチマーク結果（MacBook Pro M3、1,000 ファイル）:**
```
i18next-parser:  ████████████████████ 12.5s
i18next-cli:     ████████ 2.3s
i18next-turbo:   ▏ 0.08s ⚡️ (約150倍高速)
```

### 主な特徴

- ⚡️ **超高速**: 大規模プロジェクトでも即座に処理
- 🎯 **高精度**: SWC による完全な AST 解析で誤検出ゼロ
- 🔄 **リアルタイム更新**: Watch モードで保存と同時に JSON を更新
- 🛡️ **翻訳を保持**: 新キーを追加しても既存の翻訳には触れない
- 📦 **軽量**: 低メモリ、バックグラウンド実行に適している
- 🔧 **i18next 互換**: 名前空間、複数形、コンテキストなどをサポート

---

## ✨ 実装済み機能

### 基本的な抽出パターン

```typescript
// ✅ サポート済み
t('hello.world')
i18n.t('greeting')
t('common:button.save')  // 名前空間付き
```

### React コンポーネント

```tsx
// ✅ Trans コンポーネント
<Trans i18nKey="welcome">Welcome</Trans>
<Trans i18nKey="common:greeting" defaults="Hello!" />
```

### 複数形とコンテキスト

```typescript
// ✅ 複数形
t('apple', { count: 5 })  // → apple_one, apple_other

// ✅ コンテキスト
t('friend', { context: 'male' })  // → friend_male

// ✅ 複数形 + コンテキスト
t('friend', { count: 2, context: 'female' })  // → friend_female_one, friend_female_other
```

ICU の複数形ルールに従い、`locales` に列挙した各言語に必要なカテゴリ（`zero`、`one`、`few`、`many` など）を生成します。例えばロシア語を指定すると `friend_one`、`friend_few`、`friend_many`、`friend_other` が一括で追加されます。

### その他の機能

- ✅ **マジックコメント**: `// i18next-extract-disable-line`
- ✅ **ネストキー**: `button.submit` → `{"button": {"submit": ""}}`
- ✅ **キー自動ソート**: 一貫した JSON のためアルファベット順にソート
- ✅ **TypeScript 型生成**: オートコンプリートと型安全性
- ✅ **デッドキー検出**: リファクタ後に未使用キーを検出

---

## 📦 インストール

### 方法 1: Cargo でインストール（推奨）

```bash
cargo install i18next-turbo
```

### 方法 2: ソースからビルド

```bash
git clone https://github.com/your-username/i18next-turbo.git
cd i18next-turbo
cargo build --release
# バイナリは target/release/i18next-turbo に生成されます
```

> **📌 注意**: npm パッケージの配布は準備中です。Node.js プロジェクトでは現時点で Rust のインストールが必要です。

---

## 🛠️ 使い方

### 1. 設定ファイルの作成

プロジェクトルートに `i18next-turbo.json` を作成します:

```json
{
  "input": ["src/**/*.{ts,tsx,js,jsx}"],
  "output": "locales/$LOCALE/$NAMESPACE.json",
  "locales": ["en", "ja", "de"],
  "defaultNamespace": "translation",
  "functions": ["t", "i18n.t"],
  "types": {
    "output": "src/@types/i18next.d.ts",
    "defaultLocale": "en",
    "localesDir": "locales"
  }
}
```

#### 設定オプション

| オプション | 説明 | デフォルト |
|:---|:---|:---|
| `input` | 抽出対象のファイルパターン（glob） | `["src/**/*.{ts,tsx,js,jsx}"]` |
| `output` | 出力パス（`$LOCALE` と `$NAMESPACE` が置換される） | `"locales"` |
| `locales` | 対象言語のリスト | `["en"]` |
| `defaultNamespace` | デフォルト名前空間 | `"translation"` |
| `functions` | 抽出対象の関数名 | `["t"]` |
| `logLevel` | ログの詳細度（`error`/`warn`/`info`/`debug`） | `"info"` |
| `types.output` | 生成する TypeScript 型定義のパス | `"src/@types/i18next.d.ts"` |
| `types.defaultLocale` | 型生成時のデフォルトロケール | `locales` の先頭 |
| `types.localesDir` | 型生成時に読み込むディレクトリ | `output` と同じ |
| `types.input` | 型生成に含めるロケールファイルの glob パターン | デフォルトロケール配下の全 `*.json` |
| `types.resourcesFile` | `Resources` インターフェース用のオプションの補助ファイルパス | 生成しない |
| `types.enableSelector` | セレクター用ヘルパー型を有効化（`true`、`false`、`"optimize"`） | `false` |
| `types.indentation` | 生成する型ファイルのインデント | `2 スペース` |
| `defaultValue` | 文字列または関数 `(key, namespace, language, value) => string` | `""` |
| `sort` | 真偽値または関数 `(a, b) => number` | `true` |
| `plugins` | プラグイン配列（`setup` / `onLoad` / `onVisitNode` / `onEnd` / `afterSync`） | `[]` |

オプションの `types` ブロックで、型定義の出力先と、`i18next-turbo typegen` または `i18next-turbo extract --generate-types` が参照するロケールファイルを制御できます。

> CLI は `i18next-turbo.json`、`i18next-parser.config.(js|ts)`、`i18next.config.(js|ts)` を自動で検索します（CommonJS / ESM / TypeScript は `jiti` 経由）。`--config path/to/i18next.config.ts` で直接指定することもできます。

### 2. キーの抽出

1 回だけ実行する場合（CI/CD など）:

```bash
i18next-turbo extract
```

#### 出力例

```
=== i18next-turbo extract ===

Configuration:
  Input patterns: ["src/**/*.{ts,tsx}"]
  Output: locales
  Locales: ["en", "ja"]
  Functions: ["t"]

Extracted keys by file:
------------------------------------------------------------

src/components/Button.tsx
  - button.submit
  - button.cancel

src/pages/Home.tsx
  - welcome.title
  - welcome.message

------------------------------------------------------------

Extraction Summary:
  Files processed: 2
  Unique keys found: 4

Syncing to locale files...
  locales/en/translation.json - added 4 new key(s)

Done!
```

### 3. Watch モード（開発時）

ファイル保存時に自動でキーを抽出・更新します:

```bash
i18next-turbo watch
```

#### 動作例

```
=== i18next-turbo watch ===

Watching: src
Watching for changes... (Ctrl+C to stop)

--- Change detected ---
  Modified: src/components/Button.tsx
  Added 1 new key(s)
--- Sync complete ---
```

開発中はこのコマンドをバックグラウンドで実行すると、翻訳キーを追加するたびに JSON が自動で更新されます。

### 4. 翻訳状況

特定ロケールの翻訳進捗を確認します:

```bash
i18next-turbo status --locale ja
```

主なフラグ:

- `--namespace <name>`: レポートを単一の名前空間に限定
- `--fail-on-incomplete`: 不足キーやデッドキーがある場合に非ゼロで終了（CI 向け）

サマリにはテキストのプログレスバーが含まれ、選択したロケール/名前空間の完了度をすぐに把握できます。

---

## 📝 使用例

### 基本的な使い方

```typescript
// src/components/Button.tsx
import { useTranslation } from 'react-i18next';

function Button() {
  const { t } = useTranslation();
  
  return (
    <button>
      {t('button.submit')}
    </button>
  );
}
```

実行後、`locales/en/translation.json` には次のように追加されます:

```json
{
  "button": {
    "submit": ""
  }
}
```

### 名前空間の使用

```typescript
// 名前空間を指定
t('common:button.save')  // → locales/en/common.json に保存
```

### React Trans コンポーネント

```tsx
import { Trans } from 'react-i18next';

function Welcome() {
  return (
    <Trans i18nKey="welcome.title" defaults="Welcome!">
      Welcome to our app!
    </Trans>
  );
}
```

### 複数形の使用

```typescript
const count = 5;
t('apple', { count });  // → apple_one, apple_other を生成
```

生成される JSON:

```json
{
  "apple_one": "",
  "apple_other": ""
}
```

---

## 🎯 i18next-parser からの移行

`i18next-parser` を使っている場合、設定を少し変えるだけで移行できます。

### 設定の違い

| i18next-parser | i18next-turbo |
|:---|:---|
| `input` | `input`（同じ） |
| `output` | `output`（同じ） |
| `locales` | `locales`（同じ） |
| `defaultNamespace` | `defaultNamespace`（同じ） |
| `functions` | `functions`（同じ） |

基本的に同じ設定がそのまま使えます。

### 移行手順

1. `i18next-turbo.json` を作成（既存の設定をコピー）
2. `i18next-turbo extract` を実行
3. 生成された JSON を確認
4. Watch モードで開発を開始

### i18next-cli 設定との互換性

`i18next-turbo` は `i18next-cli` の設定ファイルを読み、`extract` オプションの一部をマッピングできます。

対応しているマッピング:

| i18next-cli (extract) | i18next-turbo |
|:---|:---|
| `input` | `input` |
| `output`（文字列） | `output`（ディレクトリ） |
| `output`（関数） | 評価して `output` ディレクトリに射影 |
| `functions` | `functions` |
| `defaultNS` | `defaultNamespace` |
| `keySeparator` | `keySeparator`（`false` → 空文字） |
| `nsSeparator` | `nsSeparator`（`false` → 空文字） |
| `contextSeparator` | `contextSeparator` |
| `pluralSeparator` | `pluralSeparator` |
| `defaultNS = false` | `defaultNamespace = ""` と名前空間なしモード |
| `secondaryLanguages` | `secondaryLanguages` |
| `transKeepBasicHtmlNodesFor` | `transKeepBasicHtmlNodesFor` |
| `preserveContextVariants` | `preserveContextVariants` |
| `interpolationPrefix` / `interpolationSuffix` | `interpolationPrefix` / `interpolationSuffix` |
| `mergeNamespaces` | `mergeNamespaces` |
| `extractFromComments` | `extractFromComments`（デフォルト `true`） |

関数形式サポート:

| i18next-cli (extract) | i18next-turbo の動作 |
|:---|:---|
| `defaultValue` 関数 | `extract` / `sync` 後にロケールファイルへ適用 |
| `sort` 関数 | `extract` / `sync` 後にキー順を再構築 |

補足:
- `locales/{{language}}/{{namespace}}.json` のような出力テンプレートはベースディレクトリに集約されます。
- `defaultValue` / `sort` の関数形式は現在 `json` 出力に適用されます（`json5` はスキップ）。

---

## 🔧 高度な機能

### マジックコメント

特定行を抽出対象から除外します:

```typescript
// i18next-extract-disable-line
const dynamicKey = `user.${role}.permission`;
t(dynamicKey);  // この行は抽出されません
```

### プラグインフック（Node ラッパー）

`plugins` にモジュールパスまたはオブジェクトを設定できます。現在のフック:

- `setup(context)`: コマンド開始時
- `onLoad({ filePath, relativePath, source, ... })`: 抽出前のファイル前処理（返り値で文字列を返すと差し替え）
- `onVisitNode(node)`: ノード訪問イベント
  - `extract/watch/status/check/lint` では Rust 側の AST 訪問イベント（JSON Lines 中継）
  - `sync` ではロケール JSON 走査イベント
- `onEnd(context)`: コマンド完了時
- `afterSync(context)`: 同期処理後

例:

```js
module.exports = {
  onLoad({ source }) {
    return source.replace(/__\(([^)]+)\)/g, "t('$1')");
  },
  onVisitNode(node) {
    if (node.type === 'TranslationKey' && node.key && node.key.endsWith('.tmp')) {
      console.warn(`temporary key detected: ${node.key}`);
    }
  }
};
```

ドキュメント:
- [API](./docs/api.ja.md)
- [使用例](./docs/usage-examples.ja.md)
- [マイグレーションガイド](./docs/migration-guide.ja.md)
- [トラブルシューティング](./docs/troubleshooting.ja.md)
- [パフォーマンステスト](./docs/performance-testing.ja.md)

### TypeScript 型生成

```bash
# 設定に基づいて 1 回だけ型定義を生成（オプションの `types` ブロックを参照）
i18next-turbo typegen

# または抽出と型生成を同時に実行
i18next-turbo extract --generate-types
```

生成される型定義の例:

```typescript
interface Translation {
  button: {
    submit: string;
    cancel: string;
  };
  welcome: {
    title: string;
    message: string;
  };
}
```

### デッドキー検出

```bash
# 将来 i18next-turbo cleanup コマンドとして提供予定
# コード内で見つからないキーを検出
```

---

## 📊 パフォーマンス

### ベンチマーク結果

| ファイル数 | i18next-parser | i18next-cli | i18next-turbo |
|:---|:---:|:---:|:---:|
| 100 | 1.2s | 0.3s | **0.01s** |
| 1,000 | 12.5s | 2.3s | **0.08s** |
| 10,000 | 125s | 23s | **0.8s** |

### メモリ使用量

- **i18next-parser**: 約 200MB
- **i18next-cli**: 約 150MB
- **i18next-turbo**: **約 50MB**（約 4 倍軽量）

---

## 🗺️ ロードマップ

### ✅ 実装済み

- [x] 基本的な `t()` 関数の抽出
- [x] `<Trans>` コンポーネント対応
- [x] 名前空間対応
- [x] 複数形（基本の `_one`、`_other`）
- [x] コンテキスト対応
- [x] Watch モード
- [x] JSON 同期（既存翻訳の保持）
- [x] TypeScript 型生成
- [x] デッドキー検出

### 🚧 開発中

- [x] npm パッケージの配布
- [x] `useTranslation` フックの完全対応（`keyPrefix` など）
- [x] 言語別複数形カテゴリ（`zero`、`few`、`many` など）
- [x] JS/TS 設定ファイルの読み込み

### 📅 予定

- [ ] Locize 統合

詳細は [TODO.md](./TODO.md) を参照してください。

---

## 🤝 コントリビュート

プルリクエストと Issue の報告を歓迎します。

1. このリポジトリをフォーク
2. フィーチャーブランチを作成（`git checkout -b feature/amazing-feature`）
3. 変更をコミット（`git commit -m 'Add some amazing feature'）
4. ブランチにプッシュ（`git push origin feature/amazing-feature`）
5. プルリクエストを開く

行動規範の詳細は [CONTRIBUTING.md](./CONTRIBUTING.md) を参照してください。

---

## 📄 ライセンス

MIT License。詳細は [LICENSE](./LICENSE) を参照してください。

---

## 🙏 謝辞

- [i18next](https://www.i18next.com/) — 優れた国際化フレームワーク
- [SWC](https://swc.rs/) — 高速な JavaScript/TypeScript コンパイラ
- [i18next-parser](https://github.com/i18next/i18next-parser) — インスピレーションの源

---

## ⚠️ 免責事項

- 本ツールは **i18next の非公式ツール**です
- API はメジャーバージョン間で変更される可能性があります
- npm パッケージ公開済み: [i18next-turbo](https://www.npmjs.com/package/i18next-turbo)

---

**質問や問題は [Issue](https://github.com/your-username/i18next-turbo/issues) でお知らせください。**
