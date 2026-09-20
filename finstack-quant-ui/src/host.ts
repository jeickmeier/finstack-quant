import type { ValuationDetails, ValuationResult } from "finstack-quant-wasm";
import { createWireCodec, serializeHost } from "./codec.mjs";
import schema from "./generated/schemas/valuation_result.json";
import { mapChildren } from "./schema.mjs";

export type {
  ValuationDetails,
  ValuationResult,
  MonteCarloValuationDetails,
} from "finstack-quant-wasm";
/** Cached adapter for the canonical result schema and its declared 64-bit host fields. */
export const valuationCodec = createWireCodec(schema);

// The facade declares four detail payloads as unknown. Their live usize fields
// can be bigint even though typed Monte Carlo counts are number. Preserve these
// raw trees and let Rust validate their wire export; never invent host types.
function hostSchema(
  node: Record<string, unknown> | boolean,
): Record<string, unknown> | boolean {
  if (typeof node === "boolean") return node;
  const mapped = mapChildren(node, hostSchema);
  const properties = node.properties as Record<string, unknown> | undefined;
  const tag = (properties?.type as { const?: unknown } | undefined)?.const;
  if (
    typeof tag === "string" &&
    properties?.data &&
    [
      "structured_credit_stochastic",
      "credit_derivative",
      "fx",
      "composite",
    ].includes(tag)
  ) {
    (mapped.properties as Record<string, unknown>).data = true;
  }
  return mapped;
}
const hostCodec = createWireCodec(hostSchema(schema));

/**
 * Validate published host fields while preserving unknown detail payloads exactly.
 * @param result - Actual facade result or its structured clone.
 * @returns Validated plain host tree, retaining bigint in weakly typed details.
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
 * Choose presentation using the published host declaration, without inferring types.
 * @param details - Unmodified details from the facade result.
 * @returns Typed Monte Carlo data or a lossless raw preview for unknown detail data.
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
