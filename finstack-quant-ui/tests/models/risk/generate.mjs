import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoJson } from "../../../scripts/fixture-inputs.mjs";
// The worker's ModelsApi.positionRisk over the raw package
// (models.factor.risk re-exports these raw functions unchanged).
function envelope(call) {
  try {
    return { ok: true, value: call() };
  } catch (error) {
    // The worker's errorValue shape for a facade FinstackError.
    return {
      ok: false,
      error: { name: error.name, message: error.message, kind: error.kind },
    };
  }
}
const optional = (value) => value ?? undefined;
function positionRisk({ positionIds, confidence, source, budget }) {
  const decomposition =
    source.kind === "parametric"
      ? native.parametricVarDecomposition(
          positionIds,
          source.weights,
          source.covariance,
          optional(confidence),
          optional(source.computeIncremental),
        )
      : native.historicalVarDecomposition(
          positionIds,
          source.pnls,
          optional(confidence),
        );
  return {
    decomposition,
    budget:
      budget == null
        ? null
        : envelope(() =>
            native.evaluateRiskBudgetFromDecomposition(
              decomposition,
              budget.targets,
              optional(budget.utilizationThreshold),
            ),
          ),
    stress:
      source.kind === "historical"
        ? envelope(() =>
            native.buildStressAttribution(
              positionIds,
              source.pnls,
              optional(confidence),
            ),
          )
        : null,
  };
}
// fixture-output writes null for NaN and ±Infinity, so the pinned values must be finite.
function assertFinite(value, path) {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error(`${path} is ${value}`);
  } else if (value && typeof value === "object")
    for (const [key, child] of Object.entries(value))
      assertFinite(child, `${path}.${key}`);
}
// The three-sleeve inputs of finstack-quant-py/examples/notebooks/05_portfolio/
// portfolio_risk_decomposition.ipynb: weights, annualised return covariance,
// 99% parametric VaR with incremental VaR and a 50/30/20 risk budget at a
// 1.10 utilization threshold; and its deterministic 250-scenario P&L (the
// credit sleeve carries a six-scenario left tail) at 95%, budgeted against the
// same targets with Rust's default threshold.
const positionIds = ["Equity", "Credit", "Rates"];
const targets = [0.5, 0.3, 0.2];
function scenarioPnls(n = 250) {
  const equity = [],
    credit = [],
    rates = [];
  for (let i = 0; i < n; i++) {
    equity.push(0.02 * Math.sin(i / 7) - 0.001);
    credit.push(i < 6 ? -0.08 : 0.004 * Math.cos(i / 5) + 0.001);
    rates.push(0.005 * Math.cos(i / 11));
  }
  return [equity, credit, rates];
}
const cases = {
  parametric: {
    positionIds,
    confidence: 0.99,
    source: {
      kind: "parametric",
      weights: [0.5, 0.3, 0.2],
      covariance: [
        [0.04, 0.012, -0.002],
        [0.012, 0.02, 0.001],
        [-0.002, 0.001, 0.0064],
      ],
      computeIncremental: true,
    },
    budget: { targets, utilizationThreshold: 1.1 },
  },
  historical: {
    positionIds,
    confidence: 0.95,
    source: { kind: "historical", pnls: scenarioPnls() },
    budget: { targets },
  },
};
const reports = Object.fromEntries(
  Object.entries(cases).map(([name, request]) => [
    name,
    { request, report: positionRisk(request) },
  ]),
);
// The worker's PortfolioApi.computeFactorSensitivities (over the cached
// Portfolio and MarketContext handles) and ModelsApi.decomposeFactors of the
// returned matrix's JSON, as the useFactorRisk chain calls them, over the raw
// package (portfolio and models.factor.risk re-export these unchanged).
function factorRisk(request) {
  const book = native.Portfolio.fromSpec(request.specJson);
  const market = native.MarketContext.fromJson(request.marketJson);
  try {
    const sensitivities = native.computeFactorSensitivities(
      book,
      request.factorsJson,
      market,
      request.asOf,
      optional(request.bumpConfigJson),
    );
    return {
      sensitivities,
      decomposition: native.decomposeFactors(
        JSON.stringify(sensitivities),
        request.covarianceJson,
        optional(request.riskMeasureJson),
      ),
    };
  } finally {
    book.free();
    market.free();
  }
}
// The Py/WASM host-parity book (two USD deposits) and market at t0 with its
// factor_model_config: one usd_rates factor (parallel USD-OIS shift in bp)
// and a 1x1 covariance; the risk measure is left to Rust (variance).
const { inputs } = await readRepoJson(
  "finstack-quant/portfolio/tests/fixtures/host_parity.json",
);
const factorRequest = {
  specJson: JSON.stringify(inputs.portfolio_spec),
  marketJson: JSON.stringify(inputs.market_t0),
  factorsJson: JSON.stringify(inputs.factor_model_config.factors),
  asOf: inputs.as_of_t0,
  covarianceJson: JSON.stringify(inputs.factor_model_config.covariance),
};
reports.factor = { request: factorRequest, report: factorRisk(factorRequest) };
assertFinite(reports, "risk");
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native models.factor.risk position risk reports for the three-sleeve inputs of the portfolio_risk_decomposition notebook (Equity, Credit, Rates): a 99% parametric decomposition with incremental VaR and a 50/30/20 risk budget at a 1.10 utilization threshold, and a 95% historical decomposition of 250 deterministic scenarios with the same budget (Rust's default threshold) and its tail stress attribution; and the factor sensitivities and variance decomposition of the Py/WASM host-parity book (two USD deposits, market at t0) under its one-factor usd_rates model.",
    ...reports,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} the parametric and historical position risk reports and the host-parity factor decomposition.`,
);
