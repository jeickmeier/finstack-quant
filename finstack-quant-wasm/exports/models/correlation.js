import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const correlation = {
  CopulaSpec: wasm.CopulaSpec,
  Copula: wasm.Copula,
  RecoverySpec: wasm.RecoverySpec,
  RecoveryModel: wasm.RecoveryModel,
  PortfolioLossResult: wasm.PortfolioLossResult,
  correlationBounds: wasm.correlationBounds,
  jointProbabilities: wasm.jointProbabilities,
  validateCorrelationMatrix: wasm.validateCorrelationMatrix,
  nearestCorrelation: wasm.nearestCorrelation,
};
