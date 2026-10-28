import type { ValuationResult } from "finstack-quant-wasm";
import { createWireCodec } from "../../../src/codec.mjs";
import schema from "../../../src/generated/schemas/valuation_result.json";
export const valuationCodec = createWireCodec(schema);
/**
 * Decode a recorded native result the way the facade transports it: schema
 * int64/uint64 fields become bigint, every other count stays a number.
 */
export function decode(entry: { resultJson: string }): ValuationResult {
  return valuationCodec.parse(entry.resultJson) as ValuationResult;
}
