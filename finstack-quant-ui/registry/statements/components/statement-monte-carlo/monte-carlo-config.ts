import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/monte_carlo_config.json";
import metadata from "@/lib/finstack/generated/meta/monte_carlo_config";
/**
 * Canonical generated MonteCarloConfig structure and presentation metadata.
 * The form starts from the caller's config (empty: path count and seed are
 * entered, never defaulted); Rust's published example is not preloaded.
 */
export const monteCarloConfigModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
