import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/csa_spec.json";
import metadata from "@/lib/finstack/generated/meta/csa_spec";
/** Canonical generated CSA specification structure and presentation metadata, sharing the wire codec. */
export const csaModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: schema.examples[0],
};
