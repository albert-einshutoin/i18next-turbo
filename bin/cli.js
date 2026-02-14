#!/usr/bin/env node

/**
 * i18next-turbo CLI wrapper
 * 
 * This script detects the user's OS and architecture, then calls the appropriate
 * Rust binary. It also handles loading JS/TS configuration files and converting
 * them to JSON for the Rust binary.
 */

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');
const { pathToFileURL } = require('url');

let cachedCosmiconfig = null;

function loadCosmiconfig() {
  if (cachedCosmiconfig) {
    return cachedCosmiconfig;
  }
  try {
    cachedCosmiconfig = require('cosmiconfig');
    return cachedCosmiconfig;
  } catch (error) {
    throw new Error('Config auto-discovery requires "cosmiconfig". Install dependencies with: npm install');
  }
}

const JS_TS_CONFIG_EXTENSIONS = new Set(['.js', '.cjs', '.mjs', '.ts']);

// Detect platform and architecture
const platform = os.platform();
const arch = os.arch();

// Map Node.js platform names to Rust target triplets
const platformMap = {
  'darwin': 'darwin',
  'linux': 'linux',
  'win32': 'win32'
};

// Map Node.js architecture names to Rust architecture names
const archMap = {
  'x64': 'x64',
  'arm64': 'arm64',
  'ia32': 'ia32'
};

// Get the Rust target name
const rustPlatform = platformMap[platform];
const rustArch = archMap[arch];

if (!rustPlatform || !rustArch) {
  console.error(`Error: Unsupported platform/architecture: ${platform}/${arch}`);
  console.error('Supported platforms: darwin, linux, win32');
  console.error('Supported architectures: x64, arm64, ia32');
  process.exit(1);
}

// Determine binary name and path
const binaryName = platform === 'win32' ? 'i18next-turbo.exe' : 'i18next-turbo';
const binaryPath = resolveBinaryPath(platform, arch, binaryName);

// Check if binary exists
if (!binaryPath || !fs.existsSync(binaryPath)) {
  console.error('Error: Suitable binary not found for this platform.');
  if (binaryPath) {
    console.error(`Checked: ${binaryPath}`);
  }
  console.error('Please install the optional binary package for your platform,');
  console.error('or build the project first: cargo build --release');
  process.exit(1);
}

async function main() {
  const rawArgs = process.argv.slice(2);
  const { args, inlineConfigJson, inlineRuntimeConfig, configProvided, configPathHint: inlineConfigPath } = await resolveConfigArgs(rawArgs);

  let configJson = inlineConfigJson;
  let runtimeConfig = inlineRuntimeConfig;
  let configPathHint = inlineConfigPath;

  if (!configJson && !configProvided) {
    try {
      const result = await loadConfigFromDisk();
      if (result) {
        const normalized = normalizeConfig(result.config);
        if (normalized) {
          configJson = serializeConfigForRust(normalized);
          runtimeConfig = normalized;
          configPathHint = result.filepath;
        }
      }
    } catch (error) {
      console.warn(`Warning: Failed to load config file: ${error.message}`);
    }
  }

  const rustArgs = [];
  if (configJson) {
    rustArgs.push('--config-stdin');
    if (configPathHint) {
      rustArgs.push('--config-path-hint', configPathHint);
    }
  }
  rustArgs.push(...args);

  const commandName = resolveCommandName(args);
  const features = resolveRuntimeFeatures(runtimeConfig, commandName, args);
  const plugins = await loadPlugins(runtimeConfig);
  await runPluginHook(plugins, 'setup', {
    command: commandName,
    args,
    config: runtimeConfig
  });

  const onLoadPrep = await prepareOnLoadInput(plugins, runtimeConfig, commandName, args);
  if (onLoadPrep && onLoadPrep.updatedConfig) {
    runtimeConfig = onLoadPrep.updatedConfig;
    configJson = serializeConfigForRust(runtimeConfig);
  }

  const astCapture = prepareAstEventCapture(plugins, commandName);
  let exitCode = 1;
  try {
    exitCode = await runRustBinary(binaryPath, rustArgs, configJson, astCapture.env);
  } finally {
    if (onLoadPrep && typeof onLoadPrep.cleanup === 'function') {
      onLoadPrep.cleanup();
    }
    await dispatchAstVisitEvents(plugins, astCapture, {
      command: commandName,
      args,
      config: runtimeConfig
    });
  }
  if (exitCode !== 0) {
    process.exit(exitCode);
  }

  const postSyncResult = await applyRuntimeTransforms(features);
  if (commandName === 'sync') {
    await emitOnVisitNodeEquivalent(plugins, runtimeConfig, commandName, args);
  }
  await runPluginHook(plugins, 'onEnd', {
    command: commandName,
    args,
    config: runtimeConfig,
    postSyncResult
  });
  await runPluginHook(plugins, 'afterSync', {
    command: commandName,
    args,
    config: runtimeConfig,
    postSyncResult
  });

  process.exit(0);
}

main().catch((error) => {
  console.error(`Error: ${error.message}`);
  process.exit(1);
});

async function resolveConfigArgs(argv) {
  const processedArgs = [];
  let inlineConfigJson = null;
  let inlineRuntimeConfig = null;
  let configProvided = false;
  let configPathHint = null;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];

    if (arg === '--config' || arg === '-c') {
      configProvided = true;
      const value = argv[i + 1];
      if (!value) {
        processedArgs.push(arg);
        continue;
      }

      if (shouldLoadWithNode(value)) {
        const result = await loadConfigFromFile(value);
        if (result) {
          const normalized = normalizeConfig(result.config);
          if (normalized) {
            inlineConfigJson = serializeConfigForRust(normalized);
            inlineRuntimeConfig = normalized;
            configPathHint = result.filepath;
          }
        }
        i += 1;
        continue;
      }

      processedArgs.push(arg, value);
      i += 1;
      continue;
    }

    if (arg.startsWith('--config=')) {
      configProvided = true;
      const value = arg.split('=')[1];
      if (shouldLoadWithNode(value)) {
        const result = await loadConfigFromFile(value);
        if (result) {
          const normalized = normalizeConfig(result.config);
          if (normalized) {
            inlineConfigJson = serializeConfigForRust(normalized);
            inlineRuntimeConfig = normalized;
            configPathHint = result.filepath;
          }
        }
        continue;
      }
    }

    processedArgs.push(arg);
  }

  return { args: processedArgs, inlineConfigJson, inlineRuntimeConfig, configProvided, configPathHint };
}

/**
 * Find and load configuration file in current directory
 * Priority: i18next-turbo.json > i18next-parser.config.(js|json) > i18next.config.(ts|js)
 */
async function loadConfigFromDisk() {
  const explorer = createExplorer();
  const result = await explorer.search();
  return formatExplorerResult(result);
}

async function loadConfigFromFile(filepath) {
  const explorer = createExplorer();
  const absolute = path.resolve(process.cwd(), filepath);
  const result = await explorer.load(absolute);
  return formatExplorerResult(result);
}

function createExplorer() {
  const { cosmiconfig, defaultLoaders } = loadCosmiconfig();
  return cosmiconfig('i18next-turbo', {
    searchPlaces: [
      'i18next-turbo.json',
      'i18next-parser.config.json',
      'i18next-parser.config.js',
      'i18next-parser.config.cjs',
      'i18next-parser.config.mjs',
      'i18next-parser.config.ts',
      'i18next.config.ts',
      'i18next.config.js',
      'i18next.config.cjs',
      'i18next.config.mjs'
    ],
    loaders: {
      '.js': defaultLoaders['.js'],
      '.json': defaultLoaders['.json'],
      '.cjs': loadCommonJsConfig,
      '.mjs': loadEsmConfig,
      '.ts': loadTypeScriptConfig
    }
  });
}

function formatExplorerResult(result) {
  if (!result) {
    return null;
  }
  const filepath = result.filepath ? path.resolve(result.filepath) : null;
  const config = result.config && result.config.default ? result.config.default : result.config;
  return { config, filepath };
}

function shouldLoadWithNode(filepath) {
  const ext = path.extname(filepath).toLowerCase();
  return JS_TS_CONFIG_EXTENSIONS.has(ext);
}

function loadTypeScriptConfig(filepath) {
  let jiti;
  try {
    jiti = require('jiti')(process.cwd(), {
      esmResolve: true,
      interopDefault: true
    });
  } catch (error) {
    throw new Error(
      'TypeScript config files require "jiti" package. Install it with: npm install --save-dev jiti'
    );
  }

  const config = jiti(filepath);
  return config && config.default ? config.default : config;
}

function loadCommonJsConfig(filepath) {
  delete require.cache[require.resolve(filepath)];
  const config = require(filepath);
  return config && config.default ? config.default : config;
}

async function loadEsmConfig(filepath) {
  const configUrl = pathToFileURL(filepath).href;
  const configModule = await import(`${configUrl}?t=${Date.now()}`);
  return configModule && configModule.default ? configModule.default : configModule;
}

function normalizeConfig(rawConfig) {
  if (!rawConfig || typeof rawConfig !== 'object') {
    return null;
  }

  const runtime = buildRuntimeMetadata(rawConfig);
  const mapped = mapCliExtractConfig(rawConfig);
  const normalized = {
    ...mapped,
    ...rawConfig
  };

  normalized.__runtime = runtime;
  delete normalized.extract;
  return normalized;
}

function serializeConfigForRust(config) {
  if (!config || typeof config !== 'object') {
    return null;
  }
  const clone = { ...config };
  delete clone.__runtime;
  return JSON.stringify(clone);
}

function buildRuntimeMetadata(rawConfig) {
  const extract = rawConfig.extract && typeof rawConfig.extract === 'object' ? rawConfig.extract : {};
  return {
    defaultValueFn: typeof extract.defaultValue === 'function'
      ? extract.defaultValue
      : (typeof rawConfig.defaultValue === 'function' ? rawConfig.defaultValue : null),
    sortFn: typeof extract.sort === 'function'
      ? extract.sort
      : (typeof rawConfig.sort === 'function' ? rawConfig.sort : null),
    plugins: Array.isArray(rawConfig.plugins)
      ? rawConfig.plugins
      : (Array.isArray(extract.plugins) ? extract.plugins : [])
  };
}

function resolveCommandName(args) {
  for (const arg of args) {
    if (!arg.startsWith('-')) {
      return arg;
    }
  }
  return 'extract';
}

function resolveRuntimeFeatures(config, commandName, args) {
  const runtime = config && config.__runtime ? config.__runtime : {};
  const defaultValueFn = typeof runtime.defaultValueFn === 'function' ? runtime.defaultValueFn : null;
  const sortFn = typeof runtime.sortFn === 'function' ? runtime.sortFn : null;

  const dryRun = args.includes('--dry-run');
  const supportedCommand = commandName === 'extract' || commandName === 'sync';

  return {
    config,
    defaultValueFn,
    sortFn,
    enabled: supportedCommand && !dryRun && (!!defaultValueFn || !!sortFn)
  };
}

async function runRustBinary(binaryPath, rustArgs, configJson, extraEnv = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(binaryPath, rustArgs, {
      stdio: configJson ? ['pipe', 'inherit', 'inherit'] : 'inherit',
      cwd: process.cwd(),
      env: { ...process.env, ...extraEnv }
    });

    if (configJson) {
      child.stdin.write(configJson);
      child.stdin.end();
    }

    child.on('error', (error) => {
      reject(new Error(`Failed to start i18next-turbo: ${error.message}`));
    });

    child.on('exit', (code) => {
      resolve(code || 0);
    });
  });
}

function prepareAstEventCapture(plugins, commandName) {
  const hasOnVisit = plugins.some((plugin) => plugin && typeof plugin.onVisitNode === 'function');
  const supportsAstEvents = ['extract', 'watch', 'status', 'check', 'lint'].includes(commandName);
  if (!hasOnVisit || !supportsAstEvents) {
    return { env: {}, filePath: null, cleanup: null };
  }

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'i18next-turbo-ast-events-'));
  const filePath = path.join(tmpDir, 'events.jsonl');
  fs.writeFileSync(filePath, '');
  return {
    env: { I18NEXT_TURBO_AST_EVENTS_PATH: filePath },
    filePath,
    cleanup: () => {
      try {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      } catch {
        // best effort cleanup
      }
    }
  };
}

async function dispatchAstVisitEvents(plugins, capture, context) {
  if (!capture || !capture.filePath) {
    return;
  }
  try {
    const content = fs.readFileSync(capture.filePath, 'utf8');
    const lines = content.split(/\r?\n/).filter(Boolean);
    for (const line of lines) {
      try {
        const parsed = JSON.parse(line);
        const node = parsed && parsed.type === 'AstNodeVisit'
          ? {
            ...parsed,
            eventType: parsed.type,
            type: parsed.nodeType
          }
          : parsed;
        await runPluginVisitNode(plugins, {
          ...node,
          command: context.command,
          args: context.args
        });
      } catch (error) {
        console.warn(`Warning: failed to parse AST event line: ${error.message}`);
      }
    }
  } finally {
    if (typeof capture.cleanup === 'function') {
      capture.cleanup();
    }
  }
}

function shouldApplyOnLoad(commandName, args) {
  if (args.includes('--dry-run')) {
    return false;
  }
  return ['extract', 'watch', 'status', 'check', 'lint'].includes(commandName);
}

async function prepareOnLoadInput(plugins, runtimeConfig, commandName, args) {
  const hasOnLoad = plugins.some((plugin) => plugin && typeof plugin.onLoad === 'function');
  if (!hasOnLoad || !runtimeConfig || !shouldApplyOnLoad(commandName, args)) {
    return null;
  }

  const inputPatterns = Array.isArray(runtimeConfig.input) ? runtimeConfig.input : [];
  if (inputPatterns.length === 0) {
    return null;
  }

  const ignorePatterns = Array.isArray(runtimeConfig.ignore) ? runtimeConfig.ignore : [];
  const cwd = process.cwd();
  const files = collectMatchingSourceFiles(cwd, inputPatterns, ignorePatterns);
  if (files.length === 0) {
    return null;
  }

  const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'i18next-turbo-onload-'));
  const transformedInputs = [];

  for (const absFile of files) {
    const rel = toPosix(path.relative(cwd, absFile));
    let source = fs.readFileSync(absFile, 'utf8');
    for (const plugin of plugins) {
      if (!plugin || typeof plugin.onLoad !== 'function') {
        continue;
      }
      try {
        const result = await plugin.onLoad({
          command: commandName,
          args,
          config: runtimeConfig,
          filePath: absFile,
          relativePath: rel,
          source
        });
        if (typeof result === 'string') {
          source = result;
        } else if (result && typeof result === 'object') {
          if (typeof result.code === 'string') {
            source = result.code;
          } else if (typeof result.source === 'string') {
            source = result.source;
          }
        }
      } catch (error) {
        console.warn(`Warning: plugin hook "onLoad" failed for ${rel}: ${error.message}`);
      }
    }

    const tmpFile = path.join(tmpRoot, rel);
    fs.mkdirSync(path.dirname(tmpFile), { recursive: true });
    fs.writeFileSync(tmpFile, source);
    transformedInputs.push(tmpFile);
  }

  const updatedConfig = {
    ...runtimeConfig,
    input: transformedInputs,
    ignore: []
  };

  return {
    updatedConfig,
    cleanup: () => {
      try {
        fs.rmSync(tmpRoot, { recursive: true, force: true });
      } catch {
        // best effort cleanup
      }
    }
  };
}

function collectMatchingSourceFiles(cwd, inputPatterns, ignorePatterns) {
  const normalizedInput = inputPatterns.map((p) => toPosix(String(p)));
  const normalizedIgnore = ignorePatterns.map((p) => toPosix(String(p)));
  const roots = [...new Set(normalizedInput.map(guessGlobRoot).filter(Boolean))];
  const searchRoots = roots.length > 0 ? roots : ['.'];
  const files = [];

  for (const root of searchRoots) {
    const absRoot = path.resolve(cwd, root);
    if (!fs.existsSync(absRoot)) {
      continue;
    }
    walkFiles(absRoot, (absPath) => {
      const rel = toPosix(path.relative(cwd, absPath));
      if (normalizedInput.some((pattern) => globMatch(rel, pattern))
        && !normalizedIgnore.some((pattern) => globMatch(rel, pattern))) {
        files.push(absPath);
      }
    });
  }

  return files;
}

function walkFiles(dir, onFile) {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name.startsWith('.git')) {
        continue;
      }
      walkFiles(fullPath, onFile);
    } else if (entry.isFile()) {
      onFile(fullPath);
    }
  }
}

function guessGlobRoot(pattern) {
  const wildcardIndex = pattern.search(/[*?[{]/);
  if (wildcardIndex === -1) {
    return path.dirname(pattern);
  }
  const slice = pattern.slice(0, wildcardIndex);
  return slice.endsWith('/') ? slice.slice(0, -1) : (slice || '.');
}

function toPosix(p) {
  return p.split(path.sep).join('/');
}

function globMatch(value, pattern) {
  const escaped = pattern
    .replace(/[.+^${}()|[\]\\]/g, '\\$&')
    .replace(/\\\{([^}]+)\\\}/g, (_, inner) => `(${inner.split(',').map((s) => s.trim()).join('|')})`)
    // Allow zero or more directories for `**/` patterns.
    .replace(/\*\*\//g, ':::DOUBLE_STAR_SLASH:::')
    .replace(/\*\*/g, ':::DOUBLE_STAR:::')
    .replace(/\*/g, '[^/]*')
    .replace(/\?/g, '.')
    .replace(/:::DOUBLE_STAR_SLASH:::/g, '(?:.*/)?')
    .replace(/:::DOUBLE_STAR:::/g, '.*');
  const re = new RegExp(`^${escaped}$`);
  return re.test(value);
}

function resolveOutputFormat(config) {
  const format = typeof config.outputFormat === 'string' ? config.outputFormat : 'json';
  if (format === 'json') return 'json';
  if (format === 'json5') return 'json5';
  return null;
}

function resolveIndent(config) {
  const indentation = config && config.indentation;
  if (typeof indentation === 'number' && indentation > 0) {
    return ' '.repeat(indentation);
  }
  if (typeof indentation === 'string' && indentation.length > 0) {
    return indentation;
  }
  return '  ';
}

async function applyRuntimeTransforms(features) {
  if (!features.enabled) {
    return { applied: false, files: [] };
  }

  const { config, defaultValueFn, sortFn } = features;
  const outputFormat = resolveOutputFormat(config || {});
  if (!outputFormat) {
    console.warn('Warning: function form defaultValue/sort is supported only for json/json5 outputs.');
    return { applied: false, files: [] };
  }

  if (outputFormat === 'json5') {
    console.warn('Warning: function form defaultValue/sort is currently skipped for json5 output.');
    return { applied: false, files: [] };
  }

  const outputDir = path.resolve(process.cwd(), config.output || 'locales');
  const locales = Array.isArray(config.locales) && config.locales.length > 0 ? config.locales : ['en'];
  const ext = outputFormat;
  const indent = resolveIndent(config);
  const touched = [];

  for (const locale of locales) {
    const localeDir = path.join(outputDir, locale);
    if (!fs.existsSync(localeDir) || !fs.statSync(localeDir).isDirectory()) {
      continue;
    }
    const entries = fs.readdirSync(localeDir, { withFileTypes: true });
    for (const entry of entries) {
      if (!entry.isFile()) continue;
      if (path.extname(entry.name).toLowerCase() !== `.${ext}`) continue;

      const filePath = path.join(localeDir, entry.name);
      const namespace = path.basename(entry.name, `.${ext}`);
      const content = fs.readFileSync(filePath, 'utf8');
      let parsed;
      try {
        parsed = JSON.parse(content);
      } catch {
        console.warn(`Warning: skipping transform for non-JSON file: ${filePath}`);
        continue;
      }

      let changed = false;
      if (defaultValueFn) {
        changed = applyDefaultValueFunction(parsed, defaultValueFn, namespace, locale, '') || changed;
      }
      if (sortFn) {
        parsed = sortObjectWithComparator(parsed, sortFn, namespace, locale, '');
        changed = true || changed;
      }

      if (changed) {
        fs.writeFileSync(filePath, `${JSON.stringify(parsed, null, indent)}\n`);
        touched.push(filePath);
      }
    }
  }

  if (touched.length > 0) {
    console.log(`Applied runtime defaultValue/sort transforms to ${touched.length} locale file(s).`);
  }
  return { applied: touched.length > 0, files: touched };
}

async function emitOnVisitNodeEquivalent(plugins, config, commandName, args) {
  const hasVisitor = plugins.some((plugin) => plugin && typeof plugin.onVisitNode === 'function');
  if (!hasVisitor || !config) {
    return;
  }
  if (!['extract', 'sync'].includes(commandName)) {
    return;
  }

  const outputFormat = resolveOutputFormat(config);
  if (outputFormat !== 'json') {
    return;
  }

  const outputDir = path.resolve(process.cwd(), config.output || 'locales');
  const locales = Array.isArray(config.locales) && config.locales.length > 0 ? config.locales : ['en'];
  for (const locale of locales) {
    const localeDir = path.join(outputDir, locale);
    if (!fs.existsSync(localeDir) || !fs.statSync(localeDir).isDirectory()) {
      continue;
    }
    const entries = fs.readdirSync(localeDir, { withFileTypes: true });
    for (const entry of entries) {
      if (!entry.isFile()) continue;
      if (path.extname(entry.name).toLowerCase() !== '.json') continue;
      const filePath = path.join(localeDir, entry.name);
      const namespace = path.basename(entry.name, '.json');
      const content = fs.readFileSync(filePath, 'utf8');
      let parsed;
      try {
        parsed = JSON.parse(content);
      } catch {
        continue;
      }
      await traverseLocaleNodes(parsed, {
        filePath,
        namespace,
        language: locale,
        command: commandName,
        args,
        config
      }, plugins);
    }
  }
}

async function traverseLocaleNodes(value, base, plugins, parentPath = '') {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return;
  }

  for (const [k, v] of Object.entries(value)) {
    const keyPath = parentPath ? `${parentPath}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      await runPluginVisitNode(plugins, {
        ...base,
        type: 'TranslationObject',
        key: keyPath,
        value: v
      });
      await traverseLocaleNodes(v, base, plugins, keyPath);
    } else {
      await runPluginVisitNode(plugins, {
        ...base,
        type: 'TranslationKey',
        key: keyPath,
        value: v
      });
    }
  }
}

async function runPluginVisitNode(plugins, node) {
  for (const plugin of plugins) {
    if (!plugin || typeof plugin.onVisitNode !== 'function') {
      continue;
    }
    try {
      await plugin.onVisitNode(node);
    } catch (error) {
      console.warn(`Warning: plugin hook "onVisitNode" failed: ${error.message}`);
    }
  }
}

function applyDefaultValueFunction(value, fn, namespace, language, keyPath) {
  let changed = false;
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    for (const [k, v] of Object.entries(value)) {
      const nextPath = keyPath ? `${keyPath}.${k}` : k;
      if (typeof v === 'string') {
        if (v === '') {
          const computed = fn(nextPath, namespace, language, v);
          if (typeof computed === 'string' && computed !== v) {
            value[k] = computed;
            changed = true;
          }
        }
      } else if (v && typeof v === 'object' && !Array.isArray(v)) {
        changed = applyDefaultValueFunction(v, fn, namespace, language, nextPath) || changed;
      }
    }
  }
  return changed;
}

function sortObjectWithComparator(value, sortFn, namespace, language, keyPath) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return value;
  }

  const entries = Object.entries(value).map(([k, v]) => {
    const nextPath = keyPath ? `${keyPath}.${k}` : k;
    const sortedValue = sortObjectWithComparator(v, sortFn, namespace, language, nextPath);
    return [k, sortedValue, nextPath];
  });

  entries.sort((a, b) => {
    try {
      const result = sortFn(
        { key: a[2], namespace, language },
        { key: b[2], namespace, language }
      );
      if (typeof result === 'number' && Number.isFinite(result)) {
        return result;
      }
    } catch (error) {
      console.warn(`Warning: sort comparator threw error: ${error.message}`);
    }
    return a[0].localeCompare(b[0]);
  });

  const sorted = {};
  for (const [k, v] of entries) {
    sorted[k] = v;
  }
  return sorted;
}

async function loadPlugins(config) {
  const pluginDefs = (config && config.__runtime && Array.isArray(config.__runtime.plugins))
    ? config.__runtime.plugins
    : [];
  const plugins = [];

  for (const def of pluginDefs) {
    try {
      if (typeof def === 'string') {
        const mod = await loadPluginModule(def);
        plugins.push(mod);
        continue;
      }
      if (def && typeof def === 'object') {
        if (typeof def.resolve === 'string') {
          const mod = await loadPluginModule(def.resolve);
          if (typeof mod === 'function') {
            plugins.push(mod(def.options || {}));
          } else {
            plugins.push(mod);
          }
          continue;
        }
        plugins.push(def);
      }
    } catch (error) {
      console.warn(`Warning: failed to load plugin: ${error.message}`);
    }
  }

  return plugins.filter(Boolean);
}

async function loadPluginModule(specifier) {
  const resolvedPath = path.isAbsolute(specifier)
    ? specifier
    : path.resolve(process.cwd(), specifier);
  if (resolvedPath.endsWith('.mjs')) {
    const mod = await import(pathToFileURL(resolvedPath).href);
    return mod.default || mod;
  }
  delete require.cache[require.resolve(resolvedPath)];
  const mod = require(resolvedPath);
  return mod && mod.default ? mod.default : mod;
}

async function runPluginHook(plugins, hook, context) {
  for (const plugin of plugins) {
    if (!plugin || typeof plugin[hook] !== 'function') {
      continue;
    }
    try {
      await plugin[hook](context);
    } catch (error) {
      console.warn(`Warning: plugin hook "${hook}" failed: ${error.message}`);
    }
  }
}

function mapCliExtractConfig(rawConfig) {
  const extract = rawConfig.extract;
  if (!extract || typeof extract !== 'object') {
    return {};
  }

  const mapped = {};
  const outputPattern = typeof extract.output === 'string' ? extract.output : null;

  if (Array.isArray(rawConfig.locales)) {
    mapped.locales = rawConfig.locales;
  }

  if (Array.isArray(rawConfig.secondaryLanguages)) {
    mapped.secondaryLanguages = rawConfig.secondaryLanguages;
  }

  if (typeof extract.input === 'string') {
    mapped.input = [extract.input];
  } else if (Array.isArray(extract.input)) {
    mapped.input = extract.input;
  }

  if (typeof extract.output === 'string') {
    const outputDir = coerceOutputDir(extract.output);
    if (outputDir) {
      mapped.output = outputDir;
    }
  } else if (typeof extract.output === 'function') {
    const outputDir = resolveOutputDirFromFunction(extract.output, rawConfig, extract);
    if (outputDir) {
      mapped.output = outputDir;
    } else {
      console.warn('Warning: extract.output function could not be resolved to a directory.');
    }
  }

  if (Array.isArray(extract.functions)) {
    mapped.functions = extract.functions;
  }

  if (Array.isArray(extract.useTranslationNames)) {
    mapped.useTranslationNames = extract.useTranslationNames;
  }

  if (typeof extract.defaultNS === 'string') {
    mapped.defaultNamespace = extract.defaultNS;
  } else if (extract.defaultNS === false) {
    mapped.defaultNamespace = '';
    if (typeof extract.nsSeparator === 'undefined') {
      mapped.nsSeparator = '';
    }
  }

  if (typeof extract.keySeparator === 'string') {
    mapped.keySeparator = extract.keySeparator;
  } else if (extract.keySeparator === false || extract.keySeparator === null) {
    mapped.keySeparator = '';
  }

  if (typeof extract.nsSeparator === 'string') {
    mapped.nsSeparator = extract.nsSeparator;
  } else if (extract.nsSeparator === false || extract.nsSeparator === null) {
    mapped.nsSeparator = '';
  }

  if (typeof extract.contextSeparator === 'string') {
    mapped.contextSeparator = extract.contextSeparator;
  }

  if (Array.isArray(extract.secondaryLanguages)) {
    mapped.secondaryLanguages = extract.secondaryLanguages;
  }

  if (typeof extract.pluralSeparator === 'string') {
    mapped.pluralSeparator = extract.pluralSeparator;
  }

  if (Array.isArray(extract.transComponents)) {
    mapped.transComponents = extract.transComponents;
  }

  if (Array.isArray(extract.transKeepBasicHtmlNodesFor)) {
    mapped.transKeepBasicHtmlNodesFor = extract.transKeepBasicHtmlNodesFor;
  }

  if (Array.isArray(extract.ignore)) {
    mapped.ignore = extract.ignore;
  }

  if (Array.isArray(extract.preservePatterns)) {
    mapped.preservePatterns = extract.preservePatterns;
  }

  if (typeof extract.preserveContextVariants === 'boolean') {
    mapped.preserveContextVariants = extract.preserveContextVariants;
  }

  if (typeof extract.removeUnusedKeys === 'boolean') {
    mapped.removeUnusedKeys = extract.removeUnusedKeys;
  }

  if (typeof extract.mergeNamespaces === 'boolean') {
    mapped.mergeNamespaces = extract.mergeNamespaces;
  }

  if (mapped.mergeNamespaces && outputPattern) {
    const mergedFilename = resolveMergedNamespaceFilename(outputPattern);
    if (mergedFilename) {
      mapped.mergedNamespaceFilename = mergedFilename;
    }
  }

  if (typeof extract.defaultValue === 'string') {
    mapped.defaultValue = extract.defaultValue;
  }

  if (typeof extract.generateBasePluralForms === 'boolean') {
    mapped.generateBasePluralForms = extract.generateBasePluralForms;
  }

  if (typeof extract.disablePlurals === 'boolean') {
    mapped.disablePlurals = extract.disablePlurals;
  }

  if (typeof extract.nestingPrefix === 'string') {
    mapped.nestingPrefix = extract.nestingPrefix;
  }

  if (typeof extract.nestingSuffix === 'string') {
    mapped.nestingSuffix = extract.nestingSuffix;
  }

  if (typeof extract.nestingOptionsSeparator === 'string') {
    mapped.nestingOptionsSeparator = extract.nestingOptionsSeparator;
  }

  if (typeof extract.interpolationPrefix === 'string') {
    mapped.interpolationPrefix = extract.interpolationPrefix;
  }

  if (typeof extract.interpolationSuffix === 'string') {
    mapped.interpolationSuffix = extract.interpolationSuffix;
  }

  return mapped;
}

function coerceOutputDir(output) {
  if (!output) {
    return null;
  }

  const templateIndex = output.indexOf('{{');
  if (templateIndex !== -1) {
    const base = output.slice(0, templateIndex).replace(/[\\/]+$/, '');
    if (base) {
      return base;
    }
    console.warn('Warning: extract.output template could not be mapped to an output directory.');
    return null;
  }

  const ext = path.extname(output);
  if (ext) {
    return path.dirname(output);
  }

  return output;
}

function resolveMergedNamespaceFilename(outputPattern) {
  if (!outputPattern || outputPattern.includes('{{namespace}}')) {
    return null;
  }
  if (!outputPattern.includes('{{language}}') && !outputPattern.includes('$LOCALE')) {
    return null;
  }
  const normalized = outputPattern.replace(/\\/g, '/');
  const ext = path.extname(normalized);
  if (!ext) {
    return null;
  }
  const stem = path.basename(normalized, ext).trim();
  if (!stem || stem.includes('{{') || stem.includes('}}')) {
    return null;
  }
  return stem;
}

function resolveOutputDirFromFunction(outputFn, rawConfig, extractConfig) {
  const locales = Array.isArray(rawConfig.locales) && rawConfig.locales.length > 0
    ? rawConfig.locales
    : ['en', 'ja'];
  const defaultNs = typeof extractConfig.defaultNS === 'string' && extractConfig.defaultNS.length > 0
    ? extractConfig.defaultNS
    : 'translation';
  const namespaces = [defaultNs, 'common'];

  const candidates = [];
  for (const locale of locales.slice(0, 2)) {
    for (const namespace of namespaces) {
      try {
        const maybe = outputFn(locale, namespace);
        if (typeof maybe === 'string' && maybe.trim().length > 0) {
          const dir = coerceOutputDir(maybe);
          if (dir) {
            candidates.push(path.resolve(process.cwd(), dir));
          }
        }
      } catch (error) {
        console.warn(`Warning: extract.output function threw for (${locale}, ${namespace}): ${error.message}`);
      }
    }
  }

  if (candidates.length === 0) {
    return null;
  }

  const unique = [...new Set(candidates)];
  if (unique.length === 1) {
    return unique[0];
  }

  const common = longestCommonPathPrefix(unique);
  if (common) {
    return common;
  }

  return unique[0];
}

function longestCommonPathPrefix(paths) {
  if (!paths.length) {
    return null;
  }
  const split = paths.map(p => path.resolve(p).split(path.sep).filter(Boolean));
  const first = split[0];
  let i = 0;
  while (i < first.length && split.every(parts => parts[i] === first[i])) {
    i += 1;
  }
  if (i === 0) {
    return null;
  }
  const root = path.parse(path.resolve(paths[0])).root;
  return path.join(root, ...first.slice(0, i));
}

/**
 * Resolve the prebuilt binary path from optionalDependencies.
 * Falls back to local target/release for dev environments.
 */
function resolveBinaryPath(platformName, archName, binName) {
  // In Cargo test/integration contexts, Cargo can provide an up-to-date binary path.
  const cargoBin = process.env.CARGO_BIN_EXE_i18next_turbo;
  if (cargoBin && fs.existsSync(cargoBin)) {
    return cargoBin;
  }

  // Allow explicit binary override for advanced/CI scenarios.
  const explicitBin = process.env.I18NEXT_TURBO_BINARY;
  if (explicitBin && fs.existsSync(explicitBin)) {
    return explicitBin;
  }

  // Prefer local workspace builds in development/testing.
  const debugPath = path.join(__dirname, '..', 'target', 'debug', binName);
  if (fs.existsSync(debugPath)) {
    return debugPath;
  }

  const releasePath = path.join(__dirname, '..', 'target', 'release', binName);
  if (fs.existsSync(releasePath)) {
    return releasePath;
  }

  // Fallback to optional platform package in installed environments.
  const pkgNames = getBinaryPackageNames(platformName, archName);
  for (const pkgName of pkgNames) {
    try {
      const pkgJsonPath = require.resolve(`${pkgName}/package.json`);
      return path.join(path.dirname(pkgJsonPath), binName);
    } catch (error) {
      // Try next candidate package.
    }
  }

  return debugPath;
}

function getBinaryPackageNames(platformName, archName) {
  if (platformName === 'darwin' && archName === 'x64') {
    return ['i18next-turbo-darwin-x64'];
  }
  if (platformName === 'darwin' && archName === 'arm64') {
    return ['i18next-turbo-darwin-arm64'];
  }
  if (platformName === 'linux' && archName === 'x64') {
    return isMuslRuntime()
      ? ['i18next-turbo-linux-x64-musl', 'i18next-turbo-linux-x64']
      : ['i18next-turbo-linux-x64-gnu', 'i18next-turbo-linux-x64'];
  }
  if (platformName === 'win32' && archName === 'x64') {
    return ['i18next-turbo-win32-x64-msvc', 'i18next-turbo-win32-x64'];
  }
  if (platformName === 'win32' && archName === 'ia32') {
    return ['i18next-turbo-win32-ia32'];
  }
  return [];
}

function isMuslRuntime() {
  if (process.platform !== 'linux') {
    return false;
  }

  try {
    const report = process.report && typeof process.report.getReport === 'function'
      ? process.report.getReport()
      : null;
    const glibcVersion = report && report.header ? report.header.glibcVersionRuntime : null;
    if (glibcVersion) {
      return false;
    }
  } catch (error) {
    // Fall through to conservative default below.
  }

  return true;
}
