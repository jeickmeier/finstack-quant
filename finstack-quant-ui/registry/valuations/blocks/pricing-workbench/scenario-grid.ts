import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/scenario_grid.json";
import metadata from "@/lib/finstack/generated/meta/scenario_grid";
/** Generated structured-credit ScenarioGrid contract: the CPR, CDR and severity axes trancheScenarioTable sweeps. */
export const scenarioGridModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: schema.examples[0],
};
