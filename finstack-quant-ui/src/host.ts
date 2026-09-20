import type { ValuationDetails, ValuationResult } from "finstack-quant-wasm";
import { createWireCodec, serializeHost } from "./codec.mjs";
import schema from "./generated/schemas/valuation_result.json";
import hostSchema from "finstack-quant-wasm/contracts/valuation-result.schema.json";

export type {
  ValuationDetails,
  ValuationResult,
  MonteCarloValuationDetails,
} from "finstack-quant-wasm";
/** Cached adapter for the canonical result schema and its declared 64-bit host fields. */
export const valuationCodec = createWireCodec(schema);

const hostCodec = createWireCodec(hostSchema);

/**
 * Validate native host fields against the Rust-derived WASM contract.
 * @param result - Actual facade result or its structured clone.
 * @returns Validated plain host tree, retaining bigint in every declared wide-integer field.
 * @throws TypeError or ZodError for unsafe numbers or invalid published fields.
 */
export function adaptValuation(result: ValuationResult): ValuationResult {
  return hostCodec.fromHost(result) as ValuationResult;
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

/**
 * Choose presentation from the native detail discriminator.
 * @param details - Unmodified details from the facade result.
 * @returns Monte Carlo diagnostics or a lossless raw preview for other detail data.
 * @throws TypeError if raw data contains an unsupported non-JSON host object.
 */
export function getDetailsView(details: ValuationDetails) {
  return details.type === "monte_carlo"
    ? { kind: "monte_carlo" as const, value: details.data }
    : {
        kind: "raw" as const,
        type: details.type,
        value: details.data,
        text: serializeHost(details.data),
      };
}
