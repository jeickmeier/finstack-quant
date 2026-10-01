import * as wasm from '../pkg/finstack_quant_wasm.js';
import { composite } from './valuations/composite.js';
import { creditDerivatives } from './valuations/creditDerivatives.js';
import { fx } from './valuations/fx.js';
import { instruments } from './valuations/instruments.js';
import { market } from './valuations/market.js';
import { schema } from './valuations/schema.js';

export const valuations = {
  composite,
  creditDerivatives,
  fx,
  instruments,
  market,
  schema,
  validateValuationResultJson: wasm.validateValuationResultJson,
  valuationResultToJson: wasm.valuationResultToJson,
  valuationResultMetricSeries: wasm.valuationResultMetricSeries,
  tarnCouponProfile: wasm.tarnCouponProfile,
  snowballCouponProfile: wasm.snowballCouponProfile,
  inverseFloaterCouponProfile: wasm.inverseFloaterCouponProfile,
  cmsSpreadOptionIntrinsic: wasm.cmsSpreadOptionIntrinsic,
  callableRangeAccrualAccrued: wasm.callableRangeAccrualAccrued,
  valuationResultPriceDecimal: wasm.valuationResultPriceDecimal,
  valuationResultGetMetric: wasm.valuationResultGetMetric,
  valuationResultMetricKeys: wasm.valuationResultMetricKeys,
  valuationResultMetricCount: wasm.valuationResultMetricCount,
  valuationResultMetricUnits: wasm.valuationResultMetricUnits,
  valuationResultAllCovenantsPassed: wasm.valuationResultAllCovenantsPassed,
  valuationResultFailedCovenants: wasm.valuationResultFailedCovenants,
};
