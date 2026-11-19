import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Namespaced TypeScript entry points for Monte Carlo calculations.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const contract = models.optionContractParams({ spot: 100, strike: 100, rate: 0.05, dividend_yield: 0, time_to_expiry: 1, option_type: "call" });
 * const heston = models.hestonParams({ v0: 0.04, kappa: 2, theta: 0.04, vol_of_vol: 0.3, rho: -0.7 });
 * const estimate = models.monteCarlo.priceHeston(contract, heston, {
 *   num_paths: 5000,
 *   seed: 42,
 * });
 * console.log(estimate.mean.amount, estimate.mean.currency);
 * ```
 */
export const monteCarlo = {
  EuropeanPricer: wasm.EuropeanPricer,
  PathDependentPricer: wasm.PathDependentPricer,
  LsmcPricer: wasm.LsmcPricer,
  priceHeston: wasm.priceHeston,
  hestonSatisfiesFeller: wasm.hestonSatisfiesFeller,
  simulatePaths: wasm.simulatePaths,
  finiteDiffDelta: wasm.finiteDiffDelta,
  finiteDiffGamma: wasm.finiteDiffGamma,
  relativeStderr: wasm.relativeStderr,
};
