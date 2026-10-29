import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/simm_sensitivities_json.json";
import metadata from "@/lib/finstack/generated/meta/simm_sensitivities_json";
/** Canonical generated SIMM sensitivity structure and presentation metadata, sharing the wire codec. */
export const simmModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: schema.examples[0],
};
