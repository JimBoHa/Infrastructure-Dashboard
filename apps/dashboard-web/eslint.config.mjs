import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

const eslintConfig = defineConfig([
  ...nextVitals,
  ...nextTs,
  // Override default ignores of eslint-config-next.
  globalIgnores([
    // Default ignores of eslint-config-next:
    ".next/**",
    "out/**",
    "build/**",
    "next-env.d.ts",
    // Vendored third-party bundles (minified, not our code):
    "public/vendor/**",
  ]),
  {
    rules: {
      // Allow _-prefixed variables to signal intentionally unused bindings
      // (e.g., destructuring rest, unused loop vars, omitted callback params).
      "@typescript-eslint/no-unused-vars": [
        "warn",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
          destructuredArrayIgnorePattern: "^_",
        },
      ],
    },
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "highcharts-react-official",
              message: "Use '@/components/charts/HighchartsPanel' instead of importing highcharts-react-official directly.",
            },
          ],
        },
      ],
    },
  },
  {
    files: ["src/components/charts/HighchartsPanel.tsx"],
    rules: {
      "no-restricted-imports": "off",
    },
  },
  {
    files: ["src/lib/api-client/**"],
    linterOptions: {
      reportUnusedDisableDirectives: "off",
    },
  },
]);

export default eslintConfig;
