// Native statements_analytics.capitalStructureSummary results and rejections
// for capital-structure-report, over a quarterly model with a debt stack in two
// payment classes. Rerun with `mise run ui-fixtures` after rebuilding the WASM
// package.
import { serializeHost } from "../../../src/codec.mjs";
import { emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoJson } from "../../../scripts/fixture-inputs.mjs";

const rejection = (run) => {
  try {
    run();
  } catch (error) {
    return error.message;
  }
  throw new Error("Rust accepted a request it should reject");
};
/** A fixed-coupon, semi-annual 30/360 bond of the capital structure. */
const bond = (id, amount, rate, issue, maturity) => ({
  id,
  spec: {
    type: "bond",
    spec: {
      id,
      notional: { amount, currency: "USD" },
      issue_date: issue,
      maturity,
      cashflow_spec: {
        fixed: {
          coupon_type: "cash",
          rate,
          frequency: { count: 6, unit: "months" },
          day_count: "30_360",
          business_day_convention: "modified_following",
          calendar_id: "usny",
          stub: "short_front",
          end_of_month: false,
          payment_lag_days: 0,
        },
      },
      quote_compounding: { periodic: 2 },
      discount_curve_id: "USD-OIS",
      credit_curve_id: null,
      call_put: null,
      attributes: {},
      settlement_days: 1,
      ex_coupon_days: 0,
      ex_coupon_calendar_id: "usny",
    },
  },
});
const quarters = ["2025Q1", "2025Q2", "2025Q3", "2025Q4"];
const starts = ["2025-01-01", "2025-04-01", "2025-07-01", "2025-10-01"];
const ends = ["2025-04-01", "2025-07-01", "2025-10-01", "2026-01-01"];
const values = (amounts) =>
  Object.fromEntries(quarters.map((period, i) => [period, amounts[i]]));
/**
 * Four quarters of single-period EBITDA and period-end cash, two first-lien
 * bonds and a second-lien bond (the cross-host capital structure of
 * 162e15d4d3 without its swap, so it evaluates without a market).
 */
const modelWithCash = (cash) => ({
  id: "capital-structure",
  periods: quarters.map((id, i) => ({
    id,
    start: starts[i],
    end: ends[i],
    is_actual: false,
  })),
  nodes: {
    ebitda: {
      node_id: "ebitda",
      node_type: "value",
      values: values([100000, 110000, 120000, 130000]),
      value_type: { type: "scalar" },
    },
    cash: {
      node_id: "cash",
      node_type: "value",
      values: values(cash),
      value_type: { type: "scalar" },
    },
  },
  capital_structure: {
    debt_instruments: [
      bond("SUB", "420000", "0.08", "2024-01-01", "2031-03-01"),
      bond("SR-A", "400000", "0.05", "2024-01-01", "2029-06-01"),
      bond("SR-B", "100000", "0.05", "2024-01-15", "2029-01-15"),
    ],
    waterfall: {
      available_cash_node: "cash",
      priority_of_payments: ["fees", "interest", "amortization"],
      payment_classes: [
        { id: "2L", rank: 1, instrument_ids: ["SUB"] },
        { id: "1L", rank: 0, instrument_ids: ["SR-A", "SR-B"] },
      ],
    },
  },
  schema_version: 1,
});
const evaluate = (model) => {
  const modelJson = native.validateFinancialModelJson(JSON.stringify(model));
  const evaluator = new native.Evaluator();
  try {
    return {
      modelJson,
      resultJson: serializeHost(evaluator.evaluate(modelJson)),
    };
  } finally {
    evaluator.free();
  }
};
const { modelJson, resultJson } = evaluate(
  modelWithCash([150000, 150000, 150000, 150000]),
);
const summary = (config, evaluated = { modelJson, resultJson }) => ({
  config,
  summary: native.capitalStructureSummary(
    evaluated.resultJson,
    evaluated.modelJson,
    JSON.stringify(config),
  ),
});
// Cash above total debt: net debt and net leverage are negative.
const netCash = evaluate(modelWithCash([980000, 980000, 980000, 980000]));
// The statements workbench's analyst model with its debt stack removed.
const noCapitalStructure = evaluate({
  ...(await readRepoJson(
    "finstack-quant-ui/src/fixtures/statements/analyst-model.json",
  )),
  capital_structure: null,
});
const full = { period: "2025Q4", ebitda_node: "ebitda", cash_node: "cash" };
const cases = {
  // A full trailing year: every ratio is returned.
  full: summary(full),
  // Three of four quarters: Rust returns no LTM EBITDA and null ratios.
  shortWindow: summary({ ...full, period: "2025Q3" }),
  // No cash node: no cash, net debt or net leverage.
  noCash: summary({ period: "2025Q4", ebitda_node: "ebitda" }),
  netCash: summary(full, netCash),
};
const errors = {
  missingNode: {
    config: { ...full, ebitda_node: "adjusted_ebitda" },
    message: rejection(() =>
      native.capitalStructureSummary(
        resultJson,
        modelJson,
        JSON.stringify({ ...full, ebitda_node: "adjusted_ebitda" }),
      ),
    ),
  },
  // The analyst model with `capital_structure: null`.
  noCapitalStructure: {
    config: full,
    model: JSON.parse(noCapitalStructure.modelJson),
    result: JSON.parse(noCapitalStructure.resultJson),
    message: rejection(() =>
      native.capitalStructureSummary(
        noCapitalStructure.resultJson,
        noCapitalStructure.modelJson,
        JSON.stringify(full),
      ),
    ),
  },
};
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    model: JSON.parse(modelJson),
    result: JSON.parse(resultJson),
    cases,
    errors,
  }),
);
