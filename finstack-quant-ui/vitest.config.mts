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
        new URL(
          "./registry/shared/lib/finstack-form/form.tsx",
          import.meta.url,
        ),
      ),
      "@/components/finstack": fileURLToPath(
        new URL("./registry", import.meta.url),
      ),
      "@/hooks/shared": fileURLToPath(
        new URL("./registry/shared/hooks", import.meta.url),
      ),
      "@/hooks/core": fileURLToPath(
        new URL("./registry/core/hooks", import.meta.url),
      ),
      "@/hooks/valuations": fileURLToPath(
        new URL("./registry/valuations/hooks", import.meta.url),
      ),
      "@/hooks/calibration": fileURLToPath(
        new URL("./registry/calibration/hooks", import.meta.url),
      ),
      "@/hooks/models": fileURLToPath(
        new URL("./registry/models/hooks", import.meta.url),
      ),
      "@/workers": fileURLToPath(
        new URL("./registry/shared/file", import.meta.url),
      ),
      "@/lib/finstack": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
});
