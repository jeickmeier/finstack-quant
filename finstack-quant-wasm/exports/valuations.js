import * as wasm from '../pkg/finstack_quant_wasm.js';
import { composite } from './valuations/composite.js';
import { instruments } from './valuations/instruments.js';
import { market } from './valuations/market.js';
import { schema } from './valuations/schema.js';

/**
 * Namespaced TypeScript entry points for valuations calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * console.log(valuations.instruments.listStandardMetrics());
 * ```
 */
export const valuations = {
  composite,
  instruments,
  market,
  schema,
  valuationResultToJson: wasm.valuationResultToJson,
  valuationResultMetricSeries: wasm.valuationResultMetricSeries,
  valuationResultGetMetric: wasm.valuationResultGetMetric,
  valuationResultMetricUnits: wasm.valuationResultMetricUnits,
  valuationResultAllCovenantsPassed: wasm.valuationResultAllCovenantsPassed,
  valuationResultFailedCovenants: wasm.valuationResultFailedCovenants,
};
