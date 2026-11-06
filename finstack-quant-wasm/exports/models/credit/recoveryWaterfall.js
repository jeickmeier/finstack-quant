import * as wasm from '../../../pkg/finstack_quant_wasm.js';

/**
 * Absolute-priority recovery waterfall over a bankruptcy estate.
 * Mirrors Rust `credit::recovery_waterfall`.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const result = models.credit.recoveryWaterfall.allocateRecovery(80, [
 *   {
 *     id: "TL-B",
 *     seniority: "senior_secured",
 *     priority: 1,
 *     principal: 100,
 *     accrued: 0,
 *     penalties: 0,
 *     collateral_value: null,
 *     collateral_haircut: 0,
 *   },
 * ]);
 * console.log(result.total_distributed); // 80
 * ```
 */
export const recoveryWaterfall = {
  allocateRecovery: wasm.allocateRecovery,
  recoveryClaimTotalClaim: wasm.recoveryClaimTotalClaim,
};
