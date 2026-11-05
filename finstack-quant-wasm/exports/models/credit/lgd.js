import * as wasm from '../../../pkg/finstack_quant_wasm.js';

/**
 * Loss given default and exposure at default: Beta recovery, workout LGD, downturn LGD and EAD.
 * Mirrors Rust `credit::lgd`.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const recovery = new models.credit.lgd.BetaRecovery(0.4, 0.2);
 * console.log(recovery.quantile(0.95));
 * recovery.free();
 * ```
 */
export const lgd = {
  BetaRecovery: wasm.BetaRecovery,
  WorkoutCosts: wasm.WorkoutCosts,
  WorkoutLgd: wasm.WorkoutLgd,
  WorkoutLgdBuilder: wasm.WorkoutLgdBuilder,
  DownturnLgd: wasm.DownturnLgd,
  EadCalculator: wasm.EadCalculator,
  seniorityCalibrationIds: wasm.seniorityCalibrationIds,
  seniorityRecoveryStats: wasm.seniorityRecoveryStats,
  collateralPieceLiquidationValue: wasm.collateralPieceLiquidationValue,
};
