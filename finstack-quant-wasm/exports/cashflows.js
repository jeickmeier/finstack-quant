import * as wasm from '../pkg/finstack_quant_wasm.js';

// Rust `CashFlowBuilder::build` and `CashflowScheduleBuildSpec::build` take an
// optional market. wasm-bindgen cannot borrow an optional handle, so the
// `None` and `Some(market)` calls are exported separately and joined here.
const rawBuild = wasm.CashFlowBuilder.prototype.build;
const rawBuildWithMarket = wasm.CashFlowBuilder.prototype.buildWithMarket;
delete wasm.CashFlowBuilder.prototype.buildWithMarket;
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

export const cashflows = {
  AccrualIndex: wasm.AccrualIndex,
  CashFlowBuilder: wasm.CashFlowBuilder,
  CashFlowSchedule: wasm.CashFlowSchedule,
  absToSmm: wasm.absToSmm,
  accruedInterest: wasm.accruedInterest,
  accruedInterestAmount: wasm.accruedInterestAmount,
  aggregateByPeriod: wasm.aggregateByPeriod,
  aggregateCashflowsChecked: wasm.aggregateCashflowsChecked,
  amortizationSpecCustomPrincipal: wasm.amortizationSpecCustomPrincipal,
  amortizationSpecLinearBetween: wasm.amortizationSpecLinearBetween,
  amortizationSpecLinearTo: wasm.amortizationSpecLinearTo,
  amortizationSpecPercentOfOriginalPerPeriod: wasm.amortizationSpecPercentOfOriginalPerPeriod,
  amortizationSpecPercentOfRemainingPerPeriod: wasm.amortizationSpecPercentOfRemainingPerPeriod,
  amortizationSpecStepRemaining: wasm.amortizationSpecStepRemaining,
  buildCashflowSchedule: (spec, market = undefined) =>
    market === undefined || market === null
      ? wasm.buildCashflowSchedule(spec)
      : wasm.buildCashflowScheduleWithMarket(spec, market),
  buildCashflowScheduleJson: wasm.buildCashflowScheduleJson,
  calendarYearLadder: wasm.calendarYearLadder,
  cashFlowGetBalanceDate: wasm.cashFlowGetBalanceDate,
  cashFlowValidate: wasm.cashFlowValidate,
  cashFlowWithAccrual: wasm.cashFlowWithAccrual,
  cashFlowWithPrincipalDate: wasm.cashFlowWithPrincipalDate,
  cashFlowWithPrincipalDelta: wasm.cashFlowWithPrincipalDelta,
  cdrToMdr: wasm.cdrToMdr,
  cfKindIsInterestLike: wasm.cfKindIsInterestLike,
  cfKindIsPrincipalLike: wasm.cfKindIsPrincipalLike,
  cfKindParse: wasm.cfKindParse,
  couponTypeSplit: wasm.couponTypeSplit,
  cprToSmm: wasm.cprToSmm,
  datedFlows: wasm.datedFlows,
  datedFlowsJson: wasm.datedFlowsJson,
  defaultModelSpecCdr2pct: wasm.defaultModelSpecCdr2pct,
  defaultModelSpecConstantCdr: wasm.defaultModelSpecConstantCdr,
  defaultModelSpecCumulativeLoss: wasm.defaultModelSpecCumulativeLoss,
  defaultModelSpecMdr: wasm.defaultModelSpecMdr,
  defaultModelSpecSda: wasm.defaultModelSpecSda,
  defaultModelSpecTiming: wasm.defaultModelSpecTiming,
  defaultModelSpecValidate: wasm.defaultModelSpecValidate,
  defaultModelSpecVector: wasm.defaultModelSpecVector,
  exCouponRuleExDate: wasm.exCouponRuleExDate,
  feeBaseUndrawn: wasm.feeBaseUndrawn,
  feeSpecFixed: wasm.feeSpecFixed,
  feeSpecPeriodicBp: wasm.feeSpecPeriodicBp,
  floatingLegCompoundingCompoundedInArrears: wasm.floatingLegCompoundingCompoundedInArrears,
  floatingLegCompoundingCompoundedWithObservationShift:
    wasm.floatingLegCompoundingCompoundedWithObservationShift,
  floatingLegCompoundingCompoundedWithRateCutoff:
    wasm.floatingLegCompoundingCompoundedWithRateCutoff,
  floatingRateFallbackFixedRate: wasm.floatingRateFallbackFixedRate,
  floatingRateSpecEuribor3m: wasm.floatingRateSpecEuribor3m,
  floatingRateSpecSofr: wasm.floatingRateSpecSofr,
  floatingRateSpecSonia: wasm.floatingRateSpecSonia,
  floatingRateSpecValidate: wasm.floatingRateSpecValidate,
  isCashSettlementKind: wasm.isCashSettlementKind,
  materializeFixings: (market, schedules, oldDate, newDate) =>
    wasm.materializeFixings(market, scheduleJsonList(schedules), oldDate, newDate),
  mdrToCdr: wasm.mdrToCdr,
  mergeCashflowSchedules: (schedules, notional, dayCount) =>
    wasm.mergeCashflowSchedules(scheduleJsonList(schedules), notional, dayCount),
  notionalCurrency: wasm.notionalCurrency,
  notionalPar: wasm.notionalPar,
  notionalValidate: wasm.notionalValidate,
  periodAggregationGetAmount: wasm.periodAggregationGetAmount,
  prepaymentModelSpecAbs: wasm.prepaymentModelSpecAbs,
  prepaymentModelSpecCmbsWithLockout: wasm.prepaymentModelSpecCmbsWithLockout,
  prepaymentModelSpecConstantCpr: wasm.prepaymentModelSpecConstantCpr,
  prepaymentModelSpecPsa: wasm.prepaymentModelSpecPsa,
  prepaymentModelSpecPsa100: wasm.prepaymentModelSpecPsa100,
  prepaymentModelSpecSmm: wasm.prepaymentModelSpecSmm,
  prepaymentModelSpecValidate: wasm.prepaymentModelSpecValidate,
  prepaymentModelSpecVector: wasm.prepaymentModelSpecVector,
  recoveryModelSpecRecoveryRate: wasm.recoveryModelSpecRecoveryRate,
  recoveryModelSpecValidate: wasm.recoveryModelSpecValidate,
  recoveryModelSpecWithSeverityVector: wasm.recoveryModelSpecWithSeverityVector,
  scheduleCalendarYearLadder: wasm.scheduleCalendarYearLadder,
  scheduleFromClassifiedFlows: wasm.scheduleFromClassifiedFlows,
  scheduleFromDatedFlows: wasm.scheduleFromDatedFlows,
  scheduleOutstandingByDate: wasm.scheduleOutstandingByDate,
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
  scheduleWal: wasm.scheduleWal,
  smmToCpr: wasm.smmToCpr,
  validateCashflowScheduleJson: wasm.validateCashflowScheduleJson,
};
