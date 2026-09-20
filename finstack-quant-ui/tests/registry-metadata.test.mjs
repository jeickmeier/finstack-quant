import { expect, it } from "vitest";
import { fileURLToPath } from "node:url";
import { loadRegistry } from "shadcn/registry";
import { checkGraph } from "../scripts/check-registry.mjs";
import { itemMetadata, metadataInputs } from "../scripts/registry-metadata.mjs";
it("derives canonical IDs and versions from the owned installed contracts", async () => {
  const cwd = fileURLToPath(new URL("../", import.meta.url));
  const { items } = await loadRegistry({ cwd });
  const graph = checkGraph(items),
    inputs = await metadataInputs(cwd);
  const bond = inputs.roots.find((root) => root.schema === "schemas/bond.json");
  expect(graph.byName.get("contract-bond").meta.schemaIds).toEqual([bond.uri]);
  expect(graph.byName.get("instrument-form").meta.schemaIds).toContain(
    bond.uri,
  );
  expect(graph.byName.get("pricing-workbench").meta.schemaIds).toContain(
    bond.uri,
  );
  for (const name of ["finstack-chart", "field-frame", "use-linked-selection"])
    expect(graph.byName.get(name).meta.schemaIds).toEqual([]);
  expect([...new Set(items.flatMap((item) => item.categories))].sort()).toEqual(
    ["Blocks", "Individual Components", "Primitive Components"],
  );
  const changed = {
    ...inputs,
    version: "9.8.7",
    wasmVersion: "8.7.6",
    roots: inputs.roots.map((root) =>
      root === bond
        ? { ...root, uri: "https://example.test/canonical-bond" }
        : root,
    ),
  };
  expect(
    itemMetadata(graph.byName.get("contract-bond"), graph, changed).meta,
  ).toEqual({
    version: "9.8.7",
    wasmVersion: "8.7.6",
    schemaIds: ["https://example.test/canonical-bond"],
  });
});
