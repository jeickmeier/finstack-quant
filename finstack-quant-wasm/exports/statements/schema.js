import { domainSchema } from '../domainSchema.js';

/**
 * Registry of the canonical statements JSON Schemas.
 * @example
 * ```typescript
 * import init, { statements } from "finstack-quant-wasm";
 * await init();
 * const document = statements.schema.get("financial_model_spec.schema.json");
 * console.log(document.title);
 * ```
 */
export const schema = domainSchema('statements');
