import { createRequire } from "node:module";
import { checking, emit } from "../../../scripts/fixture-output.mjs";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// An illustrative one-year rating migration matrix, AAA to CCC plus an
// absorbing default (rows in the shape of published agency averages; not a
// sourced data set). Each row sums to one.
const labels = ["AAA", "AA", "A", "BBB", "BB", "B", "CCC", "D"];
const rows = [
  [0.9081, 0.0833, 0.0068, 0.0006, 0.0008, 0.0002, 0.0001, 0.0001],
  [0.007, 0.9065, 0.0779, 0.0064, 0.0006, 0.001, 0.0004, 0.0002],
  [0.0009, 0.0227, 0.9105, 0.0552, 0.0074, 0.0021, 0.0006, 0.0006],
  [0.0002, 0.0033, 0.0595, 0.8693, 0.053, 0.0102, 0.0027, 0.0018],
  [0.0003, 0.0014, 0.0067, 0.0773, 0.8053, 0.0884, 0.01, 0.0106],
  [0, 0.0011, 0.0024, 0.0043, 0.0648, 0.8346, 0.0407, 0.0521],
  [0.0022, 0, 0.0022, 0.013, 0.0238, 0.1124, 0.6486, 0.1978],
  [0, 0, 0, 0, 0, 0, 0, 1],
];
// roundTripTol is omitted: Rust's default tolerance applies.
const request = { labels, rows, horizon: 1, horizons: [1, 2, 3, 5, 7, 10] };
const optional = (value) => value ?? undefined;
const orNull = (value) => (value === undefined ? null : Array.from(value));
// The worker's ModelsApi.migrationMatrix and migrationProjection over the raw
// package (models.credit.migration re-exports these raw classes unchanged).
function scaleOf(matrix) {
  const scale = matrix.scale;
  try {
    return {
      labels: scale.labels(),
      defaultState: scale.defaultState() ?? null,
    };
  } finally {
    scale.free();
  }
}
function migrationMatrix({ labels, rows, horizon }) {
  const matrix = native.TransitionMatrix.fromLabeled(
    labels,
    labels,
    rows,
    horizon,
  );
  try {
    return {
      ...scaleOf(matrix),
      horizon: matrix.horizon,
      rows: matrix.toRows(),
      defaultProbabilities: orNull(matrix.defaultProbabilities()),
    };
  } finally {
    matrix.free();
  }
}
function migrationProjection(request) {
  const matrix = native.TransitionMatrix.fromLabeled(
    request.labels,
    request.labels,
    request.rows,
    request.horizon,
  );
  let generator;
  try {
    generator = native.GeneratorMatrix.fromTransitionMatrix(
      matrix,
      optional(request.roundTripTol),
    );
    return {
      ...scaleOf(matrix),
      generator: {
        rows: generator.toRows(),
        roundTripError: generator.roundTripError,
        regularizationL1: generator.regularizationL1,
      },
      projections: request.horizons.map((t) => {
        const projected = native.project(generator, t);
        try {
          return {
            horizon: projected.horizon,
            defaultProbabilities: orNull(projected.defaultProbabilities()),
          };
        } finally {
          projected.free();
        }
      }),
    };
  } finally {
    generator?.free();
    matrix.free();
  }
}
// fixture-output writes null for NaN and ±Infinity, so the pinned values must be finite.
function assertFinite(value, path) {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error(`${path} is ${value}`);
  } else if (value && typeof value === "object")
    for (const [key, child] of Object.entries(value))
      assertFinite(child, `${path}.${key}`);
}
const matrix = migrationMatrix(request);
const projection = migrationProjection(request);
assertFinite({ matrix, projection }, "migration");
await emit(
  new URL("migration.json", import.meta.url),
  JSON.stringify({
    description:
      "Native models.credit.migration results for an illustrative eight-state one-year matrix (AAA to CCC plus an absorbing default): the transition matrix, its regularized generator (Rust default round-trip tolerance) and cumulative default probabilities projected to each horizon.",
    request,
    matrix,
    projection,
  }),
);
const terms = { horizons: [0.5, 1, 2, 3, 5, 10], maturity: 5, recovery: 0.4 };
const mertonCases = {
  // The facade example firm: assets 100, asset volatility 25%, debt barrier
  // 60, rate 3%, no payout, terminal barrier and lognormal assets.
  model: {
    source: {
      kind: "model",
      model: {
        asset_value: 100,
        asset_vol: 0.25,
        debt_barrier: 60,
        risk_free_rate: 0.03,
        payout_rate: 0,
        barrier_type: "terminal",
        dynamics: "geometric_brownian",
      },
    },
    ...terms,
  },
  // A KMV calibration: equity 50 at 40% volatility, short-term debt 30 and
  // long-term debt 40 (default point 30 + 40 / 2), rate 3%, one-year horizon.
  equity: {
    source: {
      kind: "equity",
      equity_value: 50,
      equity_vol: 0.4,
      short_term_debt: 30,
      long_term_debt: 40,
      risk_free_rate: 0.03,
      payout_rate: 0,
      maturity: 1,
    },
    ...terms,
  },
};
const merton = Object.fromEntries(
  Object.entries(mertonCases).map(([name, request]) => [
    name,
    { request, report: native.mertonReport(request) },
  ]),
);
assertFinite(merton, "merton");
await emit(
  new URL("merton.json", import.meta.url),
  JSON.stringify({
    description:
      "Native models.credit.MertonModel reports at horizons 0.5 to 10 years with a 5-year CDS at 40% recovery, graded on models.credit.pd.MasterScale.defaultCalibration(): the facade example firm (assets 100, asset volatility 0.25, barrier 60, rate 0.03) as MertonModel JSON, and a KMV equity calibration (equity 50 at volatility 0.4, short-term debt 30, long-term debt 40, rate 0.03, one year).",
    ...merton,
  }),
);
// The worker's ModelsApi.creditScore over the raw package
// (models.credit.scoring re-exports these raw functions unchanged): each
// function's positional arguments in the facade order, then the default master
// scale's mapScore of the result (Rust rejects a result without an implied PD).
const scoringArguments = {
  altmanZScore: [
    "working_capital_to_total_assets",
    "retained_earnings_to_total_assets",
    "ebit_to_total_assets",
    "market_equity_to_total_liabilities",
    "sales_to_total_assets",
  ],
  altmanZPrime: [
    "working_capital_to_total_assets",
    "retained_earnings_to_total_assets",
    "ebit_to_total_assets",
    "book_equity_to_total_liabilities",
    "sales_to_total_assets",
  ],
  altmanZDoublePrime: [
    "working_capital_to_total_assets",
    "retained_earnings_to_total_assets",
    "ebit_to_total_assets",
    "book_equity_to_total_liabilities",
  ],
  altmanEmScore: [
    "working_capital_to_total_assets",
    "retained_earnings_to_total_assets",
    "ebit_to_total_assets",
    "book_equity_to_total_liabilities",
  ],
  ohlsonOScore: [
    "log_total_assets_adjusted",
    "total_liabilities_to_total_assets",
    "working_capital_to_total_assets",
    "current_liabilities_to_current_assets",
    "liabilities_exceed_assets",
    "net_income_to_total_assets",
    "funds_from_operations_to_total_liabilities",
    "negative_net_income_two_years",
    "net_income_change",
  ],
  zmijewskiScore: [
    "net_income_to_total_assets",
    "total_liabilities_to_total_assets",
    "current_assets_to_current_liabilities",
  ],
};
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
function creditScore({ model, input }) {
  const result = native[model](
    ...scoringArguments[model].map((field) => input[field]),
  );
  const scale = native.MasterScale.defaultCalibration();
  try {
    return { model, result, grade: envelope(() => scale.mapScore(result)) };
  } finally {
    scale.free();
  }
}
// One case per model, each the input of a Rust unit test in
// finstack-quant/models/src/credit/scoring/tests.rs (the pinned score in
// brackets): z_score_safe_zone (3.595), z_prime_private_firm (2.62415),
// z_double_prime_non_manufacturer (3.366), em_score_is_z_double_prime_plus_constant
// (7.808), o_score_healthy_firm and zmijewski_healthy_firm.
const scoringCases = {
  altmanZScore: {
    working_capital_to_total_assets: 0.1,
    retained_earnings_to_total_assets: 0.2,
    ebit_to_total_assets: 0.15,
    market_equity_to_total_liabilities: 1.5,
    sales_to_total_assets: 1.8,
  },
  altmanZPrime: {
    working_capital_to_total_assets: 0.1,
    retained_earnings_to_total_assets: 0.2,
    ebit_to_total_assets: 0.15,
    book_equity_to_total_liabilities: 1,
    sales_to_total_assets: 1.5,
  },
  altmanZDoublePrime: {
    working_capital_to_total_assets: 0.1,
    retained_earnings_to_total_assets: 0.2,
    ebit_to_total_assets: 0.15,
    book_equity_to_total_liabilities: 1,
  },
  altmanEmScore: {
    working_capital_to_total_assets: 0.2,
    retained_earnings_to_total_assets: 0.3,
    ebit_to_total_assets: 0.15,
    book_equity_to_total_liabilities: 1.2,
  },
  ohlsonOScore: {
    log_total_assets_adjusted: 8,
    total_liabilities_to_total_assets: 0.4,
    working_capital_to_total_assets: 0.2,
    current_liabilities_to_current_assets: 0.5,
    liabilities_exceed_assets: 0,
    net_income_to_total_assets: 0.1,
    funds_from_operations_to_total_liabilities: 0.3,
    negative_net_income_two_years: 0,
    net_income_change: 0.1,
  },
  zmijewskiScore: {
    net_income_to_total_assets: 0.1,
    total_liabilities_to_total_assets: 0.4,
    current_assets_to_current_liabilities: 2,
  },
};
const scorecard = Object.fromEntries(
  Object.entries(scoringCases).map(([model, input]) => {
    const request = { model, input };
    return [model, { request, score: creditScore(request) }];
  }),
);
assertFinite(scorecard, "scorecard");
await emit(
  new URL("scorecard.json", import.meta.url),
  JSON.stringify({
    description:
      "Native models.credit.scoring results, one per model, each graded with models.credit.pd.MasterScale.defaultCalibration().mapScore (Altman results carry no implied PD, so their grade is Rust's error). Inputs are those of the Rust unit tests in finstack-quant/models/src/credit/scoring/tests.rs.",
    ...scorecard,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} the native migration matrix and projection, two Merton reports and six credit scores.`,
);
