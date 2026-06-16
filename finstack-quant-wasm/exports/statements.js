import * as wasm from '../pkg/finstack_quant_wasm.js';

export const statements = {
  validateFinancialModelJson: wasm.validateFinancialModelJson,
  modelNodeIds: wasm.modelNodeIds,
  validateCheckSuiteSpec: wasm.validateCheckSuiteSpec,
  validateCapitalStructureSpec: wasm.validateCapitalStructureSpec,
  validateWaterfallSpec: wasm.validateWaterfallSpec,
  validateEcfSweepSpec: wasm.validateEcfSweepSpec,
  validatePikToggleSpec: wasm.validatePikToggleSpec,
  evaluateModel: wasm.evaluateModel,
  evaluateModelWithMarket: wasm.evaluateModelWithMarket,
  parseFormula: wasm.parseFormula,
  validateFormula: wasm.validateFormula,
};
