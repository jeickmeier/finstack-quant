import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.statementsSchemaIndex,
  get: wasm.statementsSchemaGet,
  validate: wasm.statementsSchemaValidate,
  financialModelSpecSchema: wasm.financialModelSpecSchema,
  normalizationConfigSchema: wasm.normalizationConfigSchema,
  statementResultSchema: wasm.statementResultSchema,
};
