import * as wasm from '../pkg/finstack_quant_wasm.js';
import { correlation } from './valuations/correlation.js';
import { credit } from './valuations/credit.js';
import { creditDerivatives } from './valuations/creditDerivatives.js';
import { fx } from './valuations/fx.js';
import { instruments } from './valuations/instruments.js';

export const valuations = {
  correlation,
  credit,
  creditDerivatives,
  fx,
  instruments,
  validateValuationResultJson: wasm.validateValuationResultJson,
  // Calibration: build a MarketContext from raw quotes.
  // ⚠️ BLOCKING: calibration can be CPU-heavy; callers must run it behind an
  // application-level timeout until the envelope schema carries timeout_ms.
  calibrate(envelope) {
    const json = typeof envelope === 'string' ? envelope : JSON.stringify(envelope);
    return JSON.parse(wasm.calibrate(json));
  },
  validateCalibrationJson(envelope) {
    const json = typeof envelope === 'string' ? envelope : JSON.stringify(envelope);
    return wasm.validateCalibrationJson(json);
  },
  dryRun(envelope) {
    const json = typeof envelope === 'string' ? envelope : JSON.stringify(envelope);
    return wasm.dryRun(json);
  },
  dependencyGraphJson(envelope) {
    const json = typeof envelope === 'string' ? envelope : JSON.stringify(envelope);
    return wasm.dependencyGraphJson(json);
  },
  Market: wasm.Market,
  bsPrice: wasm.bsPrice,
  bsGreeks: wasm.bsGreeks,
  bsImpliedVol: wasm.bsImpliedVol,
  black76ImpliedVol: wasm.black76ImpliedVol,
  barrierCall: wasm.barrierCall,
  asianOptionPrice: wasm.asianOptionPrice,
  lookbackOptionPrice: wasm.lookbackOptionPrice,
  quantoOptionPrice: wasm.quantoOptionPrice,
  SabrParameters: wasm.SabrParameters,
  SabrModel: wasm.SabrModel,
  SabrSmile: wasm.SabrSmile,
  SabrCalibrator: wasm.SabrCalibrator,
  bsCosPrice: wasm.bsCosPrice,
  vgCosPrice: wasm.vgCosPrice,
  mertonJumpCosPrice: wasm.mertonJumpCosPrice,
  tarnCouponProfile: wasm.tarnCouponProfile,
  snowballCouponProfile: wasm.snowballCouponProfile,
  inverseFloaterCouponProfile: wasm.inverseFloaterCouponProfile,
  cmsSpreadOptionIntrinsic: wasm.cmsSpreadOptionIntrinsic,
  callableRangeAccrualAccrued: wasm.callableRangeAccrualAccrued,
};
