import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/covenant_template.json";
import metadata from "@/lib/finstack/generated/meta/covenant_template";
/**
 * Canonical generated CovenantTemplate union (one Rust covenant package and
 * its thresholds) and presentation metadata, sharing the wire codec. Rust
 * publishes no example, so the form starts with no package chosen.
 */
export const covenantTemplateModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
