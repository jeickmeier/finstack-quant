import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const correlation = {
  CopulaSpec: wasm.CopulaSpec,
  Copula: wasm.Copula,
  RecoverySpec: wasm.RecoverySpec,
  RecoveryModel: wasm.RecoveryModel,
  PortfolioLossResult: wasm.PortfolioLossResult,
  CorrelatedBernoulli: wasm.CorrelatedBernoulli,
  LatentFactorSpec: wasm.LatentFactorSpec,
  LatentFactorKind: wasm.LatentFactorKind,
  LatentSingleFactor: wasm.LatentSingleFactor,
  LatentTwoFactor: wasm.LatentTwoFactor,
  LatentMultiFactor: wasm.LatentMultiFactor,
  correlationBounds: wasm.correlationBounds,
  jointProbabilities: wasm.jointProbabilities,
  validateCorrelationMatrix: wasm.validateCorrelationMatrix,
  nearestCorrelation: wasm.nearestCorrelation,
  choleskyDecompose: wasm.choleskyDecompose,
  maxPortfolioLossPaths: wasm.maxPortfolioLossPaths,
  simulatePortfolioLoss: wasm.simulatePortfolioLoss,
};
