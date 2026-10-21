import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './core/schema.js';

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

// `FxMatrix.rate(base, quote, date, policy?)`: an omitted policy is the Rust
// default (`new FxConversionPolicy()`, cashflow date), created once after
// `init()` and only borrowed.
let defaultFxConversionPolicy;
const rawFxRate = wasm.FxMatrix.prototype.rate;
wasm.FxMatrix.prototype.rate = function rate(base, quote, date, policy = undefined) {
  const resolved =
    policy === undefined || policy === null
      ? (defaultFxConversionPolicy ??= new wasm.FxConversionPolicy())
      : policy;
  return rawFxRate.call(this, base, quote, date, resolved);
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
    if (curve instanceof curveClass) {
      insertTyped.call(this, curve);
      return this;
    }
  }
  const error = new TypeError(
    'curve: expected a DiscountCurve, ForwardCurve, HazardCurve, InflationCurve, PriceCurve, BaseCorrelationCurve, VolSurface, FxDeltaVolSurface or VolCube'
  );
  error.kind = 'invalid_type';
  throw error;
};

// Fluent mutators update the receiver in place and return it, as the Python
// twins do (Rust `MarketContext::insert_*` and the schedule builder return the
// builder). wasm-bindgen cannot return the receiver of a `&mut self` method,
// so the Rust methods return nothing and `this` is returned here.
const chain = (cls, names) => {
  for (const name of names) {
    const raw = cls.prototype[name];
    cls.prototype[name] = {
      [name](...args) {
        raw.apply(this, args);
        return this;
      },
    }[name];
  }
};
chain(wasm.MarketContext, [
  'insertFx',
  'insertPrice',
  'insertCreditIndex',
  'insertSeries',
  'insertInflationIndex',
]);
chain(wasm.ScheduleBuilder, [
  'frequency',
  'stubRule',
  'adjustWith',
  'paymentLagDays',
  'fixingLagBusinessDays',
  'endOfMonth',
  'cdsImm',
  'imm',
  'errorPolicy',
]);

// `sifmaSettlementDate(month, year, settlementClass?)`: wasm-bindgen cannot
// take an optional borrowed handle, so an omitted class runs the raw
// `sifmaSettlementDateDefaultClass`, which applies the Rust default
// (`SifmaSettlementClass::default()`, Class A).
function sifmaSettlementDate(month, year, settlementClass = undefined) {
  return settlementClass === undefined || settlementClass === null
    ? wasm.sifmaSettlementDateDefaultClass(month, year)
    : wasm.sifmaSettlementDate(month, year, settlementClass);
}

export const core = {
  Currency: wasm.Currency,
  Money: wasm.Money,
  Rate: wasm.Rate,
  Bps: wasm.Bps,
  CreditRating: wasm.CreditRating,
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
  days30360: wasm.days30360,
  days30e360Isda: wasm.days30e360Isda,
  Tenor: wasm.Tenor,
  TenorUnit: wasm.TenorUnit,
  createDate: wasm.createDate,
  dateFromEpochDays: wasm.dateFromEpochDays,
  daysSinceEpoch: wasm.daysSinceEpoch,
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
  nextThirdFriday: wasm.nextThirdFriday,
  SifmaSettlementClass: wasm.SifmaSettlementClass,
  sifmaSettlementDate,
  estimatedSifmaSettlementDate: wasm.estimatedSifmaSettlementDate,
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
  VolCubeExpirySlice: wasm.VolCubeExpirySlice,
  FxDeltaVolSurface: wasm.FxDeltaVolSurface,
  ScalarTimeSeries: wasm.ScalarTimeSeries,
  InflationIndex: wasm.InflationIndex,
  MarketContext: wasm.MarketContext,
  ConvertedMoney: wasm.ConvertedMoney,
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
  logNormCdf: wasm.logNormCdf,
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
  cashFlowGetBalanceDate: wasm.cashFlowGetBalanceDate,
  cashFlowValidate: wasm.cashFlowValidate,
  cfKindIsCashSettlement: wasm.cfKindIsCashSettlement,
  cfKindIsInterestLike: wasm.cfKindIsInterestLike,
  cfKindIsPrincipalLike: wasm.cfKindIsPrincipalLike,
  cfKindParse: wasm.cfKindParse,
  schema,
};
