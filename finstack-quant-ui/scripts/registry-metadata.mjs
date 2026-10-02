import { readFile } from "node:fs/promises";
import path from "node:path";
const read = async (file) => JSON.parse(await readFile(file, "utf8"));
/** Registry items pin the workspace WASM package; it is unpublished, so installs resolve it by path. */
export async function wasmVersion(cwd) {
  return (await read(path.resolve(cwd, "../finstack-quant-wasm/package.json")))
    .version;
}
/** Item dependencies with every `finstack-quant-wasm@` entry pinned to `version`. */
export function pinWasm(dependencies, version) {
  return dependencies?.map((dependency) =>
    dependency.startsWith("finstack-quant-wasm@")
      ? `finstack-quant-wasm@${version}`
      : dependency,
  );
}
export async function metadataInputs(cwd) {
  const [ui, wasm, roots, provenance] = await Promise.all([
    read(path.join(cwd, "package.json")),
    wasmVersion(cwd),
    read(path.join(cwd, "src/generated/roots.json")),
    read(path.join(cwd, "src/contract-provenance.json")),
  ]);
  return { version: ui.version, wasmVersion: wasm, roots, provenance };
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
/** Base generators own content; the final metadata pass owns these two derived fields and the WASM pin. */
export function withoutMetadata(registry) {
  return {
    ...registry,
    items: registry.items.map(({ meta, categories, ...item }) => item),
  };
}
