import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './cashflows/schema.js';
import { chain } from './chain.js';

// Rust `CashFlowBuilder::build` and `CashflowScheduleBuildSpec::build` take an
// optional market. wasm-bindgen cannot borrow an optional handle, so the
// `None` and `Some(market)` calls are exported separately and joined here.
/**
 * Build the schedule, projecting floating coupons from market curves when a market is given.
 *
 * The builder is not consumed and can be built again.
 *
 * @param market - Optional `MarketContext` handle holding the forward curves and fixings the floating legs reference; omitted means floating coupons that need a forward curve use their fallback policy.
 * @returns The built `CashFlowSchedule` handle; release it with free().
 * @throws If a setter recorded an error, the principal is unset, a referenced curve or fixing is missing (kind `not_found`), or schedule generation fails (kind `validation`).
 *
 * @facadeClass CashFlowBuilder
 * @member build(market?: MarketContext | null): CashFlowSchedule
 */
const rawBuild = wasm.CashFlowBuilder.prototype.build;
const rawBuildWithMarket = wasm.CashFlowBuilder.prototype.buildWithMarket;
delete wasm.CashFlowBuilder.prototype.buildWithMarket;
// wasm-bindgen cannot borrow the optional market: joins the None and Some(market) exports.
wasm.CashFlowBuilder.prototype.build = function build(market = undefined) {
  return market === undefined || market === null
    ? rawBuild.call(this)
    : rawBuildWithMarket.call(this, market);
};

// wasm-bindgen cannot borrow an array of handles either: a list of schedules
// crosses as canonical JSON, which Rust parses back into `CashFlowSchedule`s.
const scheduleJsonList = (schedules) =>
  Array.isArray(schedules)
    ? schedules.map((schedule) =>
        schedule instanceof wasm.CashFlowSchedule ? schedule.toJson() : schedule
      )
    : schedules;

chain(wasm.CashFlowBuilder, [
  'principal',
  'principalExchange',
  'amortization',
  'addPrincipalEvent',
  'fixedCf',
  'floatingCf',
  'stepUpCf',
  'fee',
  'addFixedWindow',
  'addFloatingWindow',
  'addPaymentWindow',
  'paymentSplitProgram',
  'fixedToFloat',
  'floatMarginSteps',
]);

/**
 * Cashflow schedules from the Rust `finstack-quant-cashflows` crate.
 *
 * `CashFlowSchedule`, `CashFlowBuilder` and `AccrualIndex` are handles. Specs,
 * flows and aggregations cross as plain objects that mirror the canonical Rust
 * serde model (types under `./types`), with their Rust constructors and
 * methods as free functions. The `*Json` functions are the JSON-string bridge.
 * @example
 * ```typescript
 * import init, { cashflows } from "finstack-quant-wasm";
 * await init();
 * const spec = JSON.stringify({
 *   notional: { initial: { amount: "1000000", currency: "USD" }, amort: "none" },
 *   issue_date: "2026-01-02",
 *   maturity: "2027-01-02",
 *   coupon_program: [{
 *     kind: "fixed",
 *     spec: {
 *       coupon_type: "cash",
 *       rate: "0.05",
 *       frequency: { count: 12, unit: "months" },
 *       day_count: "30_360",
 *       business_day_convention: "following",
 *       calendar_id: "weekends_only",
 *       stub: "none",
 *       end_of_month: false,
 *       payment_lag_days: 0
 *     }
 *   }]
 * });
 * const schedule = cashflows.buildCashflowScheduleJson(spec);
 * console.log(JSON.parse(cashflows.datedFlowsJson(schedule)).length);
 * ```
 */
export const cashflows = {
  AccrualIndex: wasm.AccrualIndex,
  CashFlowBuilder: wasm.CashFlowBuilder,
  CashFlowSchedule: wasm.CashFlowSchedule,
  absToSmm: wasm.absToSmm,
  accruedInterestAmount: wasm.accruedInterestAmount,
  aggregateByPeriod: wasm.aggregateByPeriod,
  aggregateCashflowsChecked: wasm.aggregateCashflowsChecked,
  annualToMonthly: wasm.annualToMonthly,
  // wasm-bindgen cannot borrow the optional market: joins the None and Some(market) exports.
  /**
   * Build a cashflow schedule from a build spec.
   *
   * Typed twin of `buildCashflowScheduleJson`: the same spec, returning a
   * `CashFlowSchedule` handle.
   *
   * @param spec - `CashflowScheduleBuildSpec` wire object or JSON: notional, issue and maturity dates, coupon program, fees and principal events.
   * @param market - Optional `MarketContext` handle holding the forward curves and fixings the floating legs reference; omitted means floating coupons that need a forward curve use their fallback policy.
   * @param asOf - Optional ISO-8601 valuation cutoff date overriding `spec.as_of`; resets before the cutoff require historical fixings.
   * @returns The built `CashFlowSchedule` handle; release it with free().
   * @throws If `spec` does not match the build-spec schema (kind `invalid_type` or `validation`), `asOf` is not an ISO-8601 date string, a referenced curve or fixing is missing (kind `not_found`), or schedule construction fails (kind `validation`).
   *
   * @facadeType (spec: generated.cashflows.CashflowScheduleBuildSpec | string, market?: MarketContext | null, asOf?: string | null) => CashFlowSchedule
   */
  buildCashflowSchedule: (spec, market = undefined, asOf = undefined) =>
    market === undefined || market === null
      ? wasm.buildCashflowSchedule(spec, asOf)
      : wasm.buildCashflowScheduleWithMarket(spec, market, asOf),
  buildCashflowScheduleJson: wasm.buildCashflowScheduleJson,
  calendarYearLadder: wasm.calendarYearLadder,
  datedFlows: wasm.datedFlows,
  datedFlowsJson: wasm.datedFlowsJson,
  defaultModelSpecCumulativeDefaultFraction: wasm.defaultModelSpecCumulativeDefaultFraction,
  defaultModelSpecMdr: wasm.defaultModelSpecMdr,
  defaultModelSpecMdrWithSurvival: wasm.defaultModelSpecMdrWithSurvival,
  defaultModelSpecValidate: wasm.defaultModelSpecValidate,
  exCouponRuleExDate: wasm.exCouponRuleExDate,
  exCouponRuleIsExCoupon: wasm.exCouponRuleIsExCoupon,
  floatingLegCompoundingIsOvernight: wasm.floatingLegCompoundingIsOvernight,
  floatingLegCompoundingSofrObservationShift: wasm.floatingLegCompoundingSofrObservationShift,
  floatingLegCompoundingSoniaObservationShift: wasm.floatingLegCompoundingSoniaObservationShift,
  floatingRateSpecEuribor3m: wasm.floatingRateSpecEuribor3m,
  floatingRateSpecSofr: wasm.floatingRateSpecSofr,
  floatingRateSpecSonia: wasm.floatingRateSpecSonia,
  floatingRateSpecValidate: wasm.floatingRateSpecValidate,
  // wasm-bindgen cannot borrow an array of handles: `scheduleJsonList` sends canonical JSON.
  /**
   * Materialize the reset observations crossed by a realized-forward market roll.
   *
   * Every projected fixing recorded in the schedules' metadata with a date in
   * `[oldDate, newDate]` is written into a copy of the market as an observed
   * fixing; existing observations take precedence and are kept unchanged.
   *
   * @param market - `MarketContext` handle of the pre-roll market; it is not modified.
   * @param schedules - Schedules projected on `market` at `oldDate`: `CashFlowSchedule` handles (the facade passes them as JSON), canonical JSON strings or plain objects.
   * @param oldDate - ISO-8601 inclusive start of the fixing window: same-day projections become observed.
   * @param newDate - ISO-8601 inclusive end of the window; must be on or after `oldDate`.
   * @returns A new `MarketContext` handle with the crossed fixings materialized; release it with free().
   * @throws If the window runs backward, a fixing-series identifier is malformed, a crossed projection is unavailable or non-finite, or two schedules project the same index and date differently (kind `validation`).
   *
   * @facadeType (market: MarketContext, schedules: readonly (CashFlowSchedule | JsonInput)[], oldDate: string, newDate: string) => MarketContext
   */
  materializeFixings: (market, schedules, oldDate, newDate) =>
    wasm.materializeFixings(market, scheduleJsonList(schedules), oldDate, newDate),
  // wasm-bindgen cannot borrow an array of handles: `scheduleJsonList` sends canonical JSON.
  /**
   * Merge several schedules into one under a new notional and day count.
   *
   * Flows are concatenated and put into canonical order; calendar ids and
   * projected fixings are merged.
   *
   * @param schedules - Schedules to merge: `CashFlowSchedule` handles (the facade passes them as JSON), canonical JSON strings or plain objects.
   * @param notional - `Notional` wire object of the merged schedule.
   * @param dayCount - `DayCount` wire string of the merged schedule.
   * @returns The merged `CashFlowSchedule` handle.
   * @throws If `schedules` is not an array (kind `invalid_type`) or an argument does not match its wire type (kind `validation`).
   *
   * @facadeType (schedules: readonly (CashFlowSchedule | JsonInput)[], notional: generated.cashflows.Notional, dayCount: generated.core.DayCount) => CashFlowSchedule
   */
  mergeCashflowSchedules: (schedules, notional, dayCount) =>
    wasm.mergeCashflowSchedules(scheduleJsonList(schedules), notional, dayCount),
  monthlyToAnnual: wasm.monthlyToAnnual,
  notionalPar: wasm.notionalPar,
  notionalValidate: wasm.notionalValidate,
  periodAggregationGetAmount: wasm.periodAggregationGetAmount,
  periodAggregationGetPeriod: wasm.periodAggregationGetPeriod,
  prepaymentModelSpecSmm: wasm.prepaymentModelSpecSmm,
  prepaymentModelSpecValidate: wasm.prepaymentModelSpecValidate,
  recoveryModelSpecRecoveryRate: wasm.recoveryModelSpecRecoveryRate,
  recoveryModelSpecValidate: wasm.recoveryModelSpecValidate,
  scheduleFromClassifiedFlows: wasm.scheduleFromClassifiedFlows,
  scheduleFromDatedFlows: wasm.scheduleFromDatedFlows,
  scheduleParamsAnnualActact: wasm.scheduleParamsAnnualActact,
  scheduleParamsEurEstrSwap: wasm.scheduleParamsEurEstrSwap,
  scheduleParamsEurGovBond: wasm.scheduleParamsEurGovBond,
  scheduleParamsGbpSoniaSwap: wasm.scheduleParamsGbpSoniaSwap,
  scheduleParamsJpyTonaSwap: wasm.scheduleParamsJpyTonaSwap,
  scheduleParamsQuarterlyAct360: wasm.scheduleParamsQuarterlyAct360,
  scheduleParamsSemiannual30360: wasm.scheduleParamsSemiannual30360,
  scheduleParamsUsdCorporateBond: wasm.scheduleParamsUsdCorporateBond,
  scheduleParamsUsdSofrSwap: wasm.scheduleParamsUsdSofrSwap,
  scheduleParamsUsdTreasury: wasm.scheduleParamsUsdTreasury,
  scheduleParamsValidate: wasm.scheduleParamsValidate,
  validateCashflowScheduleJson: wasm.validateCashflowScheduleJson,
  schema,
};
