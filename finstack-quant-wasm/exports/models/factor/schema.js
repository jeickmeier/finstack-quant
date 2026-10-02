import * as wasm from '../../../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.factorSchemaIndex,
  get: wasm.factorSchemaGet,
  validate: wasm.factorSchemaValidate,
  creditCalibrationConfigSchema: wasm.creditCalibrationConfigSchema,
  creditCalibrationInputsSchema: wasm.creditCalibrationInputsSchema,
  creditFactorModelSchema: wasm.creditFactorModelSchema,
  factorModelConfigSchema: wasm.factorModelConfigSchema,
};
