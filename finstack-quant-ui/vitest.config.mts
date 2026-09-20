import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";
export default defineConfig({
  resolve: {
    alias: {
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
