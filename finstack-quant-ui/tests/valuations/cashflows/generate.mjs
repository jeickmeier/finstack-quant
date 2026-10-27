import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { emit } from "../../../scripts/fixture-output.mjs";
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
// Pinned inputs: the JSON files this fixture is built from. The manifest also
// supplies request fields and patches; drift there is caught by `--check`, not
// by this pin list, so its other cases do not invalidate these flows.
const paths = new Set([bondPath]);
async function source(source) {
  paths.add(source.path);
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
    request.metricPricingOverrides,
    request.marketHistory,
  );
  let cashflows = null,
    error = null;
  try {
    cashflows = JSON.stringify(
      native.instrumentCashflows(
        request.instrumentJson,
        request.marketJson,
        request.asOf,
        request.model,
      ),
    );
  } catch (failure) {
    error = failure.message;
  }
  return { type, request, pricedValue: priced.value, cashflows, error };
});
const sources = await Promise.all(
  [...paths].map(async (path) => ({
    path,
    sha256: createHash("sha256")
      .update(await readFile(new URL(path, repository)))
      .digest("hex"),
  })),
);
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Serialized native cashflow envelopes; no inferred row schema or financial reconciliation.",
    sources,
    cases,
  }),
);
