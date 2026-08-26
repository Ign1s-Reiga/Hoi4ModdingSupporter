import { defineConfig } from 'oxlint';

export default defineConfig({
  ignorePatterns: ['out/**', '.next/**', 'src-tauri/**'],
  plugins: ['eslint', 'typescript', 'node', 'react', 'nextjs', 'import', 'promise', 'oxc'],
  rules: {
    'eslint/no-prototype-builtins': 'off',
    'eslint/no-unused-expressions': ['error', { allowTernary: true }],
    'eslint/no-unused-vars': [
      'warn',
      {
        argsIgnorePattern: '^_',
        destructuredArrayIgnorePattern: '^_',
      },
    ],
    'typescript/no-explicit-any': 'error',
    'import/no-absolute-path': 'warn',
    'react/jsx-curly-brace-presence': ['warn', { propElementValues: 'always' }],
  },
});
