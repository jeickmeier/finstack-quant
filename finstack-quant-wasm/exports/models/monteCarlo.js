import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const monteCarlo = {
  EuropeanPricer: wasm.EuropeanPricer,
  PathDependentPricer: wasm.PathDependentPricer,
  LsmcPricer: wasm.LsmcPricer,
  priceHestonCall: wasm.priceHestonCall,
  priceHestonPut: wasm.priceHestonPut,
  hestonSatisfiesFeller: wasm.hestonSatisfiesFeller,
  simulatePaths: wasm.simulatePaths,
  finiteDiffDelta: wasm.finiteDiffDelta,
  finiteDiffDeltaCrn: wasm.finiteDiffDeltaCrn,
  finiteDiffGamma: wasm.finiteDiffGamma,
  finiteDiffGammaCrn: wasm.finiteDiffGammaCrn,
  relativeStderr: wasm.relativeStderr,
};
