import { chain } from './chain.js';
import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './core/schema.js';

// Rust `DayCount::year_fraction` / `signed_year_fraction` take a context; the
// published methods make it optional, which wasm-bindgen cannot express for a
// borrowed handle. An omitted context is the Rust default
// (`new DayCountContext()`), created once after `init()` and only borrowed.
// The Rust parameter is declared optional (`unchecked_optional_param_type`).
let defaultDayCountContext;
// wasm-bindgen cannot borrow an optional context: resolves an omitted one to the Rust default.
const dayCountContext = (ctx) =>
  ctx === undefined || ctx === null ? (defaultDayCountContext ??= new wasm.DayCountContext()) : ctx;
const rawYearFraction = wasm.DayCount.prototype.yearFraction;
const rawToYearsWithContext = wasm.Tenor.prototype.toYearsWithContext;
const rawSignedYearFraction = wasm.DayCount.prototype.signedYearFraction;
// wasm-bindgen cannot borrow an optional context: `dayCountContext` resolves it.
wasm.DayCount.prototype.yearFraction = function yearFraction(start, end, ctx = undefined) {
  return rawYearFraction.call(this, start, end, dayCountContext(ctx));
};
// wasm-bindgen cannot borrow an optional context: `dayCountContext` resolves it.
wasm.DayCount.prototype.signedYearFraction = function signedYearFraction(
  start,
  end,
  ctx = undefined
) {
  return rawSignedYearFraction.call(this, start, end, dayCountContext(ctx));
};
// wasm-bindgen cannot borrow an optional context: `dayCountContext` resolves it.
wasm.Tenor.prototype.toYearsWithContext = function toYearsWithContext(
  asOf,
  dayCount,
  calendarCode = undefined,
  convention = undefined,
  ctx = undefined
) {
  return rawToYearsWithContext.call(
    this,
    asOf,
    dayCount,
    calendarCode,
    convention,
    dayCountContext(ctx)
  );
};

// `MarketContext.insert(curve)` takes any of nine curve and surface classes.
// wasm-bindgen cannot express a borrowed union of handles, so the typed raw
// methods are bound in Rust and the dispatch on the handle's class is done
// here; the raw methods are removed from the published prototype (Rust marks
// them `skip_typescript`, so they are not declared either).
/**
 * Store a curve or surface under its own id, replacing any item with the same id.
 *
 * The class of the handle selects the store: discount, forward, hazard,
 * inflation, price and base-correlation curves; volatility surfaces; FX
 * delta-quoted surfaces; and SABR cubes. The context is changed in place.
 *
 * @param curve - A `DiscountCurve`, `ForwardCurve`, `HazardCurve`, `InflationCurve`, `PriceCurve`, `BaseCorrelationCurve`, `VolSurface`, `FxDeltaVolSurface` or `VolCube` handle; its data is shared with the context, not copied.
 * @returns This context, updated in place, so calls can be chained.
 * @throws `TypeError` (kind `invalid_type`) if `curve` is not one of those handles.
 *
 * @facadeClass MarketContext
 * @member insert(curve: MarketContextCurve): this
 */
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
// wasm-bindgen cannot express a borrowed union of handles: dispatches on the handle's class.
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
  'grid',
  'adjustWith',
  'unadjusted',
  'paymentLagDays',
  'fixingLagBusinessDays',
]);

/**
 * Namespaced TypeScript entry points for core calculations and types.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * console.log(core.mean([1, 2, 3]));
 * ```
 */
export const core = {
  toleranceConfigNew: wasm.toleranceConfigNew,
  ratingLevelTryNew: wasm.ratingLevelTryNew,
  scorecardScaleTryNew: wasm.scorecardScaleTryNew,
  sabrParameterDataNew: wasm.sabrParameterDataNew,
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
  Tenor: wasm.Tenor,
  TenorUnit: wasm.TenorUnit,
  fxStandardSettlementDays: wasm.fxStandardSettlementDays,
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
  nextThirdFriday: wasm.nextThirdFriday,
  SifmaSettlementClass: wasm.SifmaSettlementClass,
  sifmaSettlementDate: wasm.sifmaSettlementDate,
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
  fxPairConvention: wasm.fxPairConvention,
  invertFxRate: wasm.invertFxRate,
  applyLowerTriangular: wasm.applyLowerTriangular,
  choleskyDecomposition: wasm.choleskyDecomposition,
  choleskySolve: wasm.choleskySolve,
  symmetricEigen: wasm.symmetricEigen,
  ledoitWolfShrinkage: wasm.ledoitWolfShrinkage,
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
  cashFlowGetBalanceDate: wasm.cashFlowGetBalanceDate,
  cashFlowValidate: wasm.cashFlowValidate,
  cashFlowWithAccrual: wasm.cashFlowWithAccrual,
  cashFlowWithPrincipal: wasm.cashFlowWithPrincipal,
  cfKindIsCashSettlement: wasm.cfKindIsCashSettlement,
  cfKindIsInterestLike: wasm.cfKindIsInterestLike,
  cfKindIsPrincipalLike: wasm.cfKindIsPrincipalLike,
  cfKindParse: wasm.cfKindParse,
  schema,
};
