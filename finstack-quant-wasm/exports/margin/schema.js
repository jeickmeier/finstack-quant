import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.marginSchemaIndex,
  get: wasm.marginSchemaGet,
  validate: wasm.marginSchemaValidate,
};
