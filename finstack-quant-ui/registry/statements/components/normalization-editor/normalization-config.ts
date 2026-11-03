import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/normalization_config.json";
import metadata from "@/lib/finstack/generated/meta/normalization_config";
/**
 * Canonical generated NormalizationConfig structure and presentation metadata.
 * Rust's example names nodes a given model need not have, so the form starts
 * from the caller's config instead.
 */
export const normalizationConfigModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
