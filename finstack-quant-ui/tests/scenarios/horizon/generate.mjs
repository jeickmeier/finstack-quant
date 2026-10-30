import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { serializeHost } from "../../../src/codec.mjs";
const repository = new URL("../../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// The priced bond of the results fixture is the base, as for the scenario application.
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const bondBytes = await readFile(new URL(bondPath, repository));
const base = JSON.parse(bondBytes).request;
// Builder drafts as the workbench edits them; blank options are omitted so Rust applies its defaults.
const roll3m = {
  id: "roll-3m-50bp",
  shifts: [{ curveKind: "discount", curveId: "USD-OIS", bp: "50" }],
  roll: { period: "3M", rollMode: "" },
  hazardBumpMode: "",
};
const shift = {
  id: "usd-ois-50bp",
  shifts: [{ curveKind: "discount", curveId: "USD-OIS", bp: "50" }],
  roll: null,
  hazardBumpMode: "",
};
// Requests: method and calendarId are the worker request fields; null stays unset.
const requests = [
  { name: "roll-3m-50bp", draft: roll3m, method: null, calendarId: null },
  { name: "usd-ois-50bp", draft: shift, method: null, calendarId: null },
  {
    name: "roll-3m-50bp-taylor",
    draft: roll3m,
    method: { taylor: {} },
    calendarId: "nyse",
  },
  // The bond market's raw hazard curve has no calibration recipe, so the
  // metrics-based method drops its quote-space credit metrics with a note.
  {
    name: "roll-3m-50bp-metrics-based",
    draft: roll3m,
    method: "metrics_based",
    calendarId: null,
  },
];
// The worker's ScenariosApi.buildMarketScenario.
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
    ...draft.shifts.flatMap((entry) =>
      native.parallelBpMany(entry.curveKind, [entry.curveId], Number(entry.bp)),
    ),
  ];
  return JSON.stringify(
    native.parseScenarioSpec(JSON.stringify({ id: draft.id, operations })),
  );
}
// The worker's ScenariosApi.horizonReturn.
const cases = requests.map(({ name, draft, method, calendarId }) => {
  const scenarioJson = build(draft);
  try {
    const report = native.computeHorizonReturn(
      base.instrumentJson,
      base.marketJson,
      base.asOf,
      scenarioJson,
      method ?? undefined,
      undefined,
      calendarId ?? undefined,
    );
    return {
      name,
      draft,
      scenarioJson,
      method,
      calendarId,
      report,
      explain: native.horizonResultExplainText(report),
    };
  } catch (error) {
    return {
      name,
      draft,
      scenarioJson,
      method,
      calendarId,
      error: { kind: error.kind, message: error.message },
    };
  }
});
await emit(
  new URL("cases.json", import.meta.url),
  serializeHost({
    description:
      "Native horizon total returns of the results-fixture bond: the Rust-built spec, scenarios.computeHorizonReturn (omitted method, config and calendar left to Rust) and horizonResultExplainText, or the native error.",
    sources: [
      {
        path: bondPath,
        sha256: createHash("sha256").update(bondBytes).digest("hex"),
      },
    ],
    base: { asOf: base.asOf },
    cases,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length} native horizon returns.`,
);
