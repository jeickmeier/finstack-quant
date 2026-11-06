import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Namespaced TypeScript entry points for correlation calculations and types.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const [lower, upper] = models.correlation.correlationBounds(0.1, 0.2);
 * console.log(lower, upper);
 * ```
 */
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
  validateCorrelationMatrix: wasm.validateCorrelationMatrix,
  nearestCorrelation: wasm.nearestCorrelation,
  choleskyCorrelation: wasm.choleskyCorrelation,
  maxPortfolioLossPaths: wasm.maxPortfolioLossPaths,
  simulatePortfolioLoss: wasm.simulatePortfolioLoss,
};
