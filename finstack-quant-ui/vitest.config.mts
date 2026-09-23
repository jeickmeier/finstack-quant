import { defineConfig, configDefaults } from "vitest/config";
import { fileURLToPath } from "node:url";
import tsconfig from "./tsconfig.json" with { type: "json" };

const alias = Object.entries(tsconfig.compilerOptions.paths).map(
  ([key, [target]]) => ({
    find: key.endsWith("/*") ? key.slice(0, -2) : key,
    replacement: fileURLToPath(
      new URL(`./${target.replace(/\/\*$/, "")}`, import.meta.url),
    ),
  }),
);

export default defineConfig({
  // Hosted runners need fewer concurrent WASM instances/schema transforms.
  test: {
    maxWorkers: process.env.CI ? 2 : 4,
    exclude: [...configDefaults.exclude, "tests/e2e/**"],
  },
  resolve: { alias },
});
