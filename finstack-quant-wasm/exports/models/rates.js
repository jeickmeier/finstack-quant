import { dtsm } from './rates/dtsm.js';
import { hullWhite } from './rates/hullWhite.js';

/**
 * Product-independent interest-rate models.
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
export const rates = {
  dtsm,
  hullWhite,
};
