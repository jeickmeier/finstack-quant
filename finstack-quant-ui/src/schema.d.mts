import type { core } from "zod";
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
