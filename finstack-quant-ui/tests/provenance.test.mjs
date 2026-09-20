import { expect, it } from "vitest";
import { createHash } from "node:crypto";
import { glob, readFile } from "node:fs/promises";
import { resolve } from "node:path";
import inventory from "../src/contract-provenance.json" with { type: "json" };
import { readContracts } from "../scripts/schema.mjs";
import { generateProvenance } from "../scripts/provenance.mjs";

const repo = resolve(import.meta.dirname, "../..");
const fixtures = await readFile(
  resolve(repo, "finstack-quant-ui/src/generated/fixtures.json"),
  "utf8",
);
const contracts = await readContracts(repo);
it("ties every display to canonical sources and current fixture discovery", () => {
  expect(inventory.fixtureManifestSha256).toBe(
    createHash("sha256").update(fixtures).digest("hex"),
  );
  for (const entry of inventory.entries) {
    expect(Boolean(entry.api || entry.schemas?.length), entry.id).toBe(true);
    if (entry.deferred) expect(inventory.deferred[entry.deferred]).toBeTruthy();
    for (const source of entry.schemas ?? []) {
      const root = contracts.get(source.uri).schema;
      const value =
        source.pointer === "#"
          ? root
          : source.pointer
              .slice(2)
              .split("/")
              .reduce(
                (node, key) =>
                  node[key.replaceAll("~1", "/").replaceAll("~0", "~")],
                root,
              );
      expect(
        createHash("sha256").update(JSON.stringify(value)).digest("hex"),
      ).toBe(source.sha256);
    }
  }
});
it("records full pricing inputs and the supported raw and typed routes", () => {
  const price = inventory.entries.find(
    (entry) => entry.id === "valuation-summary",
  ).api;
  expect(price.inputs.map((input) => input.name)).toEqual([
    "instrumentJson",
    "marketJson",
    "asOf",
    "model",
    "metrics",
    "pricingOptions",
    "marketHistory",
  ]);
  expect(
    inventory.details
      .filter((detail) => detail.route === "typed")
      .map((detail) => detail.type),
  ).toEqual(["monte_carlo"]);
  expect(
    inventory.details.filter((detail) => detail.route === "raw"),
  ).toHaveLength(4);
  expect(
    inventory.entries.find((entry) => entry.id === "cashflow-viewer").route,
  ).toBe("original-text");
  expect(
    inventory.entries.find((entry) => entry.id === "fx-delta-pillars").api
      .returns,
  ).toBe("Float64Array");
  expect(
    inventory.hostDeclarations.map((declaration) => declaration.name),
  ).toContain("VolCubeConstructor");
});
it("fails generation when a required source disappears", async () => {
  const changed = new Map(contracts);
  const uri = inventory.entries.find(
    (entry) => entry.id === "valuation-summary",
  ).schemas[0].uri;
  const contract = structuredClone(changed.get(uri));
  delete contract.schema.properties.value;
  changed.set(uri, contract);
  await expect(generateProvenance(repo, changed, fixtures)).rejects.toThrow(
    "Missing schema pointer",
  );
});
it("includes every UI generated artifact in the repository manifest", async () => {
  const manifest = (
    await readFile(resolve(repo, "scripts/generated-artifacts.txt"), "utf8")
  )
    .trim()
    .split("\n")
    .filter((path) => path.startsWith("finstack-quant-ui/"))
    .sort();
  const actual = [
    "finstack-quant-ui/src/contract-provenance.json",
    "finstack-quant-ui/registry.json",
    "finstack-quant-ui/registry/theme/registry.json",
    "finstack-quant-ui/registry/theme/finstack-theme/theme.css",
    "finstack-quant-ui/tests/instruments/encoding-report.json",
  ];
  for await (const path of glob("finstack-quant-ui/src/generated/**/*", {
    cwd: repo,
    withFileTypes: true,
  }))
    if (path.isFile())
      actual.push(`${path.parentPath.slice(repo.length + 1)}/${path.name}`);
  expect(manifest).toEqual(actual.sort());
});
