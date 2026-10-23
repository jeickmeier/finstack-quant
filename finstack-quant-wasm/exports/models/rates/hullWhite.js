import * as wasm from '../../../pkg/finstack_quant_wasm.js';

export const hullWhite = {
  HullWhiteParams: wasm.HullWhiteParams,
  hw1fConvexityAdjustment: wasm.hw1fConvexityAdjustment,
  hwBondVol: wasm.hwBondVol,
  hw1fZcbOptionPrice: wasm.hw1fZcbOptionPrice,
  hw1fCapletForwardRateNormalVol: wasm.hw1fCapletForwardRateNormalVol,
  hw1fCapFloorPrice: wasm.hw1fCapFloorPrice,
};
