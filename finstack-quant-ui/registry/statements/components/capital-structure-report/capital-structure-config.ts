import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/capital_structure_summary_config.json";
import metadata from "@/lib/finstack/generated/meta/capital_structure_summary_config";
/**
 * Canonical generated CapitalStructureSummaryConfig structure and presentation
 * metadata. Rust publishes no example, so the form starts from the caller's
 * config.
 */
export const capitalStructureConfigModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
