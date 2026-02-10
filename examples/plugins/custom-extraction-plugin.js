// Example plugin: custom extractor pattern __t('key').
module.exports = {
  onLoad({ source }) {
    return source.replace(/__t\(['\"]([^'\"]+)['\"]\)/g, "t('$1')");
  },
  onVisitNode(node) {
    if (node && node.type === 'TranslationKey' && node.key && node.key.startsWith('debug.')) {
      console.warn(`[plugin] debug key detected: ${node.key}`);
    }
  }
};
