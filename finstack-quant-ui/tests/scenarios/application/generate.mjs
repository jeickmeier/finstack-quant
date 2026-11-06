import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoSource } from "../../../scripts/fixture-inputs.mjs";
// The priced bond of the results fixture (its request, metrics included) is the base.
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const bondInput = await readRepoSource(bondPath);
const base = bondInput.value.request;
// The ScenarioSpecs the builder writes: curve_parallel_bp and time_roll_forward
// operations as entered, with every omitted option left to Rust's defaults.
const shift = (curve_kind, curve_id, bp) => ({
  kind: "curve_parallel_bp",
  curve_kind,
  curve_id,
  bp,
});
const specs = [
  {
    name: "usd-ois-50bp",
    spec: {
      id: "usd-ois-50bp",
      operations: [shift("discount", "USD-OIS", 50)],
    },
  },
  {
    name: "roll-1m-25bp",
    spec: {
      id: "roll-1m-25bp",
      operations: [
        { kind: "time_roll_forward", period: "1M" },
        shift("discount", "USD-OIS", 25),
      ],
    },
  },
  {
    name: "hz-par-cds-25bp",
    spec: {
      id: "hz-par-cds-25bp",
      operations: [shift("par_cds", "HZ", 25)],
      hazard_bump_mode: "first_order_shift",
    },
  },
];
// The worker's ScenariosApi: validateScenarioSpec, then applyMarketScenario.
const canonical = (spec) =>
  JSON.stringify(native.parseScenarioSpec(JSON.stringify(spec)));
function apply(scenarioJson) {
  const applied = native.applyScenario(
    scenarioJson,
    base.marketJson,
    base.asOf,
    undefined,
    native.scenarioSpecRequiresInstruments(scenarioJson)
      ? `[${base.instrumentJson}]`
      : undefined,
  );
  const market = native.MarketContext.fromJson(JSON.stringify(applied.market));
  let marketJson;
  try {
    marketJson = market.toJson();
  } finally {
    market.free();
  }
  return {
    report: applied.report,
    marketJson,
    asOf: native.applicationReportEffectiveAsOf(applied.report, base.asOf),
    instrumentJson: native.scenarioSpecMutatesInstruments(scenarioJson)
      ? JSON.stringify(applied.instruments[0])
      : base.instrumentJson,
  };
}
const cases = specs.map(({ name, spec }) => {
  const scenarioJson = canonical(spec);
  const { report, marketJson, asOf, instrumentJson } = apply(scenarioJson);
  const shocked = native.priceInstrument(
    instrumentJson,
    marketJson,
    asOf,
    base.model,
    base.metrics,
    base.metricPricingOverrides,
    base.marketHistory,
  );
  return {
    name,
    spec,
    scenarioJson,
    report,
    asOf,
    shocked,
    metadata: native.metricMetadata(Object.keys(shocked.measures)),
  };
});
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native market scenarios on the results-fixture bond: the ScenarioSpec as written and its canonical form from parseScenarioSpec, the applyScenario report and the bond repriced with the same model and metrics on the shocked market at the returned valuation date. The shocked market itself is not stored.",
    sources: [bondInput.source],
    base: { asOf: base.asOf, value: bondInput.value.result.value },
    cases,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length} native market scenario applications.`,
);
