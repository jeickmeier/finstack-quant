import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/covenant_forecast_config.json";
import metadata from "@/lib/finstack/generated/meta/covenant_forecast_config";
/**
 * Canonical generated CovenantForecastConfig structure and presentation
 * metadata. Rust publishes no example; an empty config takes Rust's defaults.
 */
export const covenantForecastConfigModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
