import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/relative_value_report_config.json";
import metadata from "@/lib/finstack/generated/meta/relative_value_report_config";
/**
 * Canonical generated RelativeValueReportConfig structure and presentation
 * metadata. Rust publishes no example, so the form starts from the caller's
 * config.
 */
export const relativeValueConfigModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
