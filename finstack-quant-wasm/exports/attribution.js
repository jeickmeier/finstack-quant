import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './attribution/schema.js';

/**
 * Namespaced TypeScript entry points for attribution calculations and types.
 * @example
 * ```typescript
 * import init, { attribution } from "finstack-quant-wasm";
 * await init();
 * console.log(attribution.defaultWaterfallOrder());
 * ```
 */
export const attribution = {
  attributePnl: wasm.attributePnl,
  attributePnlJson: wasm.attributePnlJson,
  attributePnlEnvelope: wasm.attributePnlEnvelope,
  attributePnlEnvelopeJson: wasm.attributePnlEnvelopeJson,
  validateAttributionJson: wasm.validateAttributionJson,
  defaultWaterfallOrder: wasm.defaultWaterfallOrder,
  defaultAttributionMetrics: wasm.defaultAttributionMetrics,
  pnlBridge: wasm.pnlBridge,
  attributePnlMany: wasm.attributePnlMany,
  attributeReturnContribution: wasm.attributeReturnContribution,
  attributeReturnContributionJson: wasm.attributeReturnContributionJson,
  validateReturnContributionJson: wasm.validateReturnContributionJson,
  pnlAttributionExplainText: wasm.pnlAttributionExplainText,
  pnlAttributionPctOfTotal: wasm.pnlAttributionPctOfTotal,
  pnlAttributionPctOfTotalByBucket: wasm.pnlAttributionPctOfTotalByBucket,
  pnlAttributionResidualWithinTolerance: wasm.pnlAttributionResidualWithinTolerance,
  pnlAttributionValidateCurrencies: wasm.pnlAttributionValidateCurrencies,
  pnlAttributionRequiredMetrics: wasm.pnlAttributionRequiredMetrics,
  schema,
};
