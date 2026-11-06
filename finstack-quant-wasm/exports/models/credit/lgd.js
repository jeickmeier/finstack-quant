import * as wasm from '../../../pkg/finstack_quant_wasm.js';

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
