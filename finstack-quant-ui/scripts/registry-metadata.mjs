import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
const read = async (file) => JSON.parse(await readFile(file, "utf8"));
export async function metadataInputs(cwd) {
  const require = createRequire(path.join(cwd, "package.json"));
  const [ui, wasm, roots, provenance] = await Promise.all([
    read(path.join(cwd, "package.json")),
    read(require.resolve("finstack-quant-wasm/package.json")),
    read(path.join(cwd, "src/generated/roots.json")),
    read(path.join(cwd, "src/contract-provenance.json")),
  ]);
  return { version: ui.version, wasmVersion: wasm.version, roots, provenance };
}
const categories = {
  "registry:ui": "Primitive Components",
  "registry:component": "Individual Components",
  "registry:block": "Blocks",
};
/** Canonical URIs come from source inventory and installed contract ownership, never version guesses. */
export function itemMetadata(item, graph, inputs) {
  const ids = new Set();
  const closure = graph.closures.get(item.name);
  for (const name of closure) {
    const dependency = graph.byName.get(name);
    for (const root of inputs.roots)
      if (
        dependency.files?.some(
          (file) => file.target === `lib/finstack/generated/${root.schema}`,
        )
      )
        ids.add(root.uri);
    for (const entry of inputs.provenance.entries)
      if (entry.id === name)
        for (const schema of entry.schemas ?? []) ids.add(schema.uri);
  }
  return {
    categories: categories[item.type] ? [categories[item.type]] : [],
    meta: {
      version: inputs.version,
      wasmVersion: inputs.wasmVersion,
      schemaIds: [...ids].sort(),
    },
  };
}
/** Base generators own content; the final metadata pass owns these two derived fields. */
export function withoutMetadata(registry) {
  return {
    ...registry,
    items: registry.items.map(({ meta, categories, ...item }) => item),
  };
}
