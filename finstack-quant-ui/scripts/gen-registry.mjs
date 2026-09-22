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
const contracts = roots.map(({ schema }) => {
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
  return item(
    `contract-${name.replaceAll("_", "-")}`,
    files,
    instrument ? ["finstack-codec"] : [],
  );
});
const items = [
  {
    name: "curve-link-example",
    type: "registry:component",
    docs: "Stored discount curve table and chart share accepted semantic selection. Explicit row/cell addresses preserve original data and render identity; caller activation callbacks update detail only. No WASM or financial calculations.",
    files: [
      {
        path: "registry/core/components/curve-link-example/curve-link-example.tsx",
        type: "registry:component",
        target:
          "components/finstack/core/components/curve-link-example/curve-link-example.tsx",
      },
      file("fixtures/curves/market.json"),
    ],
    registryDependencies: [
      "@finstack/finstack-table",
      "@finstack/curve-chart",
      "@finstack/use-linked-selection",
      "@finstack/finstack-codec",
      "@finstack/contract-market-context-state",
    ],
    dependencies: ["@tanstack/charts@0.18.0", "@tanstack/react-table@9.2.4"],
  },

  item(
    "finstack-fixtures",
    ["fixtures/results/bond.json"],
    ["contract-bond", "contract-market-context-state"],
  ),
  item("primitive-contracts", ["generated/primitive-contracts.json"]),
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
