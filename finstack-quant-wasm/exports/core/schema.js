import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.coreSchemaIndex,
  get: wasm.coreSchemaGet,
  validate: wasm.coreSchemaValidate,
};
