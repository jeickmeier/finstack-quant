import * as wasm from '../pkg/finstack_quant_wasm.js';

/** Quote ingestion, market construction, and explicit model calibration. */
export const calibration = {
  calibrate: wasm.calibrate,
  validateCalibrationJson: wasm.validateCalibrationJson,
  dryRun: wasm.dryRun,
  dryRunJson: wasm.dryRunJson,
  calibrateBermudanLmmBaseVol: wasm.calibrateBermudanLmmBaseVol,
};
