import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import prettier from "prettier";
const repository = new URL("../../../../", import.meta.url);
const read = async (path) =>
  JSON.parse(await readFile(new URL(path, repository), "utf8"));
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const manifestPath =
  "finstack-quant-ui/tests/valuations/instruments/pricing-cases.json";
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const manifest = await read(manifestPath);
async function source(source) {
  let value = await read(source.path);
  for (const key of source.pointer) value = value[key];
  return value;
}
function patch(value, patches) {
  for (const { path, value: next } of patches) {
    let target = value;
    for (const key of path.slice(0, -1)) target = target[key];
    target[path.at(-1)] = next;
  }
  return value;
}
const inputs = [{ type: "bond", request: (await read(bondPath)).request }];
for (const type of ["fx_swap", "xccy_swap", "equity_option"]) {
  const entry = manifest.cases.find((c) => c.type === type);
  inputs.push({
    type,
    request: {
      ...entry.request,
      instrumentJson: JSON.stringify(
        patch(await source(entry.instrumentSource), entry.instrumentPatches),
      ),
      marketJson: JSON.stringify(
        patch(await source(entry.marketSource), entry.marketPatches),
      ),
    },
  });
}
const cases = inputs.map(({ type, request }) => {
  const priced = native.priceInstrument(
    request.instrumentJson,
    request.marketJson,
    request.asOf,
    request.model,
    request.metrics,
    request.pricingOptions,
    request.marketHistory,
  );
  let cashflows = null,
    error = null;
  try {
    cashflows = native.instrumentCashflowsJson(
      request.instrumentJson,
      request.marketJson,
      request.asOf,
      request.model,
    );
  } catch (failure) {
    error = failure.message;
  }
  return { type, request, pricedValue: priced.value, cashflows, error };
});
const paths = new Set([manifestPath, bondPath]);
for (const entry of manifest.cases.filter(
  (c) => inputs.some((i) => i.type === c.type) && c.type !== "bond",
)) {
  paths.add(entry.instrumentSource.path);
  paths.add(entry.marketSource.path);
}
const sources = await Promise.all(
  [...paths].map(async (path) => ({
    path,
    sha256: createHash("sha256")
      .update(await readFile(new URL(path, repository)))
      .digest("hex"),
  })),
);
const output = await prettier.format(
  JSON.stringify({
    description:
      "Actual native cashflow text, retained verbatim; no inferred row schema or financial reconciliation.",
    sources,
    cases,
  }),
  { parser: "json" },
);
const target = new URL("cases.json", import.meta.url);
if (process.argv.includes("--check")) {
  if ((await readFile(target, "utf8")) !== output)
    throw new Error(
      "Cashflow fixtures drifted; review canonical changes before regenerating.",
    );
} else await writeFile(target, output);
