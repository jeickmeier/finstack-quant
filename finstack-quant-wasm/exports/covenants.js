import * as wasm from '../pkg/finstack_quant_wasm.js';

/**
 * Covenant definitions, evaluation and forecasting from the Rust
 * `finstack-quant-covenants` crate.
 *
 * `CovenantEngine` is a handle. Covenant definitions, reports and forecasts
 * cross as plain objects that mirror the canonical Rust serde model (types
 * under `./types`), with their Rust constructors and methods as free
 * functions. The `*Json` functions are the JSON-string bridge.
 * @example
 * ```typescript
 * import init, { covenants } from "finstack-quant-wasm";
 * await init();
 * const specs = covenants.covenantTemplate({
 *   kind: "lbo_standard",
 *   initial_leverage: 6,
 *   interest_coverage: 2,
 *   fixed_charge_coverage: 1.5,
 *   max_capex: 50_000_000,
 * });
 * const reports = covenants.evaluateEngine(
 *   JSON.stringify({ specs }),
 *   JSON.stringify({ debt_to_ebitda: 4.5, interest_coverage: 3, fixed_charge_coverage: 2, capex: 1e7 }),
 *   "2026-03-31"
 * );
 * console.log(Object.values(reports).every((report) => report.passed));
 * ```
 */
export const covenants = {
  CovenantEngine: wasm.CovenantEngine,
  covenantConsequenceAccelerateMaturity: wasm.covenantConsequenceAccelerateMaturity,
  covenantConsequenceBlockDistributions: wasm.covenantConsequenceBlockDistributions,
  covenantConsequenceCashSweep: wasm.covenantConsequenceCashSweep,
  covenantConsequenceDefault: wasm.covenantConsequenceDefault,
  covenantConsequenceRateIncrease: wasm.covenantConsequenceRateIncrease,
  covenantConsequenceRequireCollateral: wasm.covenantConsequenceRequireCollateral,
  covenantForecastConfigWithScope: wasm.covenantForecastConfigWithScope,
  covenantNew: wasm.covenantNew,
  covenantSpecNew: wasm.covenantSpecNew,
  covenantSpecWithDenominatorMetric: wasm.covenantSpecWithDenominatorMetric,
  covenantSpecWithMetric: wasm.covenantSpecWithMetric,
  covenantSpecWithThresholdSchedule: wasm.covenantSpecWithThresholdSchedule,
  covenantTemplate: wasm.covenantTemplate,
  covenantTypeAffirmative: wasm.covenantTypeAffirmative,
  covenantTypeBasket: wasm.covenantTypeBasket,
  covenantTypeCustom: wasm.covenantTypeCustom,
  covenantTypeMaxCapex: wasm.covenantTypeMaxCapex,
  covenantTypeMaxDebtToEbitda: wasm.covenantTypeMaxDebtToEbitda,
  covenantTypeMaxNetDebtToEbitda: wasm.covenantTypeMaxNetDebtToEbitda,
  covenantTypeMaxSeniorLeverage: wasm.covenantTypeMaxSeniorLeverage,
  covenantTypeMaxTotalLeverage: wasm.covenantTypeMaxTotalLeverage,
  covenantTypeMinAssetCoverage: wasm.covenantTypeMinAssetCoverage,
  covenantTypeMinDscr: wasm.covenantTypeMinDscr,
  covenantTypeMinFixedChargeCoverage: wasm.covenantTypeMinFixedChargeCoverage,
  covenantTypeMinInterestCoverage: wasm.covenantTypeMinInterestCoverage,
  covenantTypeMinLiquidity: wasm.covenantTypeMinLiquidity,
  covenantTypeNegative: wasm.covenantTypeNegative,
  covenantWithConsequence: wasm.covenantWithConsequence,
  covenantWithCurePeriod: wasm.covenantWithCurePeriod,
  covenantWithScope: wasm.covenantWithScope,
  covenantWithSpringingCondition: wasm.covenantWithSpringingCondition,
  evaluateEngine: wasm.evaluateEngine,
  forecastBreaches: wasm.forecastBreaches,
  forecastCovenant: wasm.forecastCovenant,
  thresholdScheduleThresholdFor: wasm.thresholdScheduleThresholdFor,
  validateCovenantEngineJson: wasm.validateCovenantEngineJson,
  validateCovenantReportJson: wasm.validateCovenantReportJson,
  validateCovenantSpecJson: wasm.validateCovenantSpecJson,
};
