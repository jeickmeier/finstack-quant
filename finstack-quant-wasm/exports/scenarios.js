import * as wasm from '../pkg/finstack_quant_wasm.js';
import { schema } from './scenarios/schema.js';

export const scenarios = {
  parseScenarioSpec: wasm.parseScenarioSpec,
  composeScenarios: wasm.composeScenarios,
  validateScenarioSpec: wasm.validateScenarioSpec,
  listBuiltinTemplates: wasm.listBuiltinTemplates,
  listBuiltinTemplateMetadata: wasm.listBuiltinTemplateMetadata,
  buildFromTemplate: wasm.buildFromTemplate,
  listTemplateComponents: wasm.listTemplateComponents,
  buildTemplateComponent: wasm.buildTemplateComponent,
  applyScenario: wasm.applyScenario,
  computeHorizonReturn: wasm.computeHorizonReturn,
  parallelBpMany: wasm.parallelBpMany,
  operationSpecTimeRollForward: wasm.operationSpecTimeRollForward,
  operationSpecValidate: wasm.operationSpecValidate,
  operationSpecRequiresInstruments: wasm.operationSpecRequiresInstruments,
  operationSpecMutatesInstruments: wasm.operationSpecMutatesInstruments,
  rateBindingSpecValidate: wasm.rateBindingSpecValidate,
  scenarioSpecRequiresInstruments: wasm.scenarioSpecRequiresInstruments,
  scenarioSpecMutatesInstruments: wasm.scenarioSpecMutatesInstruments,
  horizonResultExplainText: wasm.horizonResultExplainText,
  horizonResultFactorContribution: wasm.horizonResultFactorContribution,
  schema,
};
