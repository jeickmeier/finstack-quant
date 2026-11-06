import * as wasm from '../../pkg/finstack_quant_wasm.js';

/**
 * Compiled-in JSON Schemas of the valuations wire format.
 *
 * Each accessor returns the JSON Schema document as a plain object that always matches this build;
 * `index` / `get` / `validate` cover every schema the valuations crate publishes.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const types = valuations.schema.instrumentTypes();
 * console.log(types.includes("bond"));
 * ```
 */
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
