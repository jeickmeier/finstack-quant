import { readFile, writeFile, readdir } from "node:fs/promises";
import { basename } from "node:path";
import { fileURLToPath } from "node:url";
import prettier from "prettier";

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
    name: "finstack-worker",
    type: "registry:file",
    docs: "Installs the worker and its service/contracts at project-root workers/. Initialize through FinstackProvider in the browser; install the matching local WASM package. Only existing native facade calls run here.",
    files: [
      "finstack.worker.ts",
      "finstack-service.ts",
      "finstack-contract.ts",
    ].map((name) => ({
      path: `registry/workers/${name}`,
      type: "registry:file",
      target: `~/workers/${name}`,
    })),
    registryDependencies: ["@finstack/finstack-codec"],
    dependencies: ["finstack-quant-wasm@0.8.0", "comlink@4.4.2"],
  },
  item("primitive-contracts", ["generated/primitive-contracts.json"]),
  item(
    "finstack-format",
    ["format/format.ts", "format/columns.ts", "format/transport.ts"],
    ["finstack-codec", "finstack-host", "contract-valuation-result"],
    ["big.js@7.0.1", "@tanstack/react-table@9.2.4", "@types/big.js@6.2.2"],
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
    ["generated/instruments.ts"],
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
    ["finstack-codec", "contract-valuation-result"],
    ["finstack-quant-wasm@0.8.0"],
  ),
];
const output = await prettier.format(
  JSON.stringify({
    $schema: "https://ui.shadcn.com/schema/registry.json",
    name: "finstack",
    homepage: "https://jeickmeier.github.io/finstack-quant/",
    include: [
      "base",
      "theme",
      "primitives",
      "components",
      "blocks",
      "hooks",
      "lib",
      "file",
    ].map((category) => `registry/${category}/registry.json`),
    items,
  }),
  { parser: "json" },
);
const path = `${root}registry.json`;
if (process.argv.includes("--check")) {
  if ((await readFile(path, "utf8")) !== output)
    throw new Error("Generated registry drift; run ui-gen");
} else await writeFile(path, output);
console.log(
  `${process.argv.includes("--check") ? "Checked" : "Generated"} ${items.length} contract registry items.`,
);
