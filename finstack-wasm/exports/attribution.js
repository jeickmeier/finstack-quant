import * as wasm from '../pkg/finstack_wasm.js';

export const attribution = {
  AttributionParams: wasm.AttributionParams,
  attributePnl: wasm.attributePnl,
  attributePnlFromSpec: wasm.attributePnlFromSpec,
  validateAttributionJson: wasm.validateAttributionJson,
  defaultWaterfallOrder: wasm.defaultWaterfallOrder,
  defaultAttributionMetrics: wasm.defaultAttributionMetrics,
};
