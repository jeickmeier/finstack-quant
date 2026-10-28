import type { calibration } from "finstack-quant-wasm";
import type { CalibrationApi } from "../finstack-contract";
import type { ServiceContext } from "./context";
/** calibration facade members this slice reads. */
export type CalibrationNative = {
  calibration: Pick<
    typeof calibration,
    "calibrate" | "dryRun" | "validateCalibration"
  >;
};
/** CalibrationApi over the calibration facade namespace. */
export function createCalibrationService(
  native: CalibrationNative,
  context: ServiceContext,
): CalibrationApi {
  return {
    validateCalibration(json) {
      return context.result(() => native.calibration.validateCalibration(json));
    },
    dryRun(json) {
      return context.result(() => native.calibration.dryRun(json));
    },
    calibrate(json) {
      return context.result(() => native.calibration.calibrate(json));
    },
  };
}
