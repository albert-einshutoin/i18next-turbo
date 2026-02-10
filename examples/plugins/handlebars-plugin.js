// Example plugin: convert Handlebars helper calls into t().
module.exports = {
  onLoad({ source, relativePath }) {
    if (!relativePath.endsWith('.hbs') && !relativePath.endsWith('.handlebars')) {
      return source;
    }
    return source.replace(/\{\{\s*t\s+['\"]([^'\"]+)['\"]\s*\}\}/g, "t('$1')");
  }
};
