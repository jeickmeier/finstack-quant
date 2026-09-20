import { defineConfig, configDefaults } from "vitest/config";
import { fileURLToPath } from "node:url";
export default defineConfig({
  // Hosted runners need fewer concurrent WASM instances/schema transforms.
  test: {
    maxWorkers: process.env.CI ? 2 : 4,
    exclude: [...configDefaults.exclude, "tests/e2e/**"],
  },
  resolve: {
    alias: {
      "@/components/ui": fileURLToPath(
        new URL("./components/ui", import.meta.url),
      ),
      "@/lib/finstack/form": fileURLToPath(
        new URL("./registry/lib/finstack-form/form.tsx", import.meta.url),
      ),
      "@/components/finstack": fileURLToPath(
        new URL("./registry", import.meta.url),
      ),
      "@/hooks": fileURLToPath(new URL("./registry/hooks", import.meta.url)),
      "@/lib/finstack": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
});
