import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import ts from "typescript";
import { schemaAt } from "../src/schema.mjs";
import { byCodeUnit } from "./order.mjs";

const base = "https://finstack_quant.dev/schemas/";
const market = `${base}market_data/1/market_context_state.schema.json`;
const result = `${base}results/1/valuation_result.schema.json`;
const calibration = `${base}calibration/1/calibration.schema.json`;
const price = [
  "valuations.instruments.priceInstrument",
  "ValuationInstrumentsNamespace",
  "priceInstrument",
];
const metricMetadata = [
  "valuations.instruments.metricMetadata",
  "ValuationInstrumentsNamespace",
  "metricMetadata",
];
const displays = [
  {
    id: "instrument-form",
    schema: "instrument-roots",
    pointer: "#",
    route: "schema",
    convention: "Canonical wire representations; no inferred units.",
  },
  {
    id: "market-context-form",
    schema: market,
    pointer: "#",
    route: "schema",
    convention: "Stored state; no curve reconstruction.",
  },
  {
    id: "calibration-form",
    schema: calibration,
    pointer: "#",
    route: "schema",
    convention:
      "Calibration inputs only; final_market is a returned market state.",
  },
  {
    id: "valuation-summary",
    schema: result,
    pointer: "#/properties/value",
    api: price,
    route: "typed",
    convention: "Exact decimal-string money with its returned currency.",
  },
  {
    id: "stamp-badge",
    schema: result,
    pointer: "#/properties/meta",
    api: price,
    route: "raw",
    convention: "Returned numeric, rounding, FX policy and version stamps.",
  },
  {
    id: "measures-grid",
    schema: result,
    pointer: "#/properties/measures",
    api: metricMetadata,
    route: "native-metadata",
    convention:
      "Returned values and original keys unchanged; units, decoded coordinates, and groups come from Rust.",
  },
  {
    id: "valuation-details",
    schema: result,
    pointer: "#/$defs/ValuationDetails",
    api: price,
    route: "host-declaration",
    convention:
      "All detail variants use Rust-derived WASM host declarations and validation metadata.",
  },
  {
    id: "explanation-trace",
    schema: result,
    pointer: "#/properties/explanation",
    api: price,
    route: "raw",
    convention: "Returned trace; no reconciliation or financial arithmetic.",
  },
  {
    id: "covenant-report",
    schema: result,
    pointer: "#/properties/covenants",
    api: price,
    route: "typed",
    convention:
      "Returned CovenantReport fields keyed by covenant instance key, in the order supplied: test description, actual, threshold and headroom in the metric's own units, the passed verdict and details; unset fields shown as a dash, complete reports kept as JSON. No verdict, headroom or threshold is computed.",
  },
  {
    id: "covenant-compliance",
    schema: `${base}covenants/1/dated_covenant_reports.schema.json`,
    pointer: "#",
    api: [
      "statements_analytics.evaluateCovenants",
      "StatementsAnalyticsNamespace",
      "evaluateCovenants",
    ],
    route: "typed",
    convention:
      "Returned DatedCovenantReports rows in input order and their verdicts; a covenant without a report on a date shows a dash. Rust evaluates every statement result period at its end date with the covenants applicable on that date, reading a metric only where an applicable covenant needs it, so covenant windows decide which metrics a date requires; the component builds no rows.",
  },
  {
    id: "covenant-engine-editor",
    schema: `${base}covenants/1/covenant_engine.schema.json`,
    pointer: "#",
    route: "schema",
    convention:
      "Generated covenant_engine form (the caller's engine; Rust publishes no example), optionally replaced by the specs of a Rust package template (covenants.covLite, lboStandard, realEstate or projectFinance over typed thresholds that start empty, a blank one sent as NaN); metric ids are free text with the model's node ids as suggestions, and each spec's metric id is listed with whether the model has that node, without inferring an unset one; validated natively by validateCovenantEngineJson.",
  },
  {
    id: "covenant-breach-forecast",
    schema: `${base}covenants/1/future_breach.schema.json`,
    pointer: "#",
    api: [
      "statements_analytics.forecastBreaches",
      "StatementsAnalyticsNamespace",
      "forecastBreaches",
    ],
    route: "typed",
    convention:
      "Returned FutureBreach rows in the order supplied (Rust sorts by breach date, then covenant id) with every supplied digit: projected value, threshold and headroom in the metric's own units and the breach probability as a fraction. Rust takes every statement result period as a test date, honours windows, waivers and threshold schedules, skips non-numeric covenants and starts the horizon the day before the first period; the generated covenant_forecast_config form is passed through as JSON text for Rust to validate. Nothing is forecast, filtered or sorted in the UI.",
  },
  {
    id: "cashflow-viewer",
    schema: `${base}results/1/instrument_cashflow.schema.json`,
    pointer: "#",
    api: [
      "valuations.instruments.instrumentCashflows",
      "ValuationInstrumentsNamespace",
      "instrumentCashflows",
    ],
    route: "native-envelope-table",
    convention:
      "The returned envelope object, validated against the Rust-generated cashflow schema. Native row currency and reporting-currency PV; Rust supplies total and reconciliation. beginning_balance and ending_balance show as supplied (per row for bonds, term loans and revolvers, per period for agency MBS), never accumulated, and their columns hide only when no row carries one. Its JSON view is a host serialization, not native text. The optional annual ladder is cashflows.calendarYearLadder per row currency (worker partition, first-appearance order) over each flow's native_pv: row-currency PV before FX conversion and any scenario price shock.",
  },
  {
    id: "curve-chart",
    schema: market,
    pointer: "#/$defs/CurveState",
    route: "stored-knots-or-fields",
    convention:
      "Original coordinates; base correlation and parametric variants have no synthetic knots.",
    deferred: "full-state-curve-evaluation",
  },
  {
    id: "vol-surface-chart",
    schema: market,
    pointer: "#/properties/surfaces",
    route: "stored-grid",
    convention:
      "Original nodes and quote convention; no off-grid interpolation.",
    deferred: "off-grid-surface-evaluation",
  },
  {
    id: "vol-cube-explorer-black",
    schema: market,
    pointer: "#/properties/vol_cubes",
    api: ["models.volatility.getCubeVol", "VolatilityNamespace", "getCubeVol"],
    route: "typed",
    convention:
      "Annualized Black volatility as decimal; supply stored cube and explicit expiry, tenor, strike. Fixed-tenor and fixed-expiry slices are Rust-materialized (vol-cube-tenor-slice, vol-cube-expiry-slice).",
  },
  {
    id: "vol-cube-explorer-normal",
    api: [
      "models.volatility.getCubeNormalVol",
      "VolatilityNamespace",
      "getCubeNormalVol",
    ],
    route: "typed",
    convention:
      "Annualized normal volatility in absolute rate units. Normal slices are Rust-materialized (vol-cube-tenor-slice-normal, vol-cube-expiry-slice-normal).",
  },
  {
    id: "vol-cube-tenor-slice",
    api: [
      "models.volatility.materializeCubeTenorSlice",
      "VolatilityNamespace",
      "materializeCubeTenorSlice",
    ],
    route: "returned-surface",
    convention:
      "Returned VolSurface JSON (expiry by strike, Black or shifted-Black quote type with displacements as Rust returns them); fixed tenor in years and absolute strikes in forward-rate units are explicit caller input. Off-node tenors are Rust-interpolated.",
  },
  {
    id: "vol-cube-tenor-slice-normal",
    api: [
      "models.volatility.materializeCubeTenorSliceNormal",
      "VolatilityNamespace",
      "materializeCubeTenorSliceNormal",
    ],
    route: "returned-surface",
    convention:
      "Returned VolSurface JSON of normal volatilities (expiry by strike); fixed tenor and absolute strikes are explicit caller input. Off-node tenors are Rust-interpolated.",
  },
  {
    id: "vol-cube-expiry-slice",
    api: [
      "models.volatility.materializeCubeExpirySlice",
      "VolatilityNamespace",
      "materializeCubeExpirySlice",
    ],
    route: "returned-slice",
    convention:
      "Returned VolCubeExpirySlice JSON (tenor by strike, tenor-major, Black or shifted-Black quote type with displacements as Rust returns them); fixed expiry in years and absolute strikes are explicit caller input. Off-node expiries are Rust-interpolated.",
  },
  {
    id: "vol-cube-expiry-slice-normal",
    api: [
      "models.volatility.materializeCubeExpirySliceNormal",
      "VolatilityNamespace",
      "materializeCubeExpirySliceNormal",
    ],
    route: "returned-slice",
    convention:
      "Returned VolCubeExpirySlice JSON of normal volatilities (tenor by strike); fixed expiry and absolute strikes are explicit caller input. Off-node expiries are Rust-interpolated.",
  },
  {
    id: "vol-cube-node-smile",
    api: ["models.volatility.SabrSmile", "SabrSmile", "generateSmile"],
    route: "returned-smile",
    convention:
      "SabrSmile of one stored cube node (paramsAt, forwardAt, the node expiry): normal volatility when beta < 1e-4, Black otherwise, as returned. validateNoArbitrage takes an explicit continuously compounded decimal rate; repairArbitrage runs only when the caller supplies its passes. Never routed through a surface check.",
  },
  {
    id: "surface-arbitrage-badge",
    api: [
      "models.volatility.checkSurface",
      "VolatilityNamespace",
      "checkSurface",
    ],
    route: "returned-report",
    convention:
      "Returned ArbitrageReport of a stored surface (core.VolSurface.fromJson) with every check enabled and host-supplied forwards in strike units (one broadcast or one per expiry; never inferred, and no request without them); the tolerance is sent only when supplied. Counts by type and severity and the violations as returned, most severe first; a convention Rust does not check (normal, shifted Black, tenor axis) shows its native error as not checked.",
  },
  {
    id: "fx-delta-surface",
    schema: market,
    pointer: "#/properties/fx_delta_vol_surfaces",
    api: [
      "models.volatility.getFxDeltaVol",
      "VolatilityNamespace",
      "getFxDeltaVol",
    ],
    route: "typed",
    convention:
      "Annualized Black volatility as decimal; forward is explicit caller input for evaluated samples. The strike grid is Rust-materialized (fx-strike-grid).",
  },
  {
    id: "fx-strike-grid",
    api: [
      "models.volatility.materializeFxDeltaSurface",
      "VolatilityNamespace",
      "materializeFxDeltaSurface",
    ],
    route: "returned-surface",
    convention:
      "Returned VolSurface JSON (expiry by strike, Rust quote type and interpolation metadata); spot domestic per unit of foreign and continuously compounded decimal rates are explicit caller input, never defaulted or looked up.",
  },
  {
    id: "fx-delta-pillars",
    api: [
      "models.volatility.getFxDeltaPillarVols",
      "VolatilityNamespace",
      "getFxDeltaPillarVols",
    ],
    route: "typed-array",
    convention:
      "Float64Array: ATM, 25-delta put, 25-delta call annualized decimal vols.",
  },
  {
    id: "fx-matrix-grid",
    schema: market,
    pointer: "#/$defs/FxMatrixState",
    route: "stored-fields",
    convention: "Stored quotes only; no implied cross or local FX calculation.",
  },
  {
    id: "market-context-browser",
    schema: market,
    pointer: "#",
    route: "stored-fields",
    convention:
      "Schema-owned market sections; raw values where no unit metadata exists.",
  },
  {
    id: "calibration-report",
    api: ["calibration.calibrate", "CalibrationNamespace", "calibrate"],
    route: "returned-report",
    convention:
      "Returned reports and final_market; preserve diagnostics and error metadata.",
  },
  {
    id: "calibration-fit-chart",
    api: ["calibration.calibrate", "CalibrationNamespace", "calibrate"],
    route: "returned-residuals",
    convention:
      "Residuals per step in solver units; no quote-space repricing or cross-step aggregation.",
    deferred: "quote-space-calibration-fits",
  },
  {
    id: "market-scenario-builder",
    schema: `${base}scenarios/1/scenario_spec.schema.json`,
    pointer: "#",
    api: [
      "scenarios.listBuiltinTemplateMetadata",
      "ScenariosNamespace",
      "listBuiltinTemplateMetadata",
    ],
    route: "schema",
    convention:
      "Canonical ScenarioSpec wire fields; templates, components, composition and added operations are returned by Rust, template metadata is shown as returned and template ids are placeholders the user maps to market ids.",
  },
  {
    id: "market-scenario-report",
    api: ["scenarios.applyScenario", "ScenariosNamespace", "applyScenario"],
    route: "returned-report",
    convention:
      "Returned ApplicationReport counters, applied shocks, time roll with carry and warnings; nothing summed or converted. The workbench prices the shocked market at the returned valuation date with the base model and metrics and shows both valuations; no P&L is computed.",
  },
  {
    id: "horizon-return-report",
    api: [
      "scenarios.computeHorizonReturn",
      "ScenariosNamespace",
      "computeHorizonReturn",
    ],
    route: "returned-report",
    convention:
      "Returned HorizonReport: Rust's summary total and annualized return and factor contributions as decimal fractions of the initial value (null shown as not defined, never 0), horizon calendar days, exact initial and terminal values, horizonResultExplainText, the PnlAttribution and the ApplicationReport. Omitted method, config and calendar are left to Rust; nothing is computed in TypeScript.",
  },
  {
    id: "pnl-attribution-report",
    api: [
      "attribution.attributePnlMany",
      "AttributionNamespace",
      "attributePnlMany",
    ],
    route: "returned-report",
    convention:
      "Returned PnlAttribution: total, mark-to-market and endpoint values and every factor bucket as exact money; the waterfall only from waterfall_steps pv_before/pv_after; the residual with the returned residual_pct and tolerances, with a verdict only from pnlAttributionResidualWithinTolerance; shares only from pnlAttributionPctOfTotal; explain only from pnlAttributionExplainText. Nothing is summed, differenced or converted.",
  },
  {
    id: "scenario-heatmap",
    api: [
      "valuations.instruments.trancheScenarioTable",
      "ValuationInstrumentsNamespace",
      "trancheScenarioTable",
    ],
    route: "returned-measures",
    convention:
      "Returned ScenarioCell measures only, in the units scenarioTableFieldUnits states: Rust clean settlement price in percent of current tranche balance, WAL in years, writedown in the table's currency.",
  },
  {
    id: "tranche-analytics",
    api: [
      "valuations.instruments.trancheMetrics",
      "ValuationInstrumentsNamespace",
      "trancheMetrics",
    ],
    route: "returned-metrics",
    convention:
      "Returned TrancheMetrics fields in the units each Rust field documents; break-even CDR, discount margin and OAS as returned decimals; prices percent of current tranche balance; no price default; cash flows by date only from TrancheCashflows::to_table, with its dates joined and summed in Rust.",
  },
  {
    id: "pool-report",
    api: [
      "valuations.instruments.poolReport",
      "ValuationInstrumentsNamespace",
      "poolReport",
    ],
    route: "returned-report",
    convention:
      "Returned PoolReport fields in the units poolReportFieldUnits states: statistics, Moody's WARF and concentration tables by obligor, industry and rating, in Rust's row order with balance_pct in percent points of total_balance. A null key is shown as missing, never dropped, and a null statistic as missing, never zero; nothing is summed, sorted or grouped here.",
  },
  {
    id: "waterfall-report",
    api: [
      "StructuredCredit#runSimulation",
      "StructuredCredit",
      "runSimulation",
    ],
    route: "returned-report",
    convention:
      "Returned SimulationDiagnostics.periods in payment order, in the deal currency each Money states: the period's PeriodWaterfall, and on expand its tier allocations and payment records in execution order, diversions and non-waterfall payments with their own dates. The is_principal, diverted and had_diversions flags are Rust's. Nothing is summed, joined by date, sorted or classified here.",
  },
  {
    id: "coverage-test-chart",
    api: [
      "StructuredCredit#runSimulation",
      "StructuredCredit",
      "runSimulation",
    ],
    route: "returned-report",
    convention:
      "Returned SimulationDiagnostics.first_breaches in Rust's order and, for the selected test, periods[].coverage_tests' ratio, trigger_level, cushion and passing on the dates Rust recorded them, in the units simulationFieldUnits states. The first breach date, cushion and pass or fail are Rust's. A test recorded on fewer dates than the deal has periods states the counts returned, and a null ratio or cushion is shown as no claim outstanding with no dot. waterfall.diverted_cash and diverted_amounts are shown for the deal, not attributed to a test. Nothing is computed, joined by date, sorted or classified here.",
  },
  {
    id: "credit-enhancement-chart",
    api: [
      "StructuredCredit#runSimulation",
      "StructuredCredit",
      "runSimulation",
    ],
    route: "returned-report",
    convention:
      "Returned periods[].credit_enhancement by tranche, a decimal share of the collateral beneath it at period end, and periods[].excess_spread, realized that period and annualized as a decimal of the opening pool balance (pool interest less debt coupons due, waterfall fees and net charge-offs), in the units simulationFieldUnits states. One period is listed, not charted. Nothing is computed, grouped, joined by date, sorted or classified here.",
  },
  {
    id: "equity-tranche-card",
    api: [
      "StructuredCredit#equityMetrics",
      "StructuredCredit",
      "equityMetrics",
    ],
    route: "returned-report",
    convention:
      "Returned EquityMetrics in the units equityMetricsFieldUnits states: invested in the deal currency; irr an annual XIRR decimal on ACT/365F, or null when no rate solves, stated rather than drawn as zero; moic total distributions over invested; nav_pct distributions discounted on the deal's discount curve as a percent of current balance; cash_on_cash each distribution as an unannualized decimal share of invested. Rust chooses the equity tranche and returns null for moic and cash_on_cash when nothing is invested, stated rather than drawn as zero; a returned zero is drawn as zero. A blank purchase price is omitted, so Rust's par default applies. Nothing is computed, rescaled or classified here.",
  },
  {
    id: "performance-report",
    api: ["analytics.Performance#summary", "Performance", "summary"],
    route: "returned-analytics",
    deferred: "machine-readable-units",
    convention:
      "Returned analytics.Performance results of a host-supplied panel: the summary TableEnvelope, per-ticker series against each ticker's own active dates, rolling series, monthly and annual periodic returns, drawdown episodes and lookback returns (FYTD from a core.FiscalConfig only when a fiscal-year start is supplied). Decimal fractions and ratios as returned, no unit inferred; omitted options are left to Rust; non-finite values are left out of figures. The monthly heatmap colour domain is the caller's or the returned values' extent (presentation only).",
  },
  {
    id: "benchmark-report",
    api: ["analytics.Performance#greeks", "Performance", "greeks"],
    route: "returned-analytics",
    deferred: "machine-readable-units",
    convention:
      "Returned analytics.Performance benchmark-relative results of a host-supplied panel against a named benchmark: greeks, beta with its confidence bounds, tracking error, information ratio, Treynor, M squared, captures and batting average per ticker; outperformance and drawdown difference against each ticker's own active dates; rollingGreeks against the returned window ends; multiFactorGreeks per ticker (or that ticker's native error) only when a factor panel is supplied. Decimal fractions and ratios as returned, alphas annualized by Rust, no unit inferred; omitted options are left to Rust; non-finite values are left out of figures. The default fund is the first ticker that is not the benchmark (presentation only).",
  },
  {
    id: "portfolio-summary",
    api: ["portfolio.valuePortfolio", "PortfolioNamespace", "valuePortfolio"],
    route: "returned-valuation",
    convention:
      "Returned PortfolioValuation: total_base_currency and by_entity as exact returned money (entity rows in Portfolio#entityIds registration order), the stamped fx_collapse_policy shown as text, degraded_positions with each risk_error; nothing summed or converted.",
  },
  {
    id: "positions-table",
    api: ["portfolio.valuePortfolio", "PortfolioNamespace", "valuePortfolio"],
    route: "returned-valuation",
    deferred: "machine-readable-units",
    convention:
      "Returned PositionValue rows in Portfolio#positionIds (spec) order: native and base money as returned, the applied fx_rate, risk completeness and inapplicable_metrics; the selected position's valuation_result through valuation-summary and measures-grid with Rust metricMetadata. Omitted metrics and strictRisk are left to Rust.",
  },
  {
    id: "portfolio-metrics",
    api: [
      "portfolio.aggregateMetrics",
      "PortfolioNamespace",
      "aggregateMetrics",
    ],
    route: "returned-metrics",
    deferred: "machine-readable-units",
    convention:
      "Returned PortfolioMetrics: aggregated totals with by-entity values, raw by-position metrics with their native currency, unaggregated_metrics, skipped_metrics and degraded_positions as returned. The portfolioMetricsSeries chart keeps Rust's (lexical) entry order, grouped by the first key component; no tenor sort while tenor ordering is an owner decision.",
  },
  {
    id: "portfolio-margin-report",
    schema: `${base}portfolio/1/portfolio_margin_result.schema.json`,
    pointer: "#",
    api: ["portfolio.portfolioMargin", "PortfolioNamespace", "portfolioMargin"],
    route: "returned-report",
    convention:
      "Returned PortfolioMarginResult: every total as exact base-currency money (including a -0 variation margin, as returned), total_positions and positions_without_margin as returned counts, netting sets in returned order with the returned is_approximate flag beside an always-shown Indicative badge, by_csa collateral in the netting sets' CSA order, degraded positions, and the selected set's im_breakdown and SIMM sensitivities. Omitted collateral is left to Rust; nothing summed, netted or converted.",
  },
  {
    id: "scenario-pnl-table",
    schema: `${base}portfolio/1/scenario_pnl_batch_item.schema.json`,
    pointer: "#",
    api: [
      "portfolio.scenarioPnlBatch",
      "PortfolioNamespace",
      "scenarioPnlBatch",
    ],
    route: "returned-pnl",
    convention:
      "Returned ScenarioPnlBatchItem per scenario in batch order: by_position money in Portfolio#positionIds (spec) order, the total row from pnl.total in the base currency, the totals charted as returned and each ApplicationReport; nothing summed or differenced. The batch fails whole on the earliest failing scenario.",
  },
  {
    id: "merton-calculator",
    api: ["models.credit.mertonReport", "MertonModel", "defaultProbabilities"],
    route: "returned-model",
    convention:
      "Returned models.credit.MertonModel results of a model from canonical MertonModel JSON (natively validated) or from a KMV equity calibration Rust performs: default probabilities, distances to default, implied spreads (recovery as a fraction of par) and debt spreads per requested horizon, the CDS par spread and implied equity at the requested maturity, and models.credit.pd.MasterScale.defaultCalibration grades of the default probabilities. Decimals as returned (spreads per year, probabilities and volatilities as fractions), no unit inferred; a measure Rust rejects for the model shows its native error beside the rest.",
  },
  {
    id: "curve-query",
    api: [
      "models.volatility.getSurfaceVol",
      "VolatilityNamespace",
      "getSurfaceVol",
    ],
    route: "returned-model",
    convention:
      "Returned native values of one stored object per supplied point: DiscountCurve.df and zeroRate (default continuous compounding) and forward (continuously compounded, year fractions only), ForwardCurve.rate (simple, on the curve day count), HazardCurve.sp and hazardRate, and models.volatility.getSurfaceVol (expiry in years, strike in the surface's units, inside the grid). Curve dates pass through as ISO text for Rust to convert with the curve day count; no date, unit or tenor conversion happens here. A point Rust rejects keeps its native error in its cell.",
  },
  {
    id: "yield-workout-ladder",
    api: [
      "valuations.instruments.bondWorkoutLadder",
      "ValuationInstrumentsNamespace",
      "bondWorkoutLadder",
    ],
    route: "returned-report",
    convention:
      "Returned BondWorkoutLadder of one bond: one row per call, put and maturity exit in Rust's date order, every yield solved at the one price shown. The worst row is the one is_worst marks, the exit the ytw metric selects; no yield is compared here. Yield and I-spread units are what metricMetadata declares for ytw and i_spread, and a null yield is shown as no yield, never zero. Z-spread is not reported per exit.",
  },
  {
    id: "statement-export",
    api: [
      "statements.statementResultToTableWide",
      "StatementsNamespace",
      "statementResultToTableWide",
    ],
    route: "returned-report",
    convention:
      "Returned TableEnvelope of one StatementResult in the wide or long layout the user picks, statementResultToTableLong for long, in Rust's row and column order. The CSV holds exactly those cells: RFC 4180 quoting, the native column names as its header, and each number as String(value), unrounded. A cell Rust holds with no value (NaN, null in JSON) shows as undefined and is an empty field, never zero. Nothing is computed here.",
  },
  {
    id: "wacc-calculator",
    api: ["statements_analytics.wacc", "StatementsAnalyticsNamespace", "wacc"],
    route: "returned-report",
    convention:
      "Returned WACC of the five entered decimals, itself a decimal. The two capital weights are separate inputs; nothing is blended, defaulted or checked here, and weights that do not sum to one or a tax rate outside [0, 1] show the native error. Applying the rate to a DCF is an explicit user action that passes the returned number unchanged.",
  },
  {
    id: "quote-converter",
    api: [
      "valuations.instruments.computeQuotes",
      "ValuationInstrumentsNamespace",
      "computeQuotes",
    ],
    route: "returned-report",
    convention:
      "Returned BondQuoteSet of one bond from the one quote entered: every price, yield and spread in Rust's field order, each the metric of the same name at that clean price. Quote kinds are the branches of the BondQuoteInput schema. Units are the labels bondQuoteSetFieldUnits returns for each field, which Rust tests against metricMetadata. A null field is shown as no value, never zero, and a quote that cannot be solved shows the native error. Nothing is converted or compared here.",
  },
  {
    id: "bond-credit-card",
    api: [
      "valuations.instruments.priceInstrumentWithMarket",
      "ValuationInstrumentsNamespace",
      "priceInstrumentWithMarket",
    ],
    route: "returned-model",
    convention:
      "Returned measures of one bond, each metric id priced on its own valuations.instruments.priceInstrumentWithMarket call because one failing id fails a whole native request; an id's native error stays in its row, so no script decides whether a bond has a government curve or is hazard-priced. The card chooses which ids to show and keeps those metricsForInstrument lists for the type. Units and groups are what metricMetadata declares (not declared when it says unknown, as for breakeven, whose unit follows its target); values are never rescaled, classified, summed or derived. theta_period and breakeven_config come from the MetricPricingOverrides schema's form and reach Rust as the request's metricPricingOverrides; no horizon, target or mode is assumed.",
  },
  {
    id: "convertible-card",
    api: [
      "valuations.instruments.convertibleSpotLadder",
      "ValuationInstrumentsNamespace",
      "convertibleSpotLadder",
    ],
    route: "returned-report",
    convention:
      "Returned ConvertibleSpotLadder of one convertible on the tree model: one row per share price in Rust's ascending order, the share prices the ones entered or Rust's default ladder. Each row's price, delta, parity, conversion value, premium and floor are the metrics of those names at that share price, with volatility, rates and credit as supplied. The regime and the market row are what Rust returned; nothing is classified or compared here. Regime is one market convention, delta over the conversion ratio on the 40 and 80 delta bands. A null hedge ratio, regime or premium shows as undefined, never zero. Metric rows are each priced on their own valuations.instruments.priceInstrumentWithMarket call, so an id's native error stays in its row; column units are what metricMetadata declares.",
  },
  {
    id: "loan-yield-card",
    api: [
      "valuations.instruments.priceInstrumentWithMarket",
      "ValuationInstrumentsNamespace",
      "priceInstrumentWithMarket",
    ],
    route: "returned-model",
    convention:
      "Returned measures of one loan, each metric id priced on its own valuations.instruments.priceInstrumentWithMarket call because one failing id fails a whole native request; an id's native error stays in its row. The card chooses which ids to show and keeps those metricsForInstrument lists for the type. Units and groups are what metricMetadata declares (not declared when it says unknown); values are never rescaled, classified, summed or derived. Yields and discount margin solve to a quoted clean price when the instrument carries one, otherwise to the model price; embedded_option_value is callable less straight at one spread under the tree model; recovery01 and jump_to_default price on a credit curve under the tree model.",
  },
  {
    id: "liability-management-card",
    api: [
      "models.credit.liabilityManagement.analyzeExchangeOffer",
      "CreditLiabilityManagementNamespace",
      "analyzeExchangeOffer",
    ],
    route: "returned-model",
    convention:
      "Returned models.credit.liabilityManagement.analyzeExchangeOffer and analyzeLme results as Rust returned them. The tender recommendation is Rust's tender_recommended field; tenderRecommendationHurdle shows beside it as supplied and is never compared here. Amounts are in one caller-defined unit with no currency conversion; percentages are fractions and leverage a multiple. The LME price input is a price, an extension fee or a transferred-asset fraction by structure, as the facade documents.",
  },
  {
    id: "recovery-waterfall",
    api: [
      "models.credit.recoveryWaterfall.allocateRecovery",
      "CreditRecoveryWaterfallNamespace",
      "allocateRecovery",
    ],
    route: "returned-model",
    convention:
      "Returned models.credit.recoveryWaterfall.allocateRecovery, models.credit.lgd.workoutLgd and seniorityRecoveryStats results as Rust returned them. Allocations are in Rust's priority-then-entry order; each claim's total, collateral and general recovery, recovery rate and deficiency, and the estate's distributed and undistributed totals, are Rust's fields, never summed or divided here. Amounts are in one caller-defined unit with no currency conversion; rates are fractions. Seniority classes and collateral types are the generated SeniorityClass and WorkoutCollateralType contracts; calibration ids are seniorityCalibrationIds, default first. A class a calibration lacks shows Rust's error.",
  },
  {
    id: "option-calculator",
    api: ["models.bsPrice", "ModelsNamespace", "bsPrice"],
    route: "returned-model",
    convention:
      "Returned closed-form results of one European option: models.bsPrice, black76Price or bachelierPrice of an entered volatility, or bsImpliedVol, black76ImpliedVol or models.volatility.impliedVolBachelier of an entered price, then bsGreeks, black76Greeks or bachelierGreeks at that volatility. Decimals as returned; the labels state what the facade documents (Black-76 prices discounted, Bachelier undiscounted; Black-Scholes vega and rhos per 1% move, theta per day; forward-Greek vega per unit of volatility). Rust validates every input, including a price below intrinsic value; no scaling or validity rule is applied here.",
  },
  {
    id: "credit-scorecard",
    api: [
      "models.credit.scoring.zmijewskiScore",
      "CreditScoringNamespace",
      "zmijewskiScore",
    ],
    route: "returned-model",
    convention:
      "Returned models.credit.scoring ScoringResult of one model (Altman Z, Z', Z'' or EM, Ohlson O, Zmijewski) over ratios entered in the fields of its generated input contract and passed in the facade's positional order: score, zone and implied PD as returned (probabilities as fractions), and models.credit.pd.MasterScale.defaultCalibration().mapScore of the result, or Rust's error when the result has no implied PD (the Altman models). No zone, threshold or grade is derived here.",
  },
  {
    id: "portfolio-risk-report",
    schema: `${base}models/1/position_risk_decomposition.schema.json`,
    pointer: "#",
    api: [
      "models.factor.risk.parametricVarDecomposition",
      "FactorRiskNamespace",
      "parametricVarDecomposition",
    ],
    route: "returned-report",
    convention:
      "Returned models.factor.risk PositionRiskDecomposition (parametric or historical): portfolio VaR and ES, confidence, method and the Euler residual (null for historical) as returned, component VaR charted and tabulated in returned order with relative, marginal and incremental VaR and the ES contributions; evaluateRiskBudget over positionComponentVar of each position against the supplied target shares (has_breach, total_overbudget, utilization and excess as returned, an omitted threshold left to Rust); buildStressAttribution of a historical source's P&L in Rust's largest-driver-first order with the tail scenarios worst first. Losses negative, in the units of the supplied weights or P&L; nothing summed, flagged or re-sorted. The factor section shows models.factor.risk.decomposeFactors of portfolio.computeFactorSensitivities (positions in Portfolio order, factors in definition order, base currency) as returned under the returned measure, an omitted measure and bump config left to Rust.",
  },
  {
    id: "transition-matrix-heatmap",
    api: [
      "models.credit.migration.TransitionMatrix#toRows",
      "TransitionMatrix",
      "toRows",
    ],
    route: "returned-model",
    convention:
      "Returned models.credit.migration results: TransitionMatrix rows and default probabilities in RatingScale order, the GeneratorMatrix rows with its roundTripError and regularizationL1, and the default probabilities of project(generator, t) per requested horizon. Decimals as returned; an omitted round-trip tolerance is left to Rust. Heatmap colour domains are presentation only: [0, 1] for probabilities, the intensities' extent (centred on zero) for the generator.",
  },
  {
    id: "simm-sensitivity-table",
    schema: `${base}margin/1/simm_sensitivities_json.schema.json`,
    pointer: "#",
    route: "supplied-input",
    convention:
      "A supplied or natively canonicalized SimmSensitivitiesJson: delta and vega tuples and curvature entries listed under their SIMM risk class in input order, columns named by the Rust tuple positions; amounts in the stated base currency, never summed, netted or converted.",
  },
  {
    id: "simm-report",
    schema: `${base}margin/1/im_result.schema.json`,
    pointer: "#",
    api: [
      "margin.SimmCalculator#calculateFromSensitivities",
      "SimmCalculator",
      "calculateFromSensitivities",
    ],
    route: "returned-report",
    convention:
      "Returned ImResult of SimmCalculator#calculateFromSensitivities in the set's own base currency (Rust accepts only USD): exact amount and breakdown money, methodology, as_of, mpor_days, the approximation flag beside an always-shown Indicative badge, and the simm_detail risk-class margins, buckets and weighted sensitivities as returned. Omitted version and MPOR are left to Rust.",
  },
  {
    id: "simm-calculator",
    schema: `${base}margin/1/simm_sensitivities_json.schema.json`,
    pointer: "#",
    route: "schema",
    convention:
      "Generated simm_sensitivities_json form (Rust example as the default document) and an ISO calculation date; the natively validated set and the returned ImResult are presented by simm-sensitivity-table and simm-report.",
  },
  {
    id: "csa-editor",
    schema: `${base}margin/1/csa_spec.schema.json`,
    pointer: "#",
    route: "schema",
    convention:
      "Generated csa_spec form (Rust example as the default document), optionally replaced by a Rust regulatory preset (margin.csaSpecUsdRegulatory, csaSpecEurRegulatory, or csaSpecRegulatory over a typed currency, id and collateral curve, which start empty); validated natively by csaSpecValidate and canonicalized through VmCalculator#csa.",
  },
  {
    id: "margin-call-table",
    schema: `${base}margin/1/vm_result.schema.json`,
    pointer: "#",
    api: ["margin.VmCalculator#calculate", "VmCalculator", "calculate"],
    route: "returned-report",
    convention:
      "Returned VmResult of VmCalculator#calculate in the CSA's own base currency, with Rust's vmResultRequiresCall decision and vmResultNetMargin (post minus collect, positive when the desk posts, major units); its money terms as exact returned money; the generateMarginCalls schedule in returned order and the marginCallDates list. Nothing netted, thresholded or rounded in the UI.",
  },
];

export async function generateProvenance(repo, contracts, fixtureManifest) {
  const declarationPath = "finstack-quant-wasm/index.d.ts";
  const text = await readFile(resolve(repo, declarationPath), "utf8");
  const ast = ts.createSourceFile(
    declarationPath,
    text,
    ts.ScriptTarget.Latest,
    true,
  );
  // Published owners: namespace interfaces and declared handle classes (such
  // as analytics.Performance, merged with a same-named interface).
  const owners = Map.groupBy(
    ast.statements.filter(
      (node) =>
        (ts.isInterfaceDeclaration(node) || ts.isClassDeclaration(node)) &&
        node.name,
    ),
    (node) => node.name.text,
  );
  const digest = (value) => createHash("sha256").update(value).digest("hex");
  function apiSource([name, owner, method]) {
    const member = owners
      .get(owner)
      ?.flatMap((node) => [...node.members])
      .find((node) => node.name?.getText(ast) === method);
    if (
      !member ||
      !(ts.isMethodSignature(member) || ts.isMethodDeclaration(member))
    )
      throw new Error(`Missing published API: ${owner}.${method}`);
    const declaration = member.getFullText(ast).trim();
    return {
      name,
      source: declarationPath,
      owner,
      member: method,
      inputs: member.parameters.map((parameter) => ({
        name: parameter.name.getText(ast),
        type: parameter.type.getText(ast),
        optional: Boolean(parameter.questionToken),
      })),
      returns: member.type.getText(ast),
      declaration,
      sha256: digest(declaration),
    };
  }
  function schemaSource(uri, pointer) {
    const contract = contracts.get(uri);
    if (!contract) throw new Error(`Missing schema: ${uri}`);
    const node = schemaAt(contract.schema, pointer);
    if (node === undefined)
      throw new Error(`Missing schema pointer: ${uri}${pointer}`);
    return {
      uri,
      pointer,
      source: contract.source,
      sha256: digest(JSON.stringify(node)),
      description: node.description ?? null,
    };
  }
  const instrumentRoots = [...contracts.keys()]
    .filter((uri) => /\/instrument\/1\/[^/]+\/[^/]+\.schema\.json$/.test(uri))
    .sort(byCodeUnit);
  const entries = displays.map(({ api, schema, pointer, ...entry }) => ({
    ...entry,
    ...(api ? { api: apiSource(api) } : {}),
    ...(schema
      ? {
          schemas: (schema === "instrument-roots"
            ? instrumentRoots
            : [schema]
          ).map((uri) => schemaSource(uri, pointer)),
        }
      : {}),
  }));
  const curves = contracts
    .get(market)
    .schema.$defs.CurveState.oneOf.map((variant, index) => ({
      type: variant.properties.type.const,
      route: Object.hasOwn(variant.properties, "knot_points")
        ? "stored-knots"
        : "fields",
      fields: Object.keys(variant.properties),
      source: schemaSource(market, `#/$defs/CurveState/oneOf/${index}`),
    }));
  const details = contracts
    .get(result)
    .schema.$defs.ValuationDetails.oneOf.map((variant) => ({
      type: variant.properties.type.const,
      route: variant.properties.type.const === "monte_carlo" ? "typed" : "raw",
    }));
  const hostTypes = [
    "ValuationResult",
    "ValuationDetails",
    "MonteCarloValuationDetails",
    "MoneyValue",
    "VolCubeConstructor",
    "FxDeltaVolSurfaceConstructor",
    "SabrSmileConstructor",
  ];
  const hostDeclarations = hostTypes.map((name) => {
    const node = ast.statements.find(
      (node) => node.name?.getText(ast) === name,
    );
    if (!node) throw new Error(`Missing host type: ${name}`);
    const declaration = node.getFullText(ast).trim();
    return {
      name,
      source: declarationPath,
      declaration,
      sha256: digest(declaration),
    };
  });
  // Retain the native scenario authority alongside the matching facade declaration.
  const scenarioPath =
    "finstack-quant/valuations/structured-credit/src/structured_credit/metrics/scenario.rs";
  const scenarioSource = await readFile(resolve(repo, scenarioPath), "utf8");
  return {
    inventory: {
      version: 1,
      fixtureManifest: "generated/fixtures.json",
      fixtureManifestSha256: digest(fixtureManifest),
      entries,
      hostDeclarations,
      hostContract: {
        source:
          "finstack-quant-wasm/schemas/host/1/valuation_result.schema.json",
        sha256: digest(
          await readFile(
            resolve(
              repo,
              "finstack-quant-wasm/schemas/host/1/valuation_result.schema.json",
            ),
          ),
        ),
      },
      details,
      scenarioAuthority: {
        source: scenarioPath,
        sha256: digest(scenarioSource),
      },
      deferred: {
        "machine-readable-units":
          "Generic values stay raw; do not infer units from names or decimal refs.",
        "full-state-curve-evaluation":
          "Use stored nodes and parameters; no constructor-based curve recreation.",
        "off-grid-surface-evaluation":
          "Stored nodes only; off-grid values come only from native evaluators or a Rust-materialized grid (fx-strike-grid, vol-cube-tenor-slice, vol-cube-expiry-slice).",
        "quote-space-calibration-fits":
          "Returned residuals only until quote repricing is published.",
        "missing-financial-aggregates":
          "Unavailable unless an existing canonical result supplies the value.",
      },
    },
    curves,
  };
}
