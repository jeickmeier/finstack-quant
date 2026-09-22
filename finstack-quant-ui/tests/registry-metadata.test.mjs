import { expect, it } from "vitest";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
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
it("reads the installed facade version without generated WASM package manifests", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "finstack-registry-metadata-"),
  );
  try {
    const cwd = path.join(directory, "ui");
    const packageDirectory = path.join(
      directory,
      "node_modules/finstack-quant-wasm",
    );
    await mkdir(path.join(cwd, "src/generated"), { recursive: true });
    await mkdir(packageDirectory, { recursive: true });
    const files = [
      [
        path.join(cwd, "package.json"),
        { name: "registry-test", version: "1.2.3" },
      ],
      [
        path.join(packageDirectory, "package.json"),
        {
          name: "finstack-quant-wasm",
          version: "4.5.6",
          exports: { "./package.json": "./package.json" },
        },
      ],
      [path.join(cwd, "src/generated/roots.json"), []],
      [path.join(cwd, "src/contract-provenance.json"), { entries: [] }],
    ];
    await Promise.all(
      files.map(([file, value]) => writeFile(file, JSON.stringify(value))),
    );
    await expect(metadataInputs(cwd)).resolves.toEqual({
      version: "1.2.3",
      wasmVersion: "4.5.6",
      roots: [],
      provenance: { entries: [] },
    });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
