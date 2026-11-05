import * as wasm from '../../../pkg/finstack_quant_wasm.js';

/**
 * Liability-management analytics: exchange offers and LME transactions.
 * Mirrors Rust `credit::liability_management`.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const hurdle = models.credit.liabilityManagement.tenderRecommendationHurdle();
 * console.log(hurdle); // 1.02
 * ```
 */
export const liabilityManagement = {
  analyzeExchangeOffer: wasm.analyzeExchangeOffer,
  analyzeLme: wasm.analyzeLme,
  tenderRecommendationHurdle: wasm.tenderRecommendationHurdle,
};
