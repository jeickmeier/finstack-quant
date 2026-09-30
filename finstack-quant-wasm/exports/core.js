import * as wasm from '../pkg/finstack_quant_wasm.js';

// Rust `DayCount::year_fraction` / `signed_year_fraction` take a context; the
// published methods make it optional. An omitted context is the Rust default
// (`new DayCountContext()`), created once after `init()` and only borrowed.
let defaultDayCountContext;
const dayCountContext = (ctx) =>
  ctx === undefined || ctx === null ? (defaultDayCountContext ??= new wasm.DayCountContext()) : ctx;
const rawYearFraction = wasm.DayCount.prototype.yearFraction;
const rawSignedYearFraction = wasm.DayCount.prototype.signedYearFraction;
wasm.DayCount.prototype.yearFraction = function yearFraction(start, end, ctx = undefined) {
  return rawYearFraction.call(this, start, end, dayCountContext(ctx));
};
wasm.DayCount.prototype.signedYearFraction = function signedYearFraction(
  start,
  end,
  ctx = undefined
) {
  return rawSignedYearFraction.call(this, start, end, dayCountContext(ctx));
};

// `FxMatrix.rate(base, quote, date, policy?)`: an omitted policy runs the
// Rust default query (`FxQuery::new`), exposed raw as `rateWithDefaultPolicy`.
const rateWithPolicy = wasm.FxMatrix.prototype.rate;
const rateWithDefaultPolicy = wasm.FxMatrix.prototype.rateWithDefaultPolicy;
delete wasm.FxMatrix.prototype.rateWithDefaultPolicy;
wasm.FxMatrix.prototype.rate = function rate(base, quote, date, policy = undefined) {
  return policy === undefined || policy === null
    ? rateWithDefaultPolicy.call(this, base, quote, date)
    : rateWithPolicy.call(this, base, quote, date, policy);
};

export const core = {
  Currency: wasm.Currency,
  Money: wasm.Money,
  Rate: wasm.Rate,
  Bps: wasm.Bps,
  Percentage: wasm.Percentage,
  DayCount: wasm.DayCount,
  DayCountContext: wasm.DayCountContext,
  Tenor: wasm.Tenor,
  createDate: wasm.createDate,
  dateFromEpochDays: wasm.dateFromEpochDays,
  adjust: wasm.adjust,
  availableCalendars: wasm.availableCalendars,
  DiscountCurve: wasm.DiscountCurve,
  HazardCurve: wasm.HazardCurve,
  ForwardCurve: wasm.ForwardCurve,
  VolCube: wasm.VolCube,
  FxDeltaVolSurface: wasm.FxDeltaVolSurface,
  MarketContext: wasm.MarketContext,
  FxConversionPolicy: wasm.FxConversionPolicy,
  FxRateResult: wasm.FxRateResult,
  FxMatrix: wasm.FxMatrix,
  FxQuoteConvention: wasm.FxQuoteConvention,
  FxPairConvention: wasm.FxPairConvention,
  fxMarketPair: wasm.fxMarketPair,
  fxPairConvention: wasm.fxPairConvention,
  fxPipSize: wasm.fxPipSize,
  invertFxRate: wasm.invertFxRate,
  applyLowerTriangular: wasm.applyLowerTriangular,
  choleskyDecomposition: wasm.choleskyDecomposition,
  choleskySolve: wasm.choleskySolve,
  mean: wasm.mean,
  variance: wasm.variance,
  populationVariance: wasm.populationVariance,
  correlation: wasm.correlation,
  covariance: wasm.covariance,
  quantile: wasm.quantile,
  normCdf: wasm.normCdf,
  normPdf: wasm.normPdf,
  standardNormalInvCdf: wasm.standardNormalInvCdf,
  erf: wasm.erf,
  lnGamma: wasm.lnGamma,
  kahanSum: wasm.kahanSum,
  neumaierSum: wasm.neumaierSum,
  longestPositiveRun: wasm.longestPositiveRun,
};
