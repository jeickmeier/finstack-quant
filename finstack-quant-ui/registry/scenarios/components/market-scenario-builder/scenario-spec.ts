import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/scenario_spec.json";
import metadata from "@/lib/finstack/generated/meta/scenario_spec";
/** Canonical generated ScenarioSpec structure and presentation metadata, sharing the wire codec. */
export const scenarioSpecModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: schema.examples[0],
};
