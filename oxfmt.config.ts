import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: ["/*", "!/src/", "/src/routeTree.gen.ts"],
  sortImports: {
    groups: [
      "builtin",
      "external",
      ["internal", "subpath"],
      ["parent", "sibling", "index"],
      "style",
      "unknown",
    ],
    newlinesBetween: true,
  },
});
