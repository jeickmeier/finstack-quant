import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Namespaced TypeScript entry points for Monte Carlo calculations.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const params = { r: 0.05, q: 0.0, v0: 0.04, kappa: 2.0, theta: 0.04, sigma_v: 0.3, rho: -0.7 };
 * const est = models.monteCarlo.priceHeston(100, 100, 1.0, params, true, {
 *   num_paths: 5000,
 *   seed: 42,
 * });
 * console.log(est.mean.amount, est.mean.currency);
 * // Path count and seed default to the Rust registry values.
 * const byDefault = models.monteCarlo.priceHeston(100, 100, 1.0, params, false);
 * console.log(byDefault.num_paths); // 100000
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
