import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

/** Add the existing native worker regression probe to an installed test consumer. */
export async function installProbe(sourceDirectory) {
  const route = path.join(sourceDirectory, "app/registry-probe");
  await mkdir(route, { recursive: true });
  await writeFile(
    path.join(route, "page.jsx"),
    await readFile(new URL("./fixture/page.jsx", import.meta.url)),
  );
  const footprint = (
    await readFile(new URL("./fixture/footprint.js", import.meta.url), "utf8")
  )
    .replaceAll(
      "finstack-quant-ui/instruments",
      "@/lib/finstack/generated/instruments",
    )
    .replaceAll("finstack-quant-ui/codec", "@/lib/finstack/codec.mjs");
  await writeFile(path.join(route, "footprint.js"), footprint);
}
