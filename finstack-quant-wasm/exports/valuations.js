import * as wasm from '../pkg/finstack_quant_wasm.js';
import { composite } from './valuations/composite.js';
import { instruments } from './valuations/instruments.js';
import { market } from './valuations/market.js';
import { schema } from './valuations/schema.js';

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
