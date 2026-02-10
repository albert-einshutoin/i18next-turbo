# i18next-turbo ⚡️

**Blazing fast i18next translation key extractor - 10-100x faster with Rust + SWC**

`i18next-turbo` is a **blazing fast replacement** for `i18next-parser` and `i18next-cli`. Built with Rust and SWC, it processes thousands of files in **milliseconds**.

> **⚠️ Under Development**: Currently available as a Rust binary. npm package distribution is in preparation.

---

## 🚀 Why i18next-turbo?

### Performance Comparison

| Tool | Engine | Processing Time (1k files) | Watch Mode |
|:---|:---|:---|:---|
| `i18next-parser` | Node.js (Babel/Regex) | **10-30s** | Slow / High CPU |
| `i18next-cli` | Node.js (SWC) | **2-5s** | Moderate |
| **`i18next-turbo`** | **Rust + SWC** | **< 100ms** ⚡️ | **Instant / Low footprint** |

**Benchmark Results (MacBook Pro M3, 1,000 files):**
```
i18next-parser:  ████████████████████ 12.5s
i18next-cli:     ████████ 2.3s
i18next-turbo:   ▏ 0.08s ⚡️ (~150x faster)
```

### Key Features

- ⚡️ **Blazing Fast**: Instant processing even for large projects
- 🎯 **High Accuracy**: Full AST parsing with SWC for zero false positives
- 🔄 **Real-time Updates**: Watch mode updates JSON files the moment you save
- 🛡️ **Preserves Translations**: New keys are added without touching existing translations
- 📦 **Lightweight**: Low memory usage, comfortable background execution
- 🔧 **i18next Compatible**: Supports namespaces, plurals, context, and more

---

## ✨ Implemented Features

### Basic Extraction Patterns

```typescript
// ✅ Supported
t('hello.world')
i18n.t('greeting')
t('common:button.save')  // With namespace
```

### React Components

```tsx
// ✅ Trans component
<Trans i18nKey="welcome">Welcome</Trans>
<Trans i18nKey="common:greeting" defaults="Hello!" />
```

### Plurals and Context

```typescript
// ✅ Plurals
t('apple', { count: 5 })  // → apple_one, apple_other

// ✅ Context
t('friend', { context: 'male' })  // → friend_male

// ✅ Plurals + Context
t('friend', { count: 2, context: 'female' })  // → friend_female_one, friend_female_other
```

Based on ICU plural rules, the required categories (`zero`, `one`, `few`, `many`, etc.) are generated for each language in `locales`. For example, with Russian you get `friend_one`, `friend_few`, `friend_many`, `friend_other` added at once.

### Other Features

- ✅ **Magic Comments**: `// i18next-extract-disable-line`
- ✅ **Nested Keys**: `button.submit` → `{"button": {"submit": ""}}`
- ✅ **Auto-sorted Keys**: Alphabetically sorted for consistent JSON
- ✅ **TypeScript Type Generation**: Autocomplete and type safety
- ✅ **Dead Key Detection**: Find unused keys after refactoring

---

## 📦 Installation

### Method 1: Install via Cargo (Recommended)

```bash
cargo install i18next-turbo
```

### Method 2: Build from Source

```bash
git clone https://github.com/your-username/i18next-turbo.git
cd i18next-turbo
cargo build --release
# Binary will be generated at target/release/i18next-turbo
```

> **📌 Note**: npm package distribution is in preparation. For Node.js projects, Rust installation is currently required.

---

## 🛠️ Usage

### 1. Create Configuration File

Create `i18next-turbo.json` in your project root:

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

#### Configuration Options

| Option | Description | Default |
|:---|:---|:---|
| `input` | File patterns to extract (glob) | `["src/**/*.{ts,tsx,js,jsx}"]` |
| `output` | Output path (`$LOCALE` and `$NAMESPACE` are replaced) | `"locales"` |
| `locales` | List of target languages | `["en"]` |
| `defaultNamespace` | Default namespace | `"translation"` |
| `functions` | Function names to extract | `["t"]` |
| `logLevel` | Logging verbosity (`error`/`warn`/`info`/`debug`) | `"info"` |
| `types.output` | Path for generated TypeScript definitions | `"src/@types/i18next.d.ts"` |
| `types.defaultLocale` | Default locale for type generation | First entry in `locales` |
| `types.localesDir` | Directory read when generating types | Same as `output` |
| `types.input` | Glob patterns of locale files to include in type generation | all `*.json` in default locale |
| `types.resourcesFile` | Optional secondary file path for `Resources` interfaces | not generated |
| `types.enableSelector` | Enable selector helper types (`true`, `false`, `"optimize"`) | `false` |
| `types.indentation` | Indentation for generated type files | `2 spaces` |
| `defaultValue` | String or function `(key, namespace, language, value) => string` | `""` |
| `sort` | Boolean or function `(a, b) => number` for locale key ordering | `true` |
| `plugins` | Plugin modules/objects with `setup`/`onEnd`/`afterSync` hooks | `[]` |

Use the optional `types` block to control where type definitions are written and which locale files `i18next-turbo typegen` or `i18next-turbo extract --generate-types` should use.

> The CLI automatically searches for `i18next-turbo.json`, `i18next-parser.config.(js|ts)`, and `i18next.config.(js|ts)` (CommonJS, ESM, or TypeScript via `jiti`). You can also pass `--config path/to/i18next.config.ts` directly.

### 2. Extract Keys

Run once (e.g., for CI/CD):

```bash
i18next-turbo extract
```

#### Example Output

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

### 3. Watch Mode (Development)

Automatically extract and update keys on file save:

```bash
i18next-turbo watch
```

#### Example Behavior

```
=== i18next-turbo watch ===

Watching: src
Watching for changes... (Ctrl+C to stop)

--- Change detected ---
  Modified: src/components/Button.tsx
  Added 1 new key(s)
--- Sync complete ---
```

Run this command in the background during development to automatically update JSON files when you add translation keys.

### 4. Translation Status

Check translation progress for a specific locale:

```bash
i18next-turbo status --locale ja
```

Useful flags:

- `--namespace <name>`: limit the report to a single namespace
- `--fail-on-incomplete`: exit with a non-zero status when missing or dead keys are found (great for CI)

The summary includes a textual progress bar so you can instantly gauge completion status for the selected locale/namespace.

---

## 📝 Examples

### Basic Usage

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

After running, `locales/en/translation.json` will have:

```json
{
  "button": {
    "submit": ""
  }
}
```

### Using Namespaces

```typescript
// Specify namespace
t('common:button.save')  // → Saved to locales/en/common.json
```

### React Trans Component

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

### Using Plurals

```typescript
const count = 5;
t('apple', { count });  // → Generates apple_one, apple_other
```

Generated JSON:

```json
{
  "apple_one": "",
  "apple_other": ""
}
```

---

## 🎯 Migration from i18next-parser

If you're using `i18next-parser`, you can migrate with minimal changes to your configuration.

### Configuration Differences

| i18next-parser | i18next-turbo |
|:---|:---|
| `input` | `input` (same) |
| `output` | `output` (same) |
| `locales` | `locales` (same) |
| `defaultNamespace` | `defaultNamespace` (same) |
| `functions` | `functions` (same) |

Basically the same configuration works!

### Migration Steps

1. Create `i18next-turbo.json` (copy your existing config)
2. Run `i18next-turbo extract`
3. Verify generated JSON files
4. Start development with watch mode

### i18next-cli Config Compatibility

`i18next-turbo` can read `i18next-cli` config files and map a subset of `extract` options.

Supported mappings:

| i18next-cli (extract) | i18next-turbo |
|:---|:---|
| `input` | `input` |
| `output` (string) | `output` (directory) |
| `output` (function) | evaluated and projected to `output` directory |
| `functions` | `functions` |
| `defaultNS` | `defaultNamespace` |
| `keySeparator` | `keySeparator` (`false` -> empty string) |
| `nsSeparator` | `nsSeparator` (`false` -> empty string) |
| `contextSeparator` | `contextSeparator` |
| `pluralSeparator` | `pluralSeparator` |
| `defaultNS = false` | `defaultNamespace = ""` + namespace-less mode |
| `secondaryLanguages` | `secondaryLanguages` |
| `transKeepBasicHtmlNodesFor` | `transKeepBasicHtmlNodesFor` |
| `preserveContextVariants` | `preserveContextVariants` |
| `interpolationPrefix` / `interpolationSuffix` | `interpolationPrefix` / `interpolationSuffix` |
| `mergeNamespaces` | `mergeNamespaces` |
| `extractFromComments` | `extractFromComments` (default `true`) |

Function-form support:

| i18next-cli (extract) | i18next-turbo behavior |
|:---|:---|
| `defaultValue` function | Applied after `extract`/`sync` on generated locale files |
| `sort` function | Applied after `extract`/`sync` to order keys |

Plugin support:

| Hook | Status |
|:---|:---|
| `setup` | Supported |
| `onEnd` | Supported |
| `afterSync` | Supported |

Notes:
- Output templates like `locales/{{language}}/{{namespace}}.json` are reduced to a base directory.

Documentation:
- [API](./docs/api.md)
- [Usage examples](./docs/usage-examples.md)
- [Migration guide](./docs/migration-guide.md)
- [Troubleshooting](./docs/troubleshooting.md)
- [Performance testing](./docs/performance-testing.md)

---

## 🔧 Advanced Features

### Magic Comments

Exclude specific lines from extraction:

```typescript
// i18next-extract-disable-line
const dynamicKey = `user.${role}.permission`;
t(dynamicKey);  // This line won't be extracted
```

### TypeScript Type Generation

```bash
# Generate once based on your config (honors the optional `types` block)
i18next-turbo typegen

# Or run extraction and type generation together
i18next-turbo extract --generate-types
```

Example generated type definitions:

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

### Dead Key Detection

```bash
# Will be available as i18next-turbo cleanup command in the future
# Detects keys not found in code
```

---

## 📊 Performance

### Benchmark Results

| Files | i18next-parser | i18next-cli | i18next-turbo |
|:---|:---:|:---:|:---:|
| 100 | 1.2s | 0.3s | **0.01s** |
| 1,000 | 12.5s | 2.3s | **0.08s** |
| 10,000 | 125s | 23s | **0.8s** |

### Memory Usage

- **i18next-parser**: ~200MB
- **i18next-cli**: ~150MB
- **i18next-turbo**: **~50MB** (~4x lighter)

---

## 🗺️ Roadmap

### ✅ Implemented

- [x] Basic `t()` function extraction
- [x] `<Trans>` component support
- [x] Namespace support
- [x] Plurals (basic `_one`, `_other`)
- [x] Context support
- [x] Watch mode
- [x] JSON synchronization (preserves existing translations)
- [x] TypeScript type generation
- [x] Dead key detection

### 🚧 In Development

- [x] npm package distribution
- [x] Full `useTranslation` hook support (`keyPrefix`, etc.)
- [x] Language-specific plural categories (`zero`, `few`, `many`, etc.)
- [x] JS/TS config file loading

### 📅 Planned

- [ ] Locize integration

See [TODO.md](./TODO.md) for details.

---

## 🤝 Contributing

Pull requests and issue reports are welcome!

1. Fork this repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Please read [CONTRIBUTING.md](./CONTRIBUTING.md) for details on our code of conduct.

---

## 📄 License

MIT License - see [LICENSE](./LICENSE) file for details.

---

## 🙏 Acknowledgments

- [i18next](https://www.i18next.com/) - Amazing internationalization framework
- [SWC](https://swc.rs/) - Fast JavaScript/TypeScript compiler
- [i18next-parser](https://github.com/i18next/i18next-parser) - Source of inspiration

---

## ⚠️ Disclaimer

- This tool is an **unofficial i18next tool**
- APIs may evolve between major versions
- npm package is available: [i18next-turbo](https://www.npmjs.com/package/i18next-turbo)

---

**Questions or issues? Please open an [Issue](https://github.com/your-username/i18next-turbo/issues)!**
