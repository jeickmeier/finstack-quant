import { readFile, writeFile, mkdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";
import prettier from "prettier";
import { loadRegistry } from "shadcn/registry";
const root = fileURLToPath(new URL("../", import.meta.url)),
  repo = path.resolve(root, "..");
const output = path.join(repo, "docs-site/src/components/registry-gallery");
const registry = await loadRegistry({ cwd: root });
const visualTypes = new Set([
  "registry:ui",
  "registry:component",
  "registry:block",
  "registry:theme",
]);
const visual = registry.items
  .filter((item) => visualTypes.has(item.type))
  .map((item) => ({ name: item.name, type: item.type, docs: item.docs }));
const nonvisual = registry.items.filter((item) => !visualTypes.has(item.type));
const sources = {
  bond: "src/fixtures/results/bond.json",
  market: "tests/core/market-browser/cases.json",
  cubes: "tests/core/cubes/cases.json",
  calibration: "tests/calibration/cases.json",
  scenarios: "tests/valuations/scenario-table/cases.json",
  cashflows: "tests/valuations/cashflows/cases.json",
  details: "tests/valuations/details/cases.json",
  irsInstrument:
    "../finstack-quant/valuations/tests/instruments/json_examples/interest_rate_swap.json",
  pricingMarket: "tests/valuations/instruments/pricing-market.json",
};
const data = {},
  provenance = [];
for (const [name, relative] of Object.entries(sources)) {
  const bytes = await readFile(path.join(root, relative));
  data[name] = JSON.parse(bytes);
  provenance.push({
    path: path.relative(repo, path.join(root, relative)),
    sha256: createHash("sha256").update(bytes).digest("hex"),
  });
}
const pricingCases = JSON.parse(
  await readFile(
    path.join(root, "tests/valuations/instruments/pricing-cases.json"),
    "utf8",
  ),
).cases;
const catalogue = JSON.parse(
  await readFile(path.join(root, "src/generated/catalogue.json"), "utf8"),
);
const workbenchGroups = new Set(["credit", "rates", "fixed_income"]);
const workbenchTypes = new Set(
  catalogue
    .filter((entry) => workbenchGroups.has(entry.group))
    .map((entry) => entry.type),
);
async function readPinnedSource(source) {
  const bytes = await readFile(path.join(repo, source.path));
  if (createHash("sha256").update(bytes).digest("hex") !== source.sha256)
    throw new Error(`Stale pricing source: ${source.path}`);
  return structuredClone(
    source.pointer.reduce((value, key) => value[key], JSON.parse(bytes)),
  );
}
function applyPatches(value, patches) {
  for (const patch of patches) {
    const parent = patch.path
      .slice(0, -1)
      .reduce((node, key) => node[key], value);
    parent[patch.path.at(-1)] = structuredClone(patch.value);
  }
}
data.pricingExamples = {};
for (const entry of pricingCases.filter((entry) =>
  workbenchTypes.has(entry.type),
)) {
  const instrument = await readPinnedSource(entry.instrumentSource);
  applyPatches(instrument, entry.instrumentPatches);
  const market = await readPinnedSource(entry.marketSource);
  applyPatches(market, entry.marketPatches);
  const sharedMarket =
    entry.marketSource.path ===
      "finstack-quant-ui/tests/valuations/instruments/pricing-market.json" &&
    entry.marketPatches.length === 0;
  data.pricingExamples[entry.type] = {
    instrument,
    ...(sharedMarket ? {} : { market }),
    request: entry.request,
  };
  workbenchTypes.delete(entry.type);
}
if (workbenchTypes.size)
  throw new Error(`Missing workbench pricing cases: ${[...workbenchTypes]}`);
const imports = nonvisual.map((item) => {
  const targets = (item.files ?? [])
    .map((file) => file.target.replace(/^~\//, ""))
    .filter(
      (target) =>
        /\.(?:[cm]?js|tsx?|json)$/.test(target) &&
        !target.endsWith(".d.ts") &&
        !target.endsWith(".worker.ts"),
    );
  return `${JSON.stringify(item.name)}:()=>Promise.all([${targets.map((target) => `import(${JSON.stringify("@/" + target.replace(/\.tsx?$/, ""))})`).join(",")}]).then(modules=>({name:${JSON.stringify(item.name)},modules:modules.length}))`;
});
const files = {
  "inventory.json":
    JSON.stringify(
      {
        visual,
        nonvisual: nonvisual.map(({ name, type }) => ({ name, type })),
        provenance,
      },
      null,
      2,
    ) + "\n",
  "data.json": JSON.stringify(data, null, 2) + "\n",
  "imports.ts": `// Generated consumer import harness. Worker entry executes only through FinstackProvider.\nexport const importHarnesses={${imports.join(",\n")}};\n`,
};
if (!process.argv.includes("--check")) await mkdir(output, { recursive: true });
for (const [name, text] of Object.entries(files)) {
  const file = path.join(output, name),
    formatted = await prettier.format(text, {
      ...(await prettier.resolveConfig(file)),
      filepath: file,
    });
  if (process.argv.includes("--check")) {
    if ((await readFile(file, "utf8")) !== formatted)
      throw new Error(`Stale gallery artifact: ${name}`);
  } else await writeFile(file, formatted);
}
console.log(
  `Gallery: ${visual.length} visual and ${nonvisual.length} nonvisual items`,
);
