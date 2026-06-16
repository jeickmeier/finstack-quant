import * as wasm from '../pkg/finstack_quant_wasm.js';

export const scenarios = {
  parseScenarioSpec: wasm.parseScenarioSpec,
  composeScenarios: wasm.composeScenarios,
  validateScenarioSpec: wasm.validateScenarioSpec,
  listBuiltinTemplates: wasm.listBuiltinTemplates,
  listBuiltinTemplateMetadata: wasm.listBuiltinTemplateMetadata,
  buildFromTemplate: wasm.buildFromTemplate,
  listTemplateComponents: wasm.listTemplateComponents,
  buildTemplateComponent: wasm.buildTemplateComponent,
  buildScenarioSpec: wasm.buildScenarioSpec,
  applyScenario: wasm.applyScenario,
  applyScenarioToMarket: wasm.applyScenarioToMarket,
  computeHorizonReturn: wasm.computeHorizonReturn,
};
