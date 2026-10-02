import * as wasm from '../../../pkg/finstack_quant_wasm.js';

export const dtsm = {
  YieldPanel: wasm.YieldPanel,
  DieboldLi: wasm.DieboldLi,
  YieldPca: wasm.YieldPca,
  nelsonSiegelYields: wasm.nelsonSiegelYields,
  dieboldLiFitFactors: wasm.dieboldLiFitFactors,
  dieboldLiForecast: wasm.dieboldLiForecast,
  yieldPcaFit: wasm.yieldPcaFit,
  yieldPcaScenario: wasm.yieldPcaScenario,
};
