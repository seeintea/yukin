import { defineConfig } from "oxlint";

export default defineConfig({
  plugins: ["typescript", "unicorn", "oxc"],
  categories: {
    correctness: "error",
  },
  rules: {},
  env: {
    builtin: true,
  },
  ignorePatterns: [
    "**/src-tauri/**",
    "**/src/components/ui/**",
    "**/docs/**",
    "**/routeTree.gen.ts",
  ],
});
