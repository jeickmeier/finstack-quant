import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './calibration/schema.js';

// The cap/floor Hull-White calibrators take an optional projection curve
// (`forward`). wasm-bindgen cannot borrow an optional handle, so the two raw
// exports of each are one Rust call each (`forward: None` and `Some(forward)`) and
// the published function picks between them.
const calibrateHullWhiteToCapFloors = (discount, quotes, config, forward = undefined) =>
  forward === undefined || forward === null
    ? wasm.calibrateHullWhiteToCapFloorsWithoutForward(discount, quotes, config)
    : wasm.calibrateHullWhiteToCapFloorsWithForward(discount, quotes, config, forward);
// wasm-bindgen cannot borrow the optional `forward`: picks between the two raw exports.
const bootstrapHullWhiteSigmaScheduleToCapFloors = (
  discount,
  quotes,
  config,
  forward = undefined
) =>
  forward === undefined || forward === null
    ? wasm.bootstrapHullWhiteSigmaScheduleToCapFloorsWithoutForward(discount, quotes, config)
    : wasm.bootstrapHullWhiteSigmaScheduleToCapFloorsWithForward(discount, quotes, config, forward);

/** Quote ingestion, market construction, and explicit model calibration. */
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
