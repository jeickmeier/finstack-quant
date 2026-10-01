import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.scenariosSchemaIndex,
  get: wasm.scenariosSchemaGet,
  validate: wasm.scenariosSchemaValidate,
};
