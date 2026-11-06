import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './calibration/schema.js';

// wasm-bindgen cannot borrow the optional `forward`: picks between the two raw exports.
/**
 * Fit scalar Hull-White `(kappa, sigma)` to cap/floor quotes.
 *
 * Twin of Python `calibrate_hull_white_to_cap_floors` (Rust
 * `calibrate_hull_white_to_cap_floors`).
 * @param discount - Discounting curve; its base date is the valuation date.
 * @param quotes - Array (or JSON) of `CapFloorQuote` objects: `maturity` in years, `strike` and `volatility` as decimals, `is_cap`, `is_normal_vol`. A single quote requires `config.fixed_kappa`.
 * @param config - `CapFloorCalibrationConfig` object: required `fit_tolerance` (normal-vol units), optional `frequency` (default `"semi_annual"`), `fixed_kappa` and `initial_guess`.
 * @param forward - Curve projecting the caplet forwards; the published function defaults it to `discount`.
 * @returns `[params, report]`: the fitted `HullWhiteCalibrationParams` and the `CalibrationReport`.
 * @throws Error - Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and a `FinstackError` if no quotes are supplied, a single quote is given without `fixed_kappa`, a quote or the config is invalid (`kind: "validation"`), or the solver fails to converge.
 *
 * @facadeType (discount: DiscountCurve, quotes: readonly generated.calibration.CapFloorQuote[] | string, config: generated.calibration.CapFloorCalibrationConfig | string, forward?: DiscountCurve | null) => [generated.models.HullWhiteCalibrationParams, generated.calibration.CalibrationReport]
 */
const calibrateHullWhiteToCapFloors = (discount, quotes, config, forward = undefined) =>
  forward === undefined || forward === null
    ? wasm.calibrateHullWhiteToCapFloorsWithoutForward(discount, quotes, config)
    : wasm.calibrateHullWhiteToCapFloorsWithForward(discount, quotes, config, forward);
// wasm-bindgen cannot borrow the optional `forward`: picks between the two raw exports.
/**
 * Bootstrap a piecewise-constant Hull-White sigma schedule to cap/floor quotes.
 *
 * Twin of Python `bootstrap_hull_white_sigma_schedule_to_cap_floors` (Rust
 * `bootstrap_hull_white_sigma_schedule_to_cap_floors`).
 * @param discount - Discounting curve; its base date is the valuation date.
 * @param quotes - Array (or JSON) of `CapFloorQuote` objects with distinct maturities in years; each maturity adds one constant-sigma interval.
 * @param config - `PiecewiseSigmaCalibrationConfig` object: `fixed_kappa`, `sigma_min`, `sigma_max` (absolute rate volatility), `fit_tolerance` and optional `frequency` (default `"semi_annual"`).
 * @param forward - Curve projecting the caplet forwards; the published function defaults it to `discount`.
 * @returns `[params, report]`: the piecewise `HullWhiteParams` handle (`models.rates.hullWhite.HullWhiteParams`) and the `CalibrationReport` object.
 * @throws Error - Throws a `TypeError` (`kind: "invalid_type"`) for a wrong argument type and a `FinstackError` if no quotes are supplied, maturities repeat, the configuration bounds are invalid (`kind: "validation"`), or an interval's sigma cannot be bracketed or solved.
 *
 * @facadeType (discount: DiscountCurve, quotes: readonly generated.calibration.CapFloorQuote[] | string, config: generated.calibration.PiecewiseSigmaCalibrationConfig | string, forward?: DiscountCurve | null) => [HullWhiteParams, generated.calibration.CalibrationReport]
 */
const bootstrapHullWhiteSigmaScheduleToCapFloors = (
  discount,
  quotes,
  config,
  forward = undefined
) =>
  forward === undefined || forward === null
    ? wasm.bootstrapHullWhiteSigmaScheduleToCapFloorsWithoutForward(discount, quotes, config)
    : wasm.bootstrapHullWhiteSigmaScheduleToCapFloorsWithForward(discount, quotes, config, forward);

/**
 * Quote ingestion, market construction, and explicit model calibration.
 * @example
 * ```typescript
 * import init, { calibration } from "finstack-quant-wasm";
 * await init();
 * const report = calibration.dryRun({
 *   schema: "finstack_quant.calibration/1",
 *   plan: { id: "smoke", description: null, quote_sets: {}, steps: [], settings: {} }
 * });
 * console.log(report.errors);
 * ```
 */
export const calibration = {
  calibrate: wasm.calibrate,
  validateCalibration: wasm.validateCalibration,
  dryRun: wasm.dryRun,
  calibrateBermudanLmmBaseVol: wasm.calibrateBermudanLmmBaseVol,
  calibrationEnvelopeContentHash: wasm.calibrationEnvelopeContentHash,
  calibrationResultContentHash: wasm.calibrationResultContentHash,
  calibrationResultStepReport: wasm.calibrationResultStepReport,
  calibrationResultResiduals: wasm.calibrationResultResiduals,
  rateQuoteDeposit: wasm.rateQuoteDeposit,
  rateQuoteFra: wasm.rateQuoteFra,
  rateQuoteFutures: wasm.rateQuoteFutures,
  rateQuoteSwap: wasm.rateQuoteSwap,
  cdsQuoteParSpread: wasm.cdsQuoteParSpread,
  cdsQuoteUpfront: wasm.cdsQuoteUpfront,
  volQuoteOptionVol: wasm.volQuoteOptionVol,
  volQuoteSwaptionVol: wasm.volQuoteSwaptionVol,
  volQuoteCapFloorVol: wasm.volQuoteCapFloorVol,
  inflationQuoteInflationSwap: wasm.inflationQuoteInflationSwap,
  inflationQuoteYoyInflationSwap: wasm.inflationQuoteYoyInflationSwap,
  xccyQuote: wasm.xccyQuote,
  cdsTrancheQuote: wasm.cdsTrancheQuote,
  rateBoundsForCurrency: wasm.rateBoundsForCurrency,
  rateBoundsEmergingMarkets: wasm.rateBoundsEmergingMarkets,
  calibrationStepDiscount: wasm.calibrationStepDiscount,
  calibrationStepForward: wasm.calibrationStepForward,
  calibrationStepHazard: wasm.calibrationStepHazard,
  calibrationStepInflation: wasm.calibrationStepInflation,
  calibrationStepVolSurface: wasm.calibrationStepVolSurface,
  calibrationStepSwaptionVol: wasm.calibrationStepSwaptionVol,
  calibrationStepBaseCorrelation: wasm.calibrationStepBaseCorrelation,
  calibrationStepStudentT: wasm.calibrationStepStudentT,
  calibrationStepHullWhite: wasm.calibrationStepHullWhite,
  calibrationStepCapFloorHullWhite: wasm.calibrationStepCapFloorHullWhite,
  calibrationStepSviSurface: wasm.calibrationStepSviSurface,
  calibrationStepXccyBasis: wasm.calibrationStepXccyBasis,
  calibrationStepParametric: wasm.calibrationStepParametric,
  calibrateHullWhiteToSwaptions: wasm.calibrateHullWhiteToSwaptions,
  calibrateHullWhiteToCapFloors,
  bootstrapHullWhiteSigmaScheduleToCapFloors,
  validateSurface: wasm.validateSurface,
  validateCalendarSpread: wasm.validateCalendarSpread,
  validateButterflySpread: wasm.validateButterflySpread,
  validateVolBounds: wasm.validateVolBounds,
  schema,
};
