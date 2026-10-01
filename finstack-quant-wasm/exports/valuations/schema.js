import * as wasm from '../../pkg/finstack_quant_wasm.js';

export const schema = {
  instrumentEnvelopeSchema: wasm.instrumentEnvelopeSchema,
  instrumentTypes: wasm.instrumentTypes,
  instrumentSchema: wasm.instrumentSchema,
  valuationResultSchema: wasm.valuationResultSchema,
};
