import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './analytics/schema.js';

/**
 * Namespaced TypeScript entry points for analytics calculations and types.
 * @example
 * ```typescript
 * import init, { analytics } from "finstack-quant-wasm";
 * await init();
 * const factorReturns = analytics.constrainedLeastSquares(
 *   [1, 2],
 *   1,
 *   [0.01, 0.02],
 *   [0.5, 0.5]
 * );
 * console.log(factorReturns[0]);
 * ```
 */
export const analytics = {
  Performance: wasm.Performance,
  constrainedLeastSquares: wasm.constrainedLeastSquares,
  multiFactorGreeks: wasm.multiFactorGreeks,
  validateSummary: wasm.validateSummary,
  maxDrawdown: wasm.maxDrawdown,
  schema,
  sharpe: wasm.sharpe,
  sortino: wasm.sortino,
  volatility: wasm.volatility,
};
