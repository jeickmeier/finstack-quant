import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/lbo_config.json";
import metadata from "@/lib/finstack/generated/meta/lbo_config";
/** Canonical generated LboConfig structure and presentation metadata. Rust publishes no example, so the form starts from the caller's config. */
export const lboConfigModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
