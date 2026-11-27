import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './margin/schema.js';

// Rust `compute_mva` takes an optional own-survival curve. wasm-bindgen cannot
// borrow an optional handle, so the two raw exports are one Rust call each and
// the published function picks between them.
/**
 * Integrate dated posted-margin funding costs through the horizon.
 * @param imProfile - Complete dated expected posted margin.
 * @param fundingSpread - Flat or dated basis-point spread covering the IM horizon.
 * @param discountCurve - Risk-free curve normalized at the valuation cut.
 * @param ownSurvival - Own hazard curve, or null to omit survival weighting.
 * @returns The MVA cost, average IM and complete contribution intervals.
 * @throws Error if profile, funding spread, the chosen survival or a curve sample is invalid.
 * @facadeType (imProfile: ImProfile | string, fundingSpread: generated.margin.MarginFundingSpread | string, discountCurve: DiscountCurve, ownSurvival: HazardCurve | null) => MvaResult
 */
const computeMva = (imProfile, fundingSpread, discountCurve, ownSurvival) => {
  if (ownSurvival === undefined)
    throw new TypeError('Choose a hazard curve or null for own survival');
  return ownSurvival === null
    ? wasm.computeMvaWithoutSurvival(imProfile, fundingSpread, discountCurve)
    : wasm.computeMvaWithSurvival(imProfile, fundingSpread, discountCurve, ownSurvival);
};

/**
 * Namespaced TypeScript entry points for margin calculations and types.
 * @example
 * ```typescript
 * import init, { margin } from "finstack-quant-wasm";
 * await init();
 * const csa = margin.csaSpecUsdRegulatory();
 * const vm = new margin.VmCalculator(csa).calculate( 1_000_000, 0, "USD", "2026-01-02");
 * console.log(vm.collect_amount.amount); // exact decimal string
 * ```
 */
export const margin = {
  drcPosition: wasm.drcPosition,
  otcMarginSpecBilateralSimm: wasm.otcMarginSpecBilateralSimm,
  otcMarginSpecBilateralSchedule: wasm.otcMarginSpecBilateralSchedule,
  otcMarginSpecCleared: wasm.otcMarginSpecCleared,
  computeBilateralXva: wasm.computeBilateralXva,
  FrtbSbaEngine: wasm.FrtbSbaEngine,
  FrtbSensitivities: wasm.FrtbSensitivities,
  HaircutImCalculator: wasm.HaircutImCalculator,
  SaCcrEngine: wasm.SaCcrEngine,
  ScheduleImCalculator: wasm.ScheduleImCalculator,
  SimmCalculator: wasm.SimmCalculator,
  SimmSensitivities: wasm.SimmSensitivities,
  VmCalculator: wasm.VmCalculator,
  constants: wasm.marginConstants,
  computeMva,
  collateralAssetClassFxAddon: wasm.collateralAssetClassFxAddon,
  collateralAssetClassStandardHaircut: wasm.collateralAssetClassStandardHaircut,
  CsaSpecBuilder: wasm.CsaSpecBuilder,
  csaSpecBuilder: wasm.csaSpecBuilder,
  csaSpecToBuilder: wasm.csaSpecToBuilder,
  csaSpecApplyImTerms: wasm.csaSpecApplyImTerms,
  csaSpecEurRegulatory: wasm.csaSpecEurRegulatory,
  csaSpecRegulatory: wasm.csaSpecRegulatory,
  csaSpecRegulatoryFromConfig: wasm.csaSpecRegulatoryFromConfig,
  csaSpecUsdRegulatory: wasm.csaSpecUsdRegulatory,
  csaSpecValidate: wasm.csaSpecValidate,
  csaSpecWithIm: wasm.csaSpecWithIm,
  csaSpecWithVmThreshold: wasm.csaSpecWithVmThreshold,
  eligibleCollateralScheduleBcbsStandard: wasm.eligibleCollateralScheduleBcbsStandard,
  eligibleCollateralScheduleCashOnly: wasm.eligibleCollateralScheduleCashOnly,
  eligibleCollateralScheduleCheckConcentrationLimits:
    wasm.eligibleCollateralScheduleCheckConcentrationLimits,
  eligibleCollateralScheduleFromFinstackConfig: wasm.eligibleCollateralScheduleFromFinstackConfig,
  eligibleCollateralScheduleHaircutFor: wasm.eligibleCollateralScheduleHaircutFor,
  eligibleCollateralScheduleHaircutForMaturity: wasm.eligibleCollateralScheduleHaircutForMaturity,
  eligibleCollateralScheduleIsEligible: wasm.eligibleCollateralScheduleIsEligible,
  eligibleCollateralScheduleUsTreasuries: wasm.eligibleCollateralScheduleUsTreasuries,
  excessCollateral: wasm.excessCollateral,
  excessCollateralExcessPercentage: wasm.excessCollateralExcessPercentage,
  excessCollateralHasExcess: wasm.excessCollateralHasExcess,
  excessCollateralHasShortfall: wasm.excessCollateralHasShortfall,
  exposureProfileValidate: wasm.exposureProfileValidate,
  fundingConfigEffectiveBenefitBp: wasm.fundingConfigEffectiveBenefitBp,
  haircut01: wasm.haircut01,
  haircut01HaircutBp: wasm.haircut01HaircutBp,
  imDecayProfileFactor: wasm.imDecayProfileFactor,
  imDecayProfileLinearToMaturity: wasm.imDecayProfileLinearToMaturity,
  imDecayProfileSqrtTime: wasm.imDecayProfileSqrtTime,
  imProfileFromSimm: wasm.imProfileFromSimm,
  imProfileValidate: wasm.imProfileValidate,
  imResultBreakdownAmount: wasm.imResultBreakdownAmount,
  imResultBreakdownKeys: wasm.imResultBreakdownKeys,
  marginFundingCost: wasm.marginFundingCost,
  marginFundingCostCostForPeriod: wasm.marginFundingCostCostForPeriod,
  marginFundingCostSpreadBp: wasm.marginFundingCostSpreadBp,
  marginFundingCostSpreadRate: wasm.marginFundingCostSpreadRate,
  marginUtilization: wasm.marginUtilization,
  marginUtilizationIsAdequate: wasm.marginUtilizationIsAdequate,
  marginUtilizationRatio: wasm.marginUtilizationRatio,
  marginUtilizationShortfall: wasm.marginUtilizationShortfall,
  saCcrNettingSetConfigMargined: wasm.saCcrNettingSetConfigMargined,
  saCcrNettingSetConfigUnmargined: wasm.saCcrNettingSetConfigUnmargined,
  saCcrNettingSetConfigValidate: wasm.saCcrNettingSetConfigValidate,
  vmResultNetMargin: wasm.vmResultNetMargin,
  vmResultRequiresCall: wasm.vmResultRequiresCall,
  schema,
};
