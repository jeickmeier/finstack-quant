import * as wasm from '../../../pkg/finstack_quant_wasm.js';

/**
 * Dynamic Nelson-Siegel and statistical yield-curve models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const yields = models.rates.dtsm.nelsonSiegelYields(
 *   0.7308, [0.03, -0.01, 0.005], [1, 5, 10]
 * );
 * ```
 */
export const dtsm = {
  YieldPanel: wasm.YieldPanel,
  DieboldLi: wasm.DieboldLi,
  YieldPca: wasm.YieldPca,
  nelsonSiegelYields: wasm.nelsonSiegelYields,
};
