import type { core } from "zod";

/** Canonical id of the shared definition document for wide instrument roots. */
export const sharedDefsId: string;

/**
 * Copy shared definitions into a root and rewrite its external shared references to local pointers.
 * @param schema - Bundled root whose shared references use `sharedDefsId`.
 * @param shared - Bundled shared document. Its `$defs` are copied.
 * @returns A closed schema whose shared references are `#/$defs/<name>`.
 * @throws Error when a shared reference names a missing definition, or when both documents define the same key with different bodies.
 */
export function linkSchema(
  schema: Record<string, unknown>,
  shared: { $defs?: Record<string, unknown> },
): Record<string, unknown>;

/**
 * Look up a local JSON Pointer against a schema root.
 * `"#"` returns the root itself; `"#/..."` segments decode `~1` before `~0`
 * and return the raw target with no sibling merge and no URI fetching.
 * @param root - Document root the pointer is evaluated against.
 * @param reference - Local reference such as `"#"` or `"#/$defs/name"`.
 * @returns The referenced node, or `undefined` for missing or inherited members.
 * @throws Error when `reference` is not a local `"#"`/`"#/"` pointer.
 */
export function schemaAt(root: unknown, reference: string): unknown;

/** Complete signed-integer lexical form: no prefix, whitespace, or exponent. */
export const integerText: RegExp;

/**
 * Convert finite numeric edit text while preserving invalid or incomplete input.
 * @param text - Raw edit text.
 * @param integer - When true, only a safe-integer lexical form converts;
 *   otherwise a finite JSON-number lexical form converts.
 * @returns The parsed number for valid complete input, or `text` unchanged.
 */
export function numericEdit(text: string, integer?: boolean): string | number;

/** Project canonical structural schemas for the pinned converter without applying defaults. */
export function converterSchema(
  schema: Record<string, unknown> | boolean,
  target?: "zod" | "typescript",
): core.JSONSchema.JSONSchema | boolean;

/** Map only JSON Schema child positions, preserving annotation data verbatim. */
export function mapChildren(
  schema: Record<string, unknown>,
  transform: (
    schema: Record<string, unknown> | boolean,
  ) => Record<string, unknown> | boolean,
): Record<string, unknown>;
