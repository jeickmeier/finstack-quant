import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Composite-instrument construction, decomposition, execution, and history.
 *
 * Pricing uses frozen quantities. Only `initialize` and `rebalance` calculate
 * a new state. Period return is `pnl / capital`; `return_index` starts at 100.
 * Close-effective rebalances report pre-trade P&L, then open the next interval
 * at the post-trade financed value. There is no `initializeFixed` export.
 */
export const composite = {
  /**
   * Resolve a bare specification into a canonical instrument envelope and trades.
   *
   * Fixed-quantity specs do not require `history`. Volatility weighting requires
   * strictly increasing observations that end on `asOf`.
   */
  initialize: wasm.initializeComposite,
  /**
   * Return a distinct resolved state and primitive deltas from an explicit rebalance.
   *
   * The supplied instrument is not mutated. Volatility history must end on `asOf`.
   */
  rebalance: wasm.rebalanceComposite,
  /**
   * Price frozen primitive paths and return net/gross value and additive risk.
   *
   * Non-additive metrics are rejected. Amounts use the reporting currency on `asOf`.
   */
  primitiveExposures: wasm.compositePrimitiveExposures,
  /**
   * Flatten target holdings or a state transition into primitive execution deltas.
   *
   * Omit `previous` to emit establishment trades for the current frozen state.
   */
  executionTrades: wasm.compositeExecutionTrades,
  /**
   * Initialize at the first snapshot and return dated P&L, return, index, exposure, and trade rows.
   *
   * Warmup feeds weighting only. The first row has `return_index = 100` and zero P&L.
   * Scheduled rebalances are close-effective.
   */
  historyFromSpec: wasm.compositeHistoryFromSpec,
  /**
   * Run dated history from an already-resolved immutable state.
   *
   * Period return is `pnl / capital`. The initial effective date must be on or
   * before the first observation.
   */
  history: wasm.compositeHistory,
};
