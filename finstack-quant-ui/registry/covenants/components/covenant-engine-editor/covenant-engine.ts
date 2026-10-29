import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/covenant_engine.json";
import metadata from "@/lib/finstack/generated/meta/covenant_engine";
/**
 * Canonical generated CovenantEngine structure and presentation metadata,
 * sharing the wire codec. Rust publishes no example, so the form starts from
 * the caller's engine or a loaded template.
 */
export const covenantEngineModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
