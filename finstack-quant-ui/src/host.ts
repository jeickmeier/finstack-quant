import type { ValuationResult } from "finstack-quant-wasm";
import { createWireCodec, serializeHost } from "./codec.mjs";
import hostSchema from "finstack-quant-wasm/contracts/valuation-result.schema.json";

let hostCodec: ReturnType<typeof createWireCodec> | undefined;

/**
 * Validate native host fields against the Rust-derived WASM contract.
 * @param result - Actual facade result or its structured clone.
 * @returns Validated plain host tree, retaining bigint in every declared wide-integer field.
 * @throws TypeError or ZodError for unsafe numbers or invalid published fields.
 */
export function adaptValuation(result: ValuationResult): ValuationResult {
  return (hostCodec ??= createWireCodec(hostSchema)).fromHost(
    result,
  ) as ValuationResult;
}

/**
 * Export an actual facade result as Rust-validated JSON with integer seed tokens.
 * @param result - Structured facade result; uint64 fields must be bigint.
 * @param canonicalize - Existing valuations.validateValuationResultJson export.
 * @returns Canonical JSON returned by Rust.
 * @throws TypeError or ZodError for unsafe/invalid host values; propagates Rust errors.
 */
export function exportValuation(
  result: ValuationResult,
  canonicalize: (text: string) => string,
): string {
  return canonicalize(serializeHost(adaptValuation(result)));
}
