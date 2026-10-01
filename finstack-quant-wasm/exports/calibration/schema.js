import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.calibrationSchemaIndex,
  get: wasm.calibrationSchemaGet,
  validate: wasm.calibrationSchemaValidate,
};
