import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { serveExport } from "../browser/static-server.mjs";
const directory =
  process.env.REGISTRY_EXPORT_DIR ??
  fileURLToPath(
    new URL("../../../docs-site/.registry-publish/out", import.meta.url),
  );
if (!existsSync(path.join(directory, "registry-gallery/index.html"))) {
  console.error(
    `No gallery export at ${directory}; run \`mise run ui-docs-build\` first.`,
  );
  process.exit(1);
}
const server = await serveExport(
  path.resolve(directory),
  process.env.NEXT_PUBLIC_BASE_PATH ?? "",
  Number(process.env.REGISTRY_GALLERY_PORT ?? 4178),
);
console.log(`Registry gallery server: ${server.url}`);
for (const signal of ["SIGINT", "SIGTERM"])
  process.on(signal, () => void server.close().then(() => process.exit(0)));
