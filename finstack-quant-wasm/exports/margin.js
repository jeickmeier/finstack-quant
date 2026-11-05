import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './margin/schema.js';

// Rust `compute_mva` takes an optional own-survival curve. wasm-bindgen cannot
// borrow an optional handle, so the two raw exports are one Rust call each and
// the published function picks between them.
/**
 * Margin valuation adjustment over an expected-IM profile.
 *
 * `MVA = sum_i s(t_i) * IM(t_i) * DF(t_i) * S(t_i) * dt_i` on the profile
 * grid with a `t = 0` bucket edge; IM is flat before the first grid point.
 * @param imProfile - `ImProfile` (object or JSON) of expected IM, for example from `imProfileFromSimm`.
 * @param fundingSpreadCurve - Array of `[timeYears, spreadBp]` pairs, linearly interpolated with flat extrapolation; one pair is a flat spread.
 * @param discountCurve - Risk-free discount curve handle.
 * @param survivalCurve - The bank's own hazard curve handle; omitted treats survival as 1 for all times.
 * @returns The `MvaResult` (`mva`, `average_im`, `im_profile`) as a plain object.
 * @throws Error - Throws if the profile or spread curve is malformed or invalid, or a curve evaluation is non-finite.
 *
 * @facadeType (imProfile: ImProfile | string, fundingSpreadCurve: readonly (readonly [number, number])[] | string, discountCurve: DiscountCurve, survivalCurve?: HazardCurve | null) => MvaResult
 */
const computeMva = (imProfile, fundingSpreadCurve, discountCurve, survivalCurve = undefined) =>
  survivalCurve === undefined || survivalCurve === null
    ? wasm.computeMvaWithoutSurvival(imProfile, fundingSpreadCurve, discountCurve)
    : wasm.computeMvaWithSurvival(imProfile, fundingSpreadCurve, discountCurve, survivalCurve);

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
  clearingStatusCleared: wasm.clearingStatusCleared,
  collateralAssetClassFromStr: wasm.collateralAssetClassFromStr,
  collateralAssetClassFxAddon: wasm.collateralAssetClassFxAddon,
  collateralAssetClassStandardHaircut: wasm.collateralAssetClassStandardHaircut,
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
  frtbSbaCharge: wasm.frtbSbaCharge,
  fundingConfigEffectiveBenefitBp: wasm.fundingConfigEffectiveBenefitBp,
  fundingConfigEffectiveMarginSpreadBp: wasm.fundingConfigEffectiveMarginSpreadBp,
  haircut01: wasm.haircut01,
  haircut01HaircutBp: wasm.haircut01HaircutBp,
  imDecayProfileFactor: wasm.imDecayProfileFactor,
  imDecayProfileLinearToMaturity: wasm.imDecayProfileLinearToMaturity,
  imDecayProfileSqrtTime: wasm.imDecayProfileSqrtTime,
  imMethodologyFromStr: wasm.imMethodologyFromStr,
  imProfileFromSimm: wasm.imProfileFromSimm,
  imProfileValidate: wasm.imProfileValidate,
  imResultBreakdownAmount: wasm.imResultBreakdownAmount,
  imResultBreakdownKeys: wasm.imResultBreakdownKeys,
  marginCallTypeFromStr: wasm.marginCallTypeFromStr,
  marginFundingCost: wasm.marginFundingCost,
  marginFundingCostCostForPeriod: wasm.marginFundingCostCostForPeriod,
  marginFundingCostSpread: wasm.marginFundingCostSpread,
  marginTenorFromStr: wasm.marginTenorFromStr,
  marginUtilization: wasm.marginUtilization,
  marginUtilizationIsAdequate: wasm.marginUtilizationIsAdequate,
  marginUtilizationRatio: wasm.marginUtilizationRatio,
  marginUtilizationShortfall: wasm.marginUtilizationShortfall,
  nettingSetIdBilateral: wasm.nettingSetIdBilateral,
  nettingSetIdCleared: wasm.nettingSetIdCleared,
  saCcrNettingSetConfigMargined: wasm.saCcrNettingSetConfigMargined,
  saCcrNettingSetConfigUnmargined: wasm.saCcrNettingSetConfigUnmargined,
  saCcrNettingSetConfigValidate: wasm.saCcrNettingSetConfigValidate,
  saccrEad: wasm.saccrEad,
  vmResultNetMargin: wasm.vmResultNetMargin,
  vmResultRequiresCall: wasm.vmResultRequiresCall,
  schema,
};
