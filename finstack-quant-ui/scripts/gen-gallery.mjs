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
  market: "tests/market-browser/cases.json",
  cubes: "tests/cubes/cases.json",
  calibration: "tests/calibration/cases.json",
  scenarios: "tests/scenarios/cases.json",
  cashflows: "tests/cashflows/cases.json",
  details: "tests/details/cases.json",
};
const data = {},
  provenance = [];
for (const [name, relative] of Object.entries(sources)) {
  const bytes = await readFile(path.join(root, relative));
  data[name] = JSON.parse(bytes);
  provenance.push({
    path: `finstack-quant-ui/${relative}`,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  });
}
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
