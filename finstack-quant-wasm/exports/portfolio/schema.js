import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.portfolioSchemaIndex,
  get: wasm.portfolioSchemaGet,
  validate: wasm.portfolioSchemaValidate,
};
