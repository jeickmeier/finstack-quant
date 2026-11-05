import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Namespaced TypeScript entry points for Monte Carlo calculations.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const est = models.monteCarlo.priceHeston(
 *   100, 100, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, 1.0, true, 5000, 42n,
 * );
 * console.log(est.mean.amount, est.mean.currency);
 * // Path count and seed default to the Rust registry values.
 * const byDefault = models.monteCarlo.priceHeston(
 *   100, 100, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, 1.0, false,
 * );
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
