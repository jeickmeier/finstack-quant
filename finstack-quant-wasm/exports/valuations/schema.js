import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const schema = {
  index: wasm.valuationsSchemaIndex,
  get: wasm.valuationsSchemaGet,
  validate: wasm.valuationsSchemaValidate,
  instrumentEnvelopeSchema: wasm.instrumentEnvelopeSchema,
  instrumentTypes: wasm.instrumentTypes,
  instrumentSchema: wasm.instrumentSchema,
  valuationResultSchema: wasm.valuationResultSchema,
  validateInstrumentEnvelopeJson: wasm.validateInstrumentEnvelopeJson,
  validateInstrumentTypeJson: wasm.validateInstrumentTypeJson,
};
