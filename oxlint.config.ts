import { defineConfig } from "oxlint";

export default defineConfig({
  ignorePatterns: ["/*", "!/src/", "/src/routeTree.gen.ts"],
  env: {
    browser: true,
    builtin: true,
  },
  plugins: ["oxc", "typescript", "unicorn", "react"],
  categories: {
    correctness: "error",
    suspicious: "warn",
  },
  rules: {
    "react/react-in-jsx-scope": "off",
  },
});
