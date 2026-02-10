module.exports = {
  locales: ['en'],
  extract: {
    input: ['src/**/*.ts', 'src/**/*.tsx'],
    output: 'locales/{{language}}/{{namespace}}.json',
    functions: ['t'],
  },
};
