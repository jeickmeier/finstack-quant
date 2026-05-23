import * as wasm from '../pkg/finstack_wasm.js';

export const covenants = {
  validateCovenantSpec: wasm.validateCovenantSpec,
  validateCovenantReport: wasm.validateCovenantReport,
  validateCovenantEngine: wasm.validateCovenantEngine,
  evaluateEngine: wasm.evaluateCovenantEngine,
  lboStandard: wasm.lboStandardCovenants,
  covLite: wasm.covLiteCovenants,
  realEstate: wasm.realEstateCovenants,
  projectFinance: wasm.projectFinanceCovenants,
};
