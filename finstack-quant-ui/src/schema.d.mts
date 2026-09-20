import type { core } from "zod";
/** Project canonical structural schemas for the pinned converter without applying defaults. */
export function converterSchema(
  schema: Record<string, unknown> | boolean,
  target?: "zod" | "typescript",
): core.JSONSchema.JSONSchema | boolean;
