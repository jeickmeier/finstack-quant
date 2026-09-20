import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";
export default defineConfig({
  resolve: {
    alias: {
      "@/hooks": fileURLToPath(new URL("./registry/hooks", import.meta.url)),
      "@/lib/finstack": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
});
