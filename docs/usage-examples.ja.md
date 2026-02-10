# 使用例

## 基本抽出

```bash
i18next-turbo extract
```

## 型生成を同時実行

```bash
i18next-turbo extract --generate-types
```

## デッドキー検出

```bash
i18next-turbo check
```

## デッドキー削除

```bash
i18next-turbo check --remove
```

## 名前空間を指定したステータス

```bash
i18next-turbo status --namespace common
```

## 名前空間マージ出力

```json
{
  "mergeNamespaces": true,
  "mergedNamespaceFilename": "all"
}
```

出力例:
- `locales/en/all.json`
- `locales/ja/all.json`
