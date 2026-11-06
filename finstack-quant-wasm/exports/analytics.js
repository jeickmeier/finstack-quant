import * as wasm from '../pkg/finstack_quant_wasm.js';

// Rust `lookback_returns` and `period_stats` take an optional FiscalConfig.
// wasm-bindgen cannot borrow an optional handle, so the None and Some(config)
// calls are exported separately and joined here. An omitted, null or undefined
// config is Rust None (calendar year for FYTD, ISO weeks for periodStats); the
// facade never substitutes FiscalConfig.calendarYear().
/**
 * Period-to-date lookback returns.
 *
 * MTD, QTD and YTD start on the first calendar day of `refDate`'s month,
 * calendar quarter and year. FYTD is the first observation on or after the
 * fiscal calendar start of `fiscalConfig` through `refDate`. Holidays are not
 * skipped. The first included simple return still spans the prior close.
 * @param refDate - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end. Omitted or `null` is the last active date; the result's `ref_date` states the date used.
 * @param fiscalConfig - Optional `FiscalConfig` handle for FYTD. Omitted or `null` is the calendar year (`FiscalConfig::default`).
 * @returns The window end `ref_date` and per-ticker `{ mtd, qtd, ytd, fytd }` numeric arrays of lookback returns as decimal fractions; `fytd` is never null.
 * @throws Error - Rejects an invalid ISO `ref_date`, an omitted one while no dates are active, or a result that cannot be serialized to JavaScript.
 *
 * @facadeClass Performance
 * @member lookbackReturns(refDate?: string | null, fiscalConfig?: FiscalConfig | null): LookbackReturns
 */
/**
 * Aggregated period statistics for one asset at the given frequency. Without
 * a fiscal calendar, Gregorian periods are bucketed with ISO weeks.
 * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
 * @param aggregationFrequency - Optional aggregation frequency token; defaults to monthly.
 * @param fiscalConfig - Optional FiscalConfig handle; omitted or null buckets Gregorian periods with ISO weeks.
 * @returns Period statistics object for the selected ticker at the requested frequency.
 * @throws Error - Rejects an unsupported `aggregation_frequency`, an out-of-range `ticker_idx`, an unrepresentable period boundary, or period statistics that cannot be serialized to JavaScript.
 *
 * @facadeClass Performance
 * @member periodStats(tickerIdx: number, aggregationFrequency?: string, fiscalConfig?: FiscalConfig | null): PeriodStats
 */
const performance = wasm.Performance.prototype;
const rawLookbackReturns = performance.lookbackReturns;
const rawLookbackReturnsWithFiscalConfig = performance.lookbackReturnsWithFiscalConfig;
const rawPeriodStats = performance.periodStats;
const rawPeriodStatsWithFiscalConfig = performance.periodStatsWithFiscalConfig;
delete performance.lookbackReturnsWithFiscalConfig;
delete performance.periodStatsWithFiscalConfig;

// wasm-bindgen cannot borrow an optional handle: the None test for the two joins below.
const isNone = (value) => value === undefined || value === null;

// wasm-bindgen cannot borrow an optional FiscalConfig: joins the None and Some(config) exports.
performance.lookbackReturns = function lookbackReturns(refDate, fiscalConfig = undefined) {
  return isNone(fiscalConfig)
    ? rawLookbackReturns.call(this, refDate)
    : rawLookbackReturnsWithFiscalConfig.call(this, refDate, fiscalConfig);
};

// wasm-bindgen cannot borrow an optional FiscalConfig: joins the None and Some(config) exports.
performance.periodStats = function periodStats(
  tickerIdx,
  aggregationFrequency = undefined,
  fiscalConfig = undefined
) {
  return isNone(fiscalConfig)
    ? rawPeriodStats.call(this, tickerIdx, aggregationFrequency)
    : rawPeriodStatsWithFiscalConfig.call(this, tickerIdx, aggregationFrequency, fiscalConfig);
};

/**
 * Namespaced TypeScript entry points for analytics calculations and types.
 * @example
 * ```typescript
 * import init, { analytics } from "finstack-quant-wasm";
 * await init();
 * const factorReturns = analytics.constrainedLeastSquares(
 *   [1, 2],
 *   1,
 *   [0.01, 0.02],
 *   [0.5, 0.5]
 * );
 * console.log(factorReturns[0]);
 * ```
 */
export const analytics = {
  Performance: wasm.Performance,
  constrainedLeastSquares: wasm.constrainedLeastSquares,
  maxDrawdown: wasm.maxDrawdown,
  sharpe: wasm.sharpe,
  sortino: wasm.sortino,
  volatility: wasm.volatility,
};
