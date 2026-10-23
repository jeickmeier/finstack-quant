import * as wasm from '../pkg/finstack_quant_wasm.js';

// Rust `lookback_returns` and `period_stats` take an optional FiscalConfig.
// wasm-bindgen cannot borrow an optional handle, so the None and Some(config)
// calls are exported separately and joined here. An omitted, null or undefined
// config is Rust None (calendar year for FYTD, ISO weeks for periodStats); the
// facade never substitutes FiscalConfig.calendarYear().
const performance = wasm.Performance.prototype;
const rawLookbackReturns = performance.lookbackReturns;
const rawLookbackReturnsWithFiscalConfig = performance.lookbackReturnsWithFiscalConfig;
const rawPeriodStats = performance.periodStats;
const rawPeriodStatsWithFiscalConfig = performance.periodStatsWithFiscalConfig;
delete performance.lookbackReturnsWithFiscalConfig;
delete performance.periodStatsWithFiscalConfig;

const isNone = (value) => value === undefined || value === null;

performance.lookbackReturns = function lookbackReturns(refDate, fiscalConfig = undefined) {
  return isNone(fiscalConfig)
    ? rawLookbackReturns.call(this, refDate)
    : rawLookbackReturnsWithFiscalConfig.call(this, refDate, fiscalConfig);
};

performance.periodStats = function periodStats(
  tickerIdx,
  aggregationFrequency = undefined,
  fiscalConfig = undefined
) {
  return isNone(fiscalConfig)
    ? rawPeriodStats.call(this, tickerIdx, aggregationFrequency)
    : rawPeriodStatsWithFiscalConfig.call(this, tickerIdx, aggregationFrequency, fiscalConfig);
};

export const analytics = {
  Performance: wasm.Performance,
  constrainedLeastSquares: wasm.constrainedLeastSquares,
  maxDrawdown: wasm.maxDrawdown,
  sharpe: wasm.sharpe,
  sortino: wasm.sortino,
  volatility: wasm.volatility,
};
