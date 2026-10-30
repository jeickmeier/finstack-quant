import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/merton_model.json";
import metadata from "@/lib/finstack/generated/meta/merton_model";
/** Canonical generated MertonModel structure and presentation metadata, sharing the wire codec. Rust publishes no example, so a form starts from the caller's values. */
export const mertonModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
