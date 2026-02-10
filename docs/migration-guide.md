# Migration Guide

## From i18next-parser / i18next-cli

1. Keep your existing source globs and locale directory.
2. Create `i18next-turbo.json` or keep JS/TS config via Node wrapper.
3. Map key options:
   - `defaultNS` -> `defaultNamespace`
   - `keySeparator` / `nsSeparator`: `false` becomes empty string
   - `mergeNamespaces` is supported
4. Run dry checks:
   - `i18next-turbo status`
   - `i18next-turbo check --dry-run`
5. Run extraction:
   - `i18next-turbo extract`

## Existing merged-file projects

If you already use a merged file (for example `all.json`), set:

```json
{
  "mergeNamespaces": true,
  "mergedNamespaceFilename": "all"
}
```

When omitted, turbo tries to reuse existing single-file layout automatically.
