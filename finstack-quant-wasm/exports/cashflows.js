import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './cashflows/schema.js';

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
  accruedInterestAmount: wasm.accruedInterestAmount,
  aggregateByPeriod: wasm.aggregateByPeriod,
  aggregateCashflowsChecked: wasm.aggregateCashflowsChecked,
  annualToMonthly: wasm.annualToMonthly,
  buildCashflowSchedule: (spec, market = undefined) =>
    market === undefined || market === null
      ? wasm.buildCashflowSchedule(spec)
      : wasm.buildCashflowScheduleWithMarket(spec, market),
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
  materializeFixings: (market, schedules, oldDate, newDate) =>
    wasm.materializeFixings(market, scheduleJsonList(schedules), oldDate, newDate),
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
