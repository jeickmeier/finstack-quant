import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import prettier from "prettier";
import { serializeHost } from "../../src/codec.mjs";
const repository = new URL("../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const read = async (path) =>
  JSON.parse(await readFile(new URL(path, repository), "utf8"));
const manifestPath = "finstack-quant-ui/tests/instruments/pricing-cases.json";
const commodityPath =
  "finstack-quant/valuations/tests/fixtures/valuation_binding_regressions.json";
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const manifest = await read(manifestPath);
const sources = new Set([
  manifestPath,
  commodityPath,
  bondPath,
  "finstack-quant-wasm/tests/facade/covenants.test.mjs",
  "finstack-quant-wasm/index.d.ts",
  "finstack-quant-wasm/src/utils/mod.rs",
  "finstack-quant/core/src/explain.rs",
]);
async function source(source) {
  sources.add(source.path);
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
function bigintPaths(value, path = []) {
  if (typeof value === "bigint") return [path];
  if (!value || typeof value !== "object") return [];
  return Object.entries(value).flatMap(([key, child]) =>
    bigintPaths(child, [...path, Array.isArray(value) ? Number(key) : key]),
  );
}
const cases = [];
for (const type of [
  "commodity_option",
  "composite",
  "credit_default_swap",
  "fx_option",
  "structured_credit",
  "bond",
]) {
  const entry = manifest.cases.find((c) => c.type === type);
  let instrument = patch(
    await source(entry.instrumentSource),
    entry.instrumentPatches,
  );
  let market = patch(await source(entry.marketSource), entry.marketPatches);
  let asOf = entry.request.asOf,
    model = entry.request.model;
  if (type === "commodity_option") {
    const fixture = await read(commodityPath);
    instrument = fixture.commodity;
    market = fixture.market;
    instrument.instrument.spec.instrument_pricing_overrides.model_config.mc_paths = 64;
    asOf = "2025-01-02";
    model = "monte_carlo_schwartz_smith";
  }
  if (type === "structured_credit") {
    instrument.instrument.spec.instrument_pricing_overrides = {
      model_config: { mc_paths: 8 },
    };
    model = "structured_credit_stochastic";
  }
  if (type === "bond") {
    const fixture = await read(bondPath);
    instrument = JSON.parse(fixture.request.instrumentJson);
    market = JSON.parse(fixture.request.marketJson);
    asOf = fixture.request.asOf;
    model = "rates_credit";
  }
  const request = {
    instrumentJson: JSON.stringify(instrument),
    marketJson: JSON.stringify(market),
    asOf,
    model,
    metrics: [],
  };
  const result = native.priceInstrument(
    request.instrumentJson,
    request.marketJson,
    asOf,
    model,
    [],
  );
  const expected = {
    commodity_option: "monte_carlo",
    composite: "composite",
    credit_default_swap: "credit_derivative",
    fx_option: "fx",
    structured_credit: "structured_credit_stochastic",
    bond: "monte_carlo",
  }[type];
  if (result.details?.type !== expected)
    throw new Error(`${type}: required details were not returned`);
  cases.push({
    type,
    request,
    detailType: expected,
    hostBigIntPaths: bigintPaths(result),
    resultJson: native.validateValuationResultJson(serializeHost(result)),
  });
}
const specs = JSON.parse(native.lboStandardJson(5, 1.5, 1.2, 10000000));
const covenantRequest = {
  engineJson: JSON.stringify({
    specs: [specs[0]],
    breach_history: [],
    windows: [],
    waivers: [],
  }),
  metricsJson: '{"debt_to_ebitda":6}',
  asOf: "2026-03-31",
};
const reports = native.evaluateEngine(
  covenantRequest.engineJson,
  covenantRequest.metricsJson,
  covenantRequest.asOf,
);
const output = {
  description:
    "Actual native results. hostBigIntPaths record transport kinds emitted by the binding serializer, not domain types inferred from examples. Reconstruct through the canonical wire codec then restore these exact integer leaves; tests compare with a fresh native call.",
  sources: await Promise.all(
    [...sources].map(async (path) => ({
      path,
      sha256: createHash("sha256")
        .update(await readFile(new URL(path, repository)))
        .digest("hex"),
    })),
  ),
  cases,
  covenantRequest,
  covenantReportsJson: serializeHost(reports),
};
await writeFile(
  new URL("cases.json", import.meta.url),
  await prettier.format(JSON.stringify(output), { parser: "json" }),
);
