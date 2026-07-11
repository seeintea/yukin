import { defineConfig } from "oxfmt";

export default defineConfig({
  printWidth: 80,
  ignorePatterns: [
    "**/src-tauri/**",
    "**/src/components/ui/**",
    "**/docs/**",
    "**/routeTree.gen.ts",
  ],
});
