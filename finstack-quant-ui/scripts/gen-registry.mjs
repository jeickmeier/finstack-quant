import { readFile, writeFile, readdir } from "node:fs/promises";
import { basename } from "node:path";
import { fileURLToPath } from "node:url";
import prettier from "prettier";
import { isDeepStrictEqual } from "node:util";
import { withoutMetadata } from "./registry-metadata.mjs";

import ts from "typescript";
import { shadcnItems } from "./shadcn.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const read = (path) => readFile(`${root}${path}`, "utf8").then(JSON.parse);
const roots = await read("src/generated/roots.json");
const modules = new Set(await readdir(`${root}src/generated/instrument`));
const file = (path) => ({
  path: `src/${path}`,
  type: "registry:file",
  target: `lib/finstack/${path}`,
});
const item = (name, files, registryDependencies = [], dependencies = []) => ({
  name,
  type: "registry:lib",
  docs: `Installs ${name} under lib/finstack with relative imports. Generated schemas preserve canonical Rust wire contracts; UI validation is structural and native validation remains required.`,
  files: files.map(file),
  registryDependencies: registryDependencies.map((name) => `@finstack/${name}`),
  dependencies,
});
const sharedMarker =
  "https://finstack_quant.dev/schemas/ui/1/shared-defs.schema.json";
const contracts = [];
for (const { schema } of roots) {
  const name = basename(schema, ".json");
  const files = [
    `generated/${schema}`,
    `generated/types/${name}.ts`,
    `generated/meta/${name}.ts`,
  ];
  const instrument = modules.has(`${name}.ts`);
  if (instrument)
    files.push(
      `generated/instrument/${name}.ts`,
      `generated/examples/${name}.json`,
    );
  if (name === "financial_model_spec")
    files.push(`generated/examples/${name}.json`);
  const text = await readFile(`${root}src/generated/${schema}`, "utf8");
  const dependencies = [];
  if (instrument) dependencies.push("finstack-codec");
  if (text.includes(sharedMarker)) dependencies.push("shared-schema-defs");
  contracts.push(
    item(`contract-${name.replaceAll("_", "-")}`, files, dependencies),
  );
}
const items = [
  item(
    "finstack-fixtures",
    ["fixtures/results/bond.json"],
    ["contract-bond", "contract-market-context-state"],
  ),
  item("primitive-contracts", ["generated/primitive-contracts.json"]),
  item(
    "shared-schema-defs",
    [
      "generated/defs/shared.json",
      "generated/types/shared.ts",
      "generated/meta/shared.ts",
    ],
    ["finstack-codec"],
  ),
  item(
    "finstack-format",
    ["format/format.ts", "format/columns.ts"],
    ["contract-valuation-result"],
    [
      "big.js@7.0.1",
      "@tanstack/react-table@9.2.4",
      "@types/big.js@6.2.2",
      "finstack-quant-wasm@0.8.0",
    ],
  ),
  item(
    "finstack-codec",
    ["codec.mjs", "codec.d.mts", "schema.mjs", "schema.d.mts"],
    [],
    ["lossless-json@4.3.1", "zod@4.6.5"],
  ),
  ...contracts,
  item(
    "instrument-catalogue",
    ["generated/instruments.ts", "generated/catalogue.json"],
    contracts
      .filter((entry) =>
        entry.files.some((f) => f.path.includes("/instrument/")),
      )
      .map((entry) => entry.name),
  ),
  item("contract-manifest", [
    "generated/roots.json",
    "generated/fixtures.json",
    "contract-provenance.json",
  ]),
  item("finstack-views", ["views.ts", "generated/curve-views.json"]),
  item(
    "finstack-host",
    ["host.ts"],
    ["finstack-codec"],
    ["finstack-quant-wasm@0.8.0"],
  ),
];
for (const entry of items) {
  const stock = new Set();
  for (const source of entry.files ?? []) {
    if (!/\.[cm]?[jt]sx?$/.test(source.path)) continue;
    const content = await readFile(`${root}${source.path}`, "utf8");
    for (const { fileName } of ts.preProcessFile(content, true, true)
      .importedFiles)
      if (fileName.startsWith("@/components/ui/")) {
        const name = fileName.slice("@/components/ui/".length);
        if (!shadcnItems.has(name))
          throw Error(`Unknown shadcn dependency: ${name}`);
        stock.add(name);
      }
  }
  if (stock.size)
    entry.registryDependencies = [
      ...(entry.registryDependencies ?? []),
      ...[...stock].sort(),
    ];
}
const output = await prettier.format(
  JSON.stringify({
    $schema: "https://ui.shadcn.com/schema/registry.json",
    name: "finstack",
    homepage: "https://jeickmeier.github.io/finstack-quant/",
    include: [
      "base",
      "theme",
      "shared",
      "core",
      "valuations",
      "calibration",
      "covenants",
      "models",
      "statements",
    ].map((category) => `registry/${category}/registry.json`),
    items,
  }),
  { parser: "json" },
);
const path = `${root}registry.json`;
if (process.argv.includes("--check")) {
  if (
    !isDeepStrictEqual(
      withoutMetadata(JSON.parse(await readFile(path, "utf8"))),
      JSON.parse(output),
    )
  )
    throw new Error("Generated registry drift; run ui-gen");
} else await writeFile(path, output);
console.log(
  `${process.argv.includes("--check") ? "Checked" : "Generated"} ${items.length} contract registry items.`,
);
