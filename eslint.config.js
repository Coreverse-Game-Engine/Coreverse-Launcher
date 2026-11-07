import js from "@eslint/js";
import { defineConfig, globalIgnores } from "eslint/config";
import prettier from "eslint-config-prettier";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import globals from "globals";
import tseslint from "typescript-eslint";

export default defineConfig([
    globalIgnores(["dist", "target", "src-tauri/target", "src-tauri/gen"]),
    {
        files: ["**/*.{ts,tsx}"],
        extends: [js.configs.recommended, tseslint.configs.recommended],
        languageOptions: {
            ecmaVersion: 2023,
            globals: globals.browser,
        },
        plugins: {
            "react-hooks": reactHooks,
            "react-refresh": reactRefresh,
        },
        rules: {
            "react-hooks/rules-of-hooks": "error",
            "react-hooks/exhaustive-deps": "warn",
            "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
        },
    },
    {
        // shadcn/ui files export helpers (e.g. buttonVariants) next to components.
        files: ["src/components/ui/**/*.{ts,tsx}"],
        rules: { "react-refresh/only-export-components": "off" },
    },
    {
        files: ["vite.config.ts", "vitest.config.ts"],
        languageOptions: { globals: globals.node },
    },
    prettier,
]);
