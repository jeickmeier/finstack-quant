import * as wasm from '../../pkg/finstack_wasm.js';

export const correlation = {
  CopulaSpec: wasm.CopulaSpec,
  Copula: wasm.Copula,
  RecoverySpec: wasm.RecoverySpec,
  RecoveryModel: wasm.RecoveryModel,
  correlationBounds: wasm.correlationBounds,
  jointProbabilities: wasm.jointProbabilities,
  validateCorrelationMatrix: wasm.validateCorrelationMatrix,
  nearestCorrelation: wasm.nearestCorrelation,
};
