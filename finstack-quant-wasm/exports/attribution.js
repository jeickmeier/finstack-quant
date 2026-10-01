import * as wasm from '../pkg/finstack_quant_wasm.js';

export const attribution = {
  AttributionJsonInputs: wasm.AttributionJsonInputs,
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
  pnlAttributionExplainVerboseText: wasm.pnlAttributionExplainVerboseText,
  pnlAttributionResidualWithinTolerance: wasm.pnlAttributionResidualWithinTolerance,
  pnlAttributionValidateCurrencies: wasm.pnlAttributionValidateCurrencies,
  pnlAttributionRequiredMetrics: wasm.pnlAttributionRequiredMetrics,
};
