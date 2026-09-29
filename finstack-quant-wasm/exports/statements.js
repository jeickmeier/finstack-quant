import * as wasm from '../pkg/finstack_quant_wasm.js';

export const statements = {
  validateFinancialModelJson: wasm.validateFinancialModelJson,
  modelNodeIds: wasm.modelNodeIds,
  validateCheckSuiteSpecJson: wasm.validateCheckSuiteSpecJson,
  validateCapitalStructureSpecJson: wasm.validateCapitalStructureSpecJson,
  validateWaterfallSpecJson: wasm.validateWaterfallSpecJson,
  validateEcfSweepSpecJson: wasm.validateEcfSweepSpecJson,
  validatePikToggleSpecJson: wasm.validatePikToggleSpecJson,
  evaluateModel: wasm.evaluateModel,
  evaluateModelWithMarket: wasm.evaluateModelWithMarket,
  evaluateMonteCarlo: wasm.evaluateMonteCarlo,
  parseFormula: wasm.parseFormula,
  parseAndCompile: wasm.parseAndCompile,
};
