import path from "node:path";
import { fileURLToPath } from "node:url";
import { serveExport } from "../browser/static-server.mjs";
const directory =
  process.env.REGISTRY_EXPORT_DIR ??
  fileURLToPath(
    new URL("../../../docs-site/.registry-publish/out", import.meta.url),
  );
const server = await serveExport(
  path.resolve(directory),
  process.env.NEXT_PUBLIC_BASE_PATH ?? "",
  4178,
);
console.log(`Registry gallery server: ${server.url}`);
for (const signal of ["SIGINT", "SIGTERM"])
  process.on(signal, () => void server.close().then(() => process.exit(0)));
