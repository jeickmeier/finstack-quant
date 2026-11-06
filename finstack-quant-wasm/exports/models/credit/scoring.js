import * as wasm from '../../../pkg/finstack_quant_wasm.js';

/**
 * Accounting-based credit scores: Altman Z / Z' / Z'' / EM, Ohlson O-Score and Zmijewski.
 * Mirrors Rust `credit::scoring`: each scoring function takes its canonical input struct.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const input = {
 *   net_income_to_total_assets: 0.05,
 *   total_liabilities_to_total_assets: 0.6,
 *   current_assets_to_current_liabilities: 1.5,
 * };
 * console.log(models.credit.scoring.zmijewskiScore(input).implied_pd);
 * console.log(models.credit.scoring.creditScoreReport({ model: "zmijewski_score", input }).grade?.grade);
 * ```
 */
export const scoring = {
  altmanZScore: wasm.altmanZScore,
  altmanZPrime: wasm.altmanZPrime,
  altmanZDoublePrime: wasm.altmanZDoublePrime,
  altmanEmScore: wasm.altmanEmScore,
  creditScoreReport: wasm.creditScoreReport,
  ohlsonOScore: wasm.ohlsonOScore,
  zmijewskiScore: wasm.zmijewskiScore,
};
