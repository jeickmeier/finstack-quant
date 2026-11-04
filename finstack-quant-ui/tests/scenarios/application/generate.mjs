import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoSource } from "../../../scripts/fixture-inputs.mjs";
// The priced bond of the results fixture (its request, metrics included) is the base.
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const bondInput = await readRepoSource(bondPath);
const base = bondInput.value.request;
// Builder drafts; blank options are omitted so Rust applies its defaults.
const drafts = [
  {
    name: "usd-ois-50bp",
    draft: {
      id: "usd-ois-50bp",
      shifts: [{ curveKind: "discount", curveId: "USD-OIS", bp: "50" }],
      roll: null,
      hazardBumpMode: "",
    },
  },
  {
    name: "roll-1m-25bp",
    draft: {
      id: "roll-1m-25bp",
      shifts: [{ curveKind: "discount", curveId: "USD-OIS", bp: "25" }],
      roll: { period: "1M", rollMode: "" },
      hazardBumpMode: "",
    },
  },
  {
    name: "hz-par-cds-25bp",
    draft: {
      id: "hz-par-cds-25bp",
      shifts: [{ curveKind: "par_cds", curveId: "HZ", bp: "25" }],
      roll: null,
      hazardBumpMode: "first_order_shift",
    },
  },
];
// The worker's ScenariosApi: buildMarketScenario, then applyMarketScenario.
function build(draft) {
  const operations = [
    ...(draft.roll
      ? [
          native.operationSpecTimeRollForward(
            draft.roll.period,
            undefined,
            draft.roll.rollMode || undefined,
          ),
        ]
      : []),
    ...draft.shifts.flatMap((shift) =>
      native.parallelBpMany(shift.curveKind, [shift.curveId], Number(shift.bp)),
    ),
  ];
  return JSON.stringify(
    native.parseScenarioSpec(
      JSON.stringify({
        id: draft.id,
        operations,
        ...(draft.hazardBumpMode
          ? { hazard_bump_mode: draft.hazardBumpMode }
          : {}),
      }),
    ),
  );
}
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
const cases = drafts.map(({ name, draft }) => {
  const scenarioJson = build(draft);
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
    draft,
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
      "Native market scenarios on the results-fixture bond: the Rust-built spec, the applyScenario report and the bond repriced with the same model and metrics on the shocked market at the returned valuation date. The shocked market itself is not stored.",
    sources: [bondInput.source],
    base: { asOf: base.asOf, value: bondInput.value.result.value },
    cases,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length} native market scenario applications.`,
);
