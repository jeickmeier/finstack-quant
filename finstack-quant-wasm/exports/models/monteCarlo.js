import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const monteCarlo = {
  EuropeanPricer: wasm.EuropeanPricer,
  PathDependentPricer: wasm.PathDependentPricer,
  LsmcPricer: wasm.LsmcPricer,
  priceHeston: wasm.priceHeston,
  hestonSatisfiesFeller: wasm.hestonSatisfiesFeller,
  simulatePaths: wasm.simulatePaths,
  finiteDiffDelta: wasm.finiteDiffDelta,
  finiteDiffGamma: wasm.finiteDiffGamma,
  relativeStderr: wasm.relativeStderr,
};
