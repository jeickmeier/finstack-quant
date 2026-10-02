import * as wasm from '../../../pkg/finstack_quant_wasm.js';

export const hullWhite = {
  HullWhiteParams: wasm.HullWhiteParams,
  hw1fConvexityAdjustment: wasm.hw1fConvexityAdjustment,
  hwBondVol: wasm.hwBondVol,
  hw1fZcbOptionPrice: wasm.hw1fZcbOptionPrice,
  hw1fCapletForwardRateNormalVol: wasm.hw1fCapletForwardRateNormalVol,
  // wasm-bindgen cannot borrow an optional handle (`Option<&DiscountCurve>`),
  // so the Rust export takes both curves and this wrapper passes the discount
  // curve as the forward curve when `forwardCurve` is omitted, as Python does.
  hw1fCapFloorPrice: (kappa, sigma, periods, strike, isCap, discountCurve, forwardCurve) =>
    wasm.hw1fCapFloorPrice(
      kappa,
      sigma,
      periods,
      strike,
      isCap,
      discountCurve,
      forwardCurve ?? discountCurve
    ),
};
