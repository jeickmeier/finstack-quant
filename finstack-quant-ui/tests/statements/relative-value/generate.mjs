// Native statements_analytics.relativeValueReport results and a rejection for
// relative-value-table, over a host-supplied universe of ten issuers in two
// sectors. Rerun with `mise run ui-fixtures` after rebuilding the WASM package.
import { createRequire } from "node:module";
import { emit } from "../../../scripts/fixture-output.mjs";

const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const rejection = (run) => {
  try {
    run();
  } catch (error) {
    return error.message;
  }
  throw new Error("Rust accepted a request it should reject");
};
/** One issuer's CompanyMetrics serde document; omitted metrics are missing. */
const company = (id, sector, rating, metrics) => ({
  id,
  attributes: { meta: { gics_sector: sector, rating } },
  ...metrics,
});
// Monetary metrics in one currency (USD millions); spreads in basis points.
const subject = company("NORTHWIND", "Energy", "BB", {
  enterprise_value: 5400,
  market_cap: 2600,
  revenue: 6100,
  ebitda: 720,
  net_income: 210,
  oas_bp: 365,
  leverage: 3.9,
});
const universe = [
  company("ALDER", "Energy", "BB", {
    enterprise_value: 4800,
    market_cap: 2500,
    revenue: 5200,
    ebitda: 690,
    net_income: 230,
    oas_bp: 290,
    leverage: 3.1,
    interest_coverage: 5.2,
  }),
  company("BIRCH", "Energy", "B", {
    enterprise_value: 3900,
    market_cap: 1500,
    revenue: 4700,
    ebitda: 520,
    net_income: 120,
    oas_bp: 455,
    leverage: 4.6,
    interest_coverage: 3.1,
  }),
  company("CEDAR", "Energy", "BB", {
    enterprise_value: 7200,
    market_cap: 3900,
    revenue: 7900,
    ebitda: 980,
    net_income: 360,
    oas_bp: 250,
    leverage: 2.8,
    interest_coverage: 6.0,
  }),
  company("DOGWOOD", "Utilities", "BBB", {
    enterprise_value: 9800,
    market_cap: 5200,
    revenue: 4100,
    ebitda: 1150,
    net_income: 420,
    oas_bp: 160,
    leverage: 4.1,
    interest_coverage: 4.4,
  }),
  company("ELM", "Energy", "B", {
    enterprise_value: 2600,
    market_cap: 900,
    revenue: 3300,
    ebitda: 340,
    net_income: 60,
    oas_bp: 540,
    leverage: 5.3,
    interest_coverage: 2.4,
  }),
  company("FIR", "Energy", "BB", {
    enterprise_value: 6100,
    market_cap: 3100,
    revenue: 6800,
    ebitda: 800,
    net_income: 270,
    oas_bp: 330,
    leverage: 3.6,
    interest_coverage: 4.7,
  }),
  company("GINKGO", "Energy", "BB", {
    enterprise_value: 5000,
    market_cap: 2300,
    revenue: 5600,
    ebitda: 610,
    // No reported earnings: its P/E is missing, not zero.
    oas_bp: 395,
    leverage: 4.2,
    interest_coverage: 3.9,
  }),
  company("HAZEL", "Energy", "B", {
    enterprise_value: 3300,
    market_cap: 1200,
    revenue: 4000,
    ebitda: 430,
    net_income: 90,
    oas_bp: 480,
    // No leverage reported: it stays in the table and the Y statistics,
    // and drops from the regression scatter.
    interest_coverage: 2.9,
  }),
  company("IRONWOOD", "Utilities", "BBB", {
    enterprise_value: 11200,
    market_cap: 6000,
    revenue: 4600,
    ebitda: 1300,
    net_income: 480,
    oas_bp: 145,
    leverage: 4.3,
    interest_coverage: 4.6,
  }),
  company("JUNIPER", "Energy", "BB", {
    enterprise_value: 4400,
    market_cap: 2100,
    revenue: 5000,
    ebitda: 560,
    net_income: 180,
    oas_bp: 345,
    leverage: 3.7,
    interest_coverage: 4.1,
  }),
];
const spreadOnLeverage = {
  label: "Spread vs leverage",
  y_extractor: { named: "oas_bp" },
  x_extractor: { named: "leverage" },
  weight: 0.6,
  direction: "higher_is_cheap",
};
const evEbitda = {
  label: "EV/EBITDA",
  y_extractor: { multiple: "ev_ebitda" },
  weight: 0.4,
  direction: "higher_is_rich",
};
// The subject reports no interest coverage, so this regression has no subject X.
const spreadOnCoverage = {
  label: "Spread vs coverage",
  y_extractor: { named: "oas_bp" },
  x_extractor: { named: "interest_coverage" },
  weight: 0.3,
  direction: "higher_is_cheap",
};
// A partial filter: the omitted PeerFilter fields screen on nothing.
const full = {
  filter: { gics_sectors: ["Energy"] },
  period_basis: "ltm",
  dimensions: [spreadOnLeverage, evEbitda],
  multiples: ["ev_ebitda", "ev_revenue", "pe", "spread_per_turn"],
};
const report = (config) => ({
  config,
  report: native.relativeValueReport(subject, universe, config),
});
const cases = {
  // Eight energy peers; both dimensions score.
  full: report(full),
  // A third dimension the subject cannot be placed on, listed with its reason.
  excludedDimension: report({
    ...full,
    dimensions: [spreadOnLeverage, evEbitda, spreadOnCoverage],
  }),
  // One utility under the cap: no dispersion and no regression, so no composite.
  tooFewPeers: report({
    ...full,
    filter: { gics_sectors: ["Utilities"], market_cap_max: 5500 },
    period_basis: { custom: "FY2025E" },
  }),
};
const typo = { ...full, filter: { gics_sector: ["Energy"] } };
const errors = {
  // A misspelt PeerFilter field is rejected, not dropped.
  typo: {
    config: typo,
    message: rejection(() =>
      native.relativeValueReport(subject, universe, typo),
    ),
  },
};
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({ subject, universe, cases, errors }),
);
