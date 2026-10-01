import * as wasm from '../pkg/finstack_quant_wasm.js';

// The cap/floor Hull-White calibrators take an optional projection curve
// (`forward`). wasm-bindgen cannot borrow an optional class handle
// (`Option<&T>`), so the raw functions take it as a required handle and the
// published functions pass `discount` when it is omitted — the Rust default
// (`forward: None` projects on the discount curve). Same workaround as the
// optional `DayCountContext` in `exports/core.js`.
const projectionCurve = (discount, forward) =>
  forward === undefined || forward === null ? discount : forward;
const calibrateHullWhiteToCapFloors = (discount, quotes, config, forward = undefined) =>
  wasm.calibrateHullWhiteToCapFloors(discount, quotes, config, projectionCurve(discount, forward));
const bootstrapHullWhiteSigmaScheduleToCapFloors = (
  discount,
  quotes,
  config,
  forward = undefined
) =>
  wasm.bootstrapHullWhiteSigmaScheduleToCapFloors(
    discount,
    quotes,
    config,
    projectionCurve(discount, forward)
  );

/** Quote ingestion, market construction, and explicit model calibration. */
export const calibration = {
  calibrate: wasm.calibrate,
  validateCalibration: wasm.validateCalibration,
  validateCalibrationJson: wasm.validateCalibrationJson,
  dryRun: wasm.dryRun,
  dryRunJson: wasm.dryRunJson,
  calibrateBermudanLmmBaseVol: wasm.calibrateBermudanLmmBaseVol,
  calibrationEnvelopeContentHash: wasm.calibrationEnvelopeContentHash,
  calibrationResultContentHash: wasm.calibrationResultContentHash,
  calibrationResultStepReport: wasm.calibrationResultStepReport,
  calibrationResultStepReportJson: wasm.calibrationResultStepReportJson,
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
  hullWhiteParamsSigmaAt: wasm.hullWhiteParamsSigmaAt,
};
