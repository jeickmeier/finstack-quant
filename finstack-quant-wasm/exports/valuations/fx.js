import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const fx = {
  FxSpot: wasm.FxSpot,
  FxForward: wasm.FxForward,
  FxSwap: wasm.FxSwap,
  Ndf: wasm.Ndf,
  FxOption: wasm.FxOption,
  FxDigitalOption: wasm.FxDigitalOption,
  FxTouchOption: wasm.FxTouchOption,
  FxBarrierOption: wasm.FxBarrierOption,
  FxVarianceSwap: wasm.FxVarianceSwap,
  QuantoOption: wasm.QuantoOption,
};
