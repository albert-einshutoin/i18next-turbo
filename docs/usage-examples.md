# Usage Examples

## Basic extraction

```bash
i18next-turbo extract
```

## Generate types together

```bash
i18next-turbo extract --generate-types
```

## Check dead keys

```bash
i18next-turbo check
```

## Remove dead keys

```bash
i18next-turbo check --remove
```

## Status for specific namespace

```bash
i18next-turbo status --namespace common
```

## Merge namespaces into one file

```json
{
  "mergeNamespaces": true,
  "mergedNamespaceFilename": "all"
}
```

Output example:
- `locales/en/all.json`
- `locales/ja/all.json`
