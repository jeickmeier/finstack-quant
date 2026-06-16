import * as wasm from '../pkg/finstack_quant_wasm.js';

export const covenants = {
  validateCovenantSpec: wasm.validateCovenantSpec,
  validateCovenantReport: wasm.validateCovenantReport,
  validateCovenantEngine: wasm.validateCovenantEngine,
  evaluateEngine: wasm.evaluateEngine,
  lboStandard: wasm.lboStandard,
  covLite: wasm.covLite,
  realEstate: wasm.realEstate,
  projectFinance: wasm.projectFinance,
};
