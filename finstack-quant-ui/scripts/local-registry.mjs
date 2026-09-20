import { mkdtemp, readFile, readdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { serveExport } from "../tests/browser/static-server.mjs";

/** Serve built sources unchanged, replacing only unpublished npm dependency transport. */
export async function serveLocalRegistry(root) {
  const staged = await mkdtemp(path.join(tmpdir(), "finstack-local-registry-"));
  const wasm = path.resolve(root, "../finstack-quant-wasm");
  const pkg = JSON.parse(
    await readFile(path.join(wasm, "package.json"), "utf8"),
  );
  let server;
  try {
    for (const name of await readdir(path.join(root, "public/r"))) {
      if (!name.endsWith(".json")) continue;
      const item = JSON.parse(
        await readFile(path.join(root, "public/r", name), "utf8"),
      );
      if (item.dependencies)
        item.dependencies = item.dependencies.map((value) =>
          value === `${pkg.name}@${pkg.version}` ? `file:${wasm}` : value,
        );
      await writeFile(path.join(staged, name), JSON.stringify(item));
    }
    server = await serveExport(staged);
    return {
      url: server.url,
      wasm,
      close: async () => {
        await server.close();
        await rm(staged, { recursive: true, force: true });
      },
    };
  } catch (error) {
    await server?.close();
    await rm(staged, { recursive: true, force: true });
    throw error;
  }
}
