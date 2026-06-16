import * as wasm from '../pkg/finstack_quant_wasm.js';

export const margin = {
  csaUsdRegulatory: wasm.csaUsdRegulatory,
  csaEurRegulatory: wasm.csaEurRegulatory,
  validateCsaJson: wasm.validateCsaJson,
  calculateVm: wasm.calculateVm,
};
