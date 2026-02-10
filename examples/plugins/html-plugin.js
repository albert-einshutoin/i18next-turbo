// Example plugin: convert simple HTML translate() calls into t().
module.exports = {
  onLoad({ source, relativePath }) {
    if (!relativePath.endsWith('.html')) return source;
    return source.replace(/translate\(['\"]([^'\"]+)['\"]\)/g, "t('$1')");
  }
};
