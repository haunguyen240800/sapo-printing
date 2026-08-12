import sapo from "@sapo/eslint-plugin";
import { defineConfig, globalIgnores } from "eslint/config";
import reactRefresh from "eslint-plugin-react-refresh";
import unusedImports from "eslint-plugin-unused-imports";

export default defineConfig([
  globalIgnores(["dist", "src-tauri", "public", "node_modules"]),
  sapo.configs.typescript,
  sapo.configs.react,
  {
    files: ["src/**/*.{ts,tsx}"],
    plugins: {
      "react-refresh": reactRefresh,
      "unused-imports": unusedImports,
    },
    rules: {
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
      "@typescript-eslint/no-unused-vars": "off",
      "unused-imports/no-unused-imports": "error",
      "unused-imports/no-unused-vars": [
        "warn",
        { vars: "all", varsIgnorePattern: "^_", args: "after-used", argsIgnorePattern: "^_" },
      ],
    },
  },
  sapo.configs.prettier,
]);
