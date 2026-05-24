import * as wasm from '../pkg/finstack_wasm.js';

export const covenants = {
  validateCovenantSpec: wasm.validateCovenantSpec,
  validateCovenantReport: wasm.validateCovenantReport,
  validateCovenantEngine: wasm.validateCovenantEngine,
  evaluateCovenantEngine: wasm.evaluateCovenantEngine,
  lboStandardCovenants: wasm.lboStandardCovenants,
  covLiteCovenants: wasm.covLiteCovenants,
  realEstateCovenants: wasm.realEstateCovenants,
  projectFinanceCovenants: wasm.projectFinanceCovenants,
};
