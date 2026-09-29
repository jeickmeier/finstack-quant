import * as wasm from '../pkg/finstack_quant_wasm.js';
import { composite } from './valuations/composite.js';
import { creditDerivatives } from './valuations/creditDerivatives.js';
import { fx } from './valuations/fx.js';
import { instruments } from './valuations/instruments.js';
import { market } from './valuations/market.js';

export const valuations = {
  composite,
  creditDerivatives,
  fx,
  instruments,
  market,
  validateValuationResultJson: wasm.validateValuationResultJson,
  valuationResultToJson: wasm.valuationResultToJson,
  Market: wasm.Market,
  tarnCouponProfile: wasm.tarnCouponProfile,
  snowballCouponProfile: wasm.snowballCouponProfile,
  inverseFloaterCouponProfile: wasm.inverseFloaterCouponProfile,
  cmsSpreadOptionIntrinsic: wasm.cmsSpreadOptionIntrinsic,
  callableRangeAccrualAccrued: wasm.callableRangeAccrualAccrued,
};
