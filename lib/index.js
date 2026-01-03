/**
 * i18next-turbo Node.js API
 * 
 * This module provides a programmatic API for using i18next-turbo from Node.js.
 * 
 * Currently, this is a placeholder. In the future, this will export functions
 * that call the NAPI .node addon.
 */

// Placeholder for future NAPI implementation
module.exports = {
  extract: async function(config) {
    throw new Error('Node.js API not yet implemented. Use CLI instead: npx i18next-turbo extract');
  },
  watch: async function(config) {
    throw new Error('Node.js API not yet implemented. Use CLI instead: npx i18next-turbo watch');
  }
};

