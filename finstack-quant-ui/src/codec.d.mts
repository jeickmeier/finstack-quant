import type { ZodType } from "zod";
/** Schema-directed JSON text transport; wide integers are bigint in UI state. */
export interface WireCodec {
  /** Structural validator returning UI state; throws ZodError on invalid or unsafe values. */
  validator: ZodType<unknown>;
  /** Parse JSON text without rounding integer tokens. Throws SyntaxError or ZodError on invalid input. */
  parse(text: string): unknown;
  /** Check a host tree, requiring bigint at schema int64/uint64 fields. Throws on shape/range mismatch. */
  fromHost(value: unknown): unknown;
  /** Emit numeric integer tokens, optionally validated/canonicalized by the supplied Rust export. */
  stringify(value: unknown, canonicalize?: (text: string) => string): string;
}
/** Build a cached codec from a generated, locally referenced schema bundle. */
export function createWireCodec(
  schema: Record<string, unknown> | boolean,
): WireCodec;
/** Serialize plain host data with numeric bigint tokens. Throws on unsafe numbers, cycles or unsupported objects. This is not Rust validation. */
export function serializeHost(value: unknown): string;
