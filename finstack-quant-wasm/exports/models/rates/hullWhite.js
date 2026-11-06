import * as wasm from '../../../pkg/finstack_quant_wasm.js';

/**
 * Hull-White one-factor short-rate kernels: bond volatility, zero-coupon bond
 * options, caplet normal volatility, caps and floors, and the futures
 * convexity adjustment.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const bondVol = models.rates.hullWhite.hwBondVol(0.05, 0.01, 0, 1, 5);
 * console.log(models.rates.hullWhite.hw1fZcbOptionPrice(0.95, 0.8, 0.84, bondVol, true));
 * ```
 */
export const hullWhite = {
  HullWhiteParams: wasm.HullWhiteParams,
  hw1fConvexityAdjustment: wasm.hw1fConvexityAdjustment,
  hwBondVol: wasm.hwBondVol,
  hw1fZcbOptionPrice: wasm.hw1fZcbOptionPrice,
  hw1fCapletForwardRateNormalVol: wasm.hw1fCapletForwardRateNormalVol,
  hw1fCapFloorPrice: wasm.hw1fCapFloorPrice,
};
