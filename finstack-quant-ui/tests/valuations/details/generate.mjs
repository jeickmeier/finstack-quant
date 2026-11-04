import { serializeHost } from "../../../src/codec.mjs";
import { emit } from "../../../scripts/fixture-output.mjs";
import {
  native,
  readRepoJson,
  sourceRecord,
} from "../../../scripts/fixture-inputs.mjs";
const manifestPath =
  "finstack-quant-ui/tests/valuations/instruments/pricing-cases.json";
const commodityPath =
  "finstack-quant/valuations/tests/fixtures/valuation_binding_regressions.json";
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const manifest = await readRepoJson(manifestPath);
// Pinned inputs: only the JSON files each case is built from. The manifest's
// request fields and patches are covered by `--check`; binding sources are not
// pinned because the test re-prices every case through the native package.
const sources = new Set([commodityPath, bondPath]);
async function source(source) {
  sources.add(source.path);
  let value = await readRepoJson(source.path);
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
const cases = [];
for (const type of [
  "commodity_spot_option",
  "composite",
  "credit_default_swap",
  "fx_option",
  "structured_credit",
  "bond",
]) {
  let instrument, market, asOf, model;
  if (type === "commodity_spot_option") {
    const fixture = await readRepoJson(commodityPath);
    instrument = fixture.commodity;
    market = fixture.market;
    instrument.instrument.spec.instrument_pricing_overrides.model_config.mc_paths = 64;
    asOf = "2025-01-02";
    model = "monte_carlo_schwartz_smith";
  } else if (type === "bond") {
    const fixture = await readRepoJson(bondPath);
    instrument = JSON.parse(fixture.request.instrumentJson);
    market = JSON.parse(fixture.request.marketJson);
    asOf = fixture.request.asOf;
    model = "rates_credit";
  } else {
    const entry = manifest.cases.find((c) => c.type === type);
    instrument = patch(
      await source(entry.instrumentSource),
      entry.instrumentPatches,
    );
    market = patch(await source(entry.marketSource), entry.marketPatches);
    asOf = entry.request.asOf;
    model = entry.request.model;
    if (type === "structured_credit") {
      instrument.instrument.spec.instrument_pricing_overrides = {
        model_config: { mc_paths: 8 },
      };
      model = "structured_credit_stochastic";
    }
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
    commodity_spot_option: "monte_carlo",
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
    resultJson: native.valuationResultToJson(serializeHost(result)),
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
    "Actual native results. Decode resultJson through the canonical wire codec, which yields the facade transport (bigint for schema int64/uint64 fields); tests compare with a fresh native call.",
  sources: await Promise.all([...sources].map(sourceRecord)),
  cases,
  covenantRequest,
  covenantReportsJson: serializeHost(reports),
};
await emit(new URL("cases.json", import.meta.url), JSON.stringify(output));
