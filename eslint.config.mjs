import nextTypescript from "eslint-config-next/typescript";
import nextPlugin from "@next/eslint-plugin-next";
import reactHooks from "eslint-plugin-react-hooks";

/*
  The rules are assembled from the individual plugins rather than through
  `eslint-config-next`'s base config. That config also enables
  eslint-plugin-import with its TypeScript resolver, whose native binding
  hangs on Node 26 — dropping it costs only the import-ordering rules, which
  nothing here relies on.
*/
const eslintConfig = [
  {
    ignores: ["out/**", ".next/**", "src-tauri/**", "node_modules/**"],
  },
  ...nextTypescript,
  reactHooks.configs.flat.recommended,
  nextPlugin.configs["core-web-vitals"],
  {
    rules: {
      // Deliberately unused bindings are prefixed with an underscore.
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
    },
  },
];

export default eslintConfig;
