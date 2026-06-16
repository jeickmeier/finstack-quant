import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const credit = {
  mertonModelJson: wasm.mertonModelJson,
  creditGradesModelJson: wasm.creditGradesModelJson,
  mertonDefaultProbability: wasm.mertonDefaultProbability,
  mertonDistanceToDefault: wasm.mertonDistanceToDefault,
  mertonImpliedSpread: wasm.mertonImpliedSpread,
  dynamicRecoveryAtNotional: wasm.dynamicRecoveryAtNotional,
  endogenousHazardAtLeverage: wasm.endogenousHazardAtLeverage,
  endogenousHazardAfterPikAccrual: wasm.endogenousHazardAfterPikAccrual,
  dynamicRecoveryConstantJson: wasm.dynamicRecoveryConstantJson,
  endogenousHazardPowerLawJson: wasm.endogenousHazardPowerLawJson,
  creditStateJson: wasm.creditStateJson,
  toggleExerciseThresholdJson: wasm.toggleExerciseThresholdJson,
  toggleExerciseOptimalJson: wasm.toggleExerciseOptimalJson,
};
