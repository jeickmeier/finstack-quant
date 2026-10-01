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

// `MarketContext.insert(curve)` takes any of nine curve and surface classes.
// wasm-bindgen cannot express a borrowed union of handles, so the typed raw
// methods are bound in Rust and the dispatch on the handle's class is done
// here; the raw methods are removed from the published prototype.
const marketContextInserts = [
  [wasm.DiscountCurve, 'insertDiscountCurve'],
  [wasm.ForwardCurve, 'insertForwardCurve'],
  [wasm.HazardCurve, 'insertHazardCurve'],
  [wasm.InflationCurve, 'insertInflationCurve'],
  [wasm.PriceCurve, 'insertPriceCurve'],
  [wasm.BaseCorrelationCurve, 'insertBaseCorrelationCurve'],
  [wasm.VolSurface, 'insertVolSurface'],
  [wasm.FxDeltaVolSurface, 'insertFxDeltaVolSurface'],
  [wasm.VolCube, 'insertVolCube'],
].map(([curveClass, name]) => {
  const insertTyped = wasm.MarketContext.prototype[name];
  delete wasm.MarketContext.prototype[name];
  return [curveClass, insertTyped];
});
wasm.MarketContext.prototype.insert = function insert(curve) {
  for (const [curveClass, insertTyped] of marketContextInserts) {
    if (curve instanceof curveClass) return insertTyped.call(this, curve);
  }
  const error = new TypeError(
    'curve: expected a DiscountCurve, ForwardCurve, HazardCurve, InflationCurve, PriceCurve, BaseCorrelationCurve, VolSurface, FxDeltaVolSurface or VolCube'
  );
  error.kind = 'invalid_type';
  throw error;
};

export const core = {
  Currency: wasm.Currency,
  Money: wasm.Money,
  Rate: wasm.Rate,
  Bps: wasm.Bps,
  Percentage: wasm.Percentage,
  CreditRating: wasm.CreditRating,
  CurveId: wasm.CurveId,
  InstrumentId: wasm.InstrumentId,
  Attributes: wasm.Attributes,
  RoundingMode: wasm.RoundingMode,
  FinstackConfig: wasm.FinstackConfig,
  UnknownScalePolicy: wasm.UnknownScalePolicy,
  RatingScaleRegistry: wasm.RatingScaleRegistry,
  embeddedRegistry: wasm.embeddedRegistry,
  registryFromConfig: wasm.registryFromConfig,
  ratingScalesExtensionKey: wasm.ratingScalesExtensionKey,
  DayCount: wasm.DayCount,
  DayCountContext: wasm.DayCountContext,
  Thirty360Convention: wasm.Thirty360Convention,
  days30360: wasm.days30360,
  days30e360Isda: wasm.days30e360Isda,
  Tenor: wasm.Tenor,
  TenorUnit: wasm.TenorUnit,
  createDate: wasm.createDate,
  dateFromEpochDays: wasm.dateFromEpochDays,
  daysSinceEpoch: wasm.daysSinceEpoch,
  BusinessDayConvention: wasm.BusinessDayConvention,
  HolidayCalendar: wasm.HolidayCalendar,
  adjust: wasm.adjust,
  availableCalendars: wasm.availableCalendars,
  addBusinessDays: wasm.addBusinessDays,
  addWeekdays: wasm.addWeekdays,
  addMonths: wasm.addMonths,
  endOfMonth: wasm.endOfMonth,
  isWeekend: wasm.isWeekend,
  quarter: wasm.quarter,
  fiscalYear: wasm.fiscalYear,
  monthsUntil: wasm.monthsUntil,
  PeriodKind: wasm.PeriodKind,
  PeriodId: wasm.PeriodId,
  FiscalConfig: wasm.FiscalConfig,
  buildPeriods: wasm.buildPeriods,
  buildFiscalPeriods: wasm.buildFiscalPeriods,
  StubKind: wasm.StubKind,
  ScheduleErrorPolicy: wasm.ScheduleErrorPolicy,
  Schedule: wasm.Schedule,
  ScheduleBuilder: wasm.ScheduleBuilder,
  thirdWednesday: wasm.thirdWednesday,
  thirdFriday: wasm.thirdFriday,
  nextImm: wasm.nextImm,
  isImmDate: wasm.isImmDate,
  isCdsDate: wasm.isCdsDate,
  nextCdsDate: wasm.nextCdsDate,
  prevCdsDate: wasm.prevCdsDate,
  prevCdsSemiannualRoll: wasm.prevCdsSemiannualRoll,
  nextSemiannualCdsMaturity: wasm.nextSemiannualCdsMaturity,
  immOptionExpiry: wasm.immOptionExpiry,
  nextImmOptionExpiry: wasm.nextImmOptionExpiry,
  nextEquityOptionExpiry: wasm.nextEquityOptionExpiry,
  SifmaSettlementClass: wasm.SifmaSettlementClass,
  sifmaSettlementDate: wasm.sifmaSettlementDate,
  sifmaSettlementDateForClass: wasm.sifmaSettlementDateForClass,
  estimatedSifmaSettlementDateForClass: wasm.estimatedSifmaSettlementDateForClass,
  nextSifmaSettlement: wasm.nextSifmaSettlement,
  DiscountCurve: wasm.DiscountCurve,
  HazardCurve: wasm.HazardCurve,
  ForwardCurve: wasm.ForwardCurve,
  InflationCurve: wasm.InflationCurve,
  PriceCurve: wasm.PriceCurve,
  BaseCorrelationCurve: wasm.BaseCorrelationCurve,
  CreditIndexData: wasm.CreditIndexData,
  VolSurface: wasm.VolSurface,
  VolCube: wasm.VolCube,
  FxDeltaVolSurface: wasm.FxDeltaVolSurface,
  ScalarTimeSeries: wasm.ScalarTimeSeries,
  InflationIndex: wasm.InflationIndex,
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
  symmetricEigen: wasm.symmetricEigen,
  ledoitWolfShrinkage: wasm.ledoitWolfShrinkage,
  singularThreshold: wasm.singularThreshold,
  diagonalTolerance: wasm.diagonalTolerance,
  symmetryTolerance: wasm.symmetryTolerance,
  mean: wasm.mean,
  variance: wasm.variance,
  populationVariance: wasm.populationVariance,
  meanVar: wasm.meanVar,
  meanOrNan: wasm.meanOrNan,
  sampleVarianceOrNan: wasm.sampleVarianceOrNan,
  sampleStdOrNan: wasm.sampleStdOrNan,
  medianOrNan: wasm.medianOrNan,
  quantileLinearOrNan: wasm.quantileLinearOrNan,
  finiteMinOrNan: wasm.finiteMinOrNan,
  finiteMaxOrNan: wasm.finiteMaxOrNan,
  finiteCount: wasm.finiteCount,
  logReturns: wasm.logReturns,
  correlation: wasm.correlation,
  covariance: wasm.covariance,
  quantile: wasm.quantile,
  realizedVariance: wasm.realizedVariance,
  realizedVarianceOhlc: wasm.realizedVarianceOhlc,
  normCdf: wasm.normCdf,
  normPdf: wasm.normPdf,
  normCdfWithParams: wasm.normCdfWithParams,
  normPdfWithParams: wasm.normPdfWithParams,
  standardNormalInvCdf: wasm.standardNormalInvCdf,
  studentTCdf: wasm.studentTCdf,
  studentTInvCdf: wasm.studentTInvCdf,
  erf: wasm.erf,
  lnGamma: wasm.lnGamma,
  kahanSum: wasm.kahanSum,
  neumaierSum: wasm.neumaierSum,
  longestPositiveRun: wasm.longestPositiveRun,
};
