module.exports = {
  locales: ['en'],
  pluginErrorMode: 'continue',
  plugins: [{ resolve: './noop-plugin.cjs' }],
  extract: {
    input: ['src/**/*.ts', 'src/**/*.tsx'],
    output: 'locales/{{language}}/{{namespace}}.json',
    functions: ['t'],
  },
};
