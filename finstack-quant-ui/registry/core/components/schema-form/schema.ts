import type { WireCodec } from "@/lib/finstack/codec.mjs";

/** Generated schema shape used for presentation, not a second financial validator. */
export interface Schema {
  $ref?: string;
  $defs?: Record<string, Schema>;
  type?: string | string[];
  title?: string;
  description?: string;
  format?: string;
  properties?: Record<string, Schema | undefined>;
  required?: string[];
  items?: Schema;
  prefixItems?: Schema[];
  additionalProperties?: boolean | Schema;
  anyOf?: Schema[];
  oneOf?: Schema[];
  allOf?: Schema[];
  enum?: unknown[];
  const?: unknown;
  default?: unknown;
  examples?: unknown[];
  pattern?: string;
}
export interface Metadata {
  path: string;
  source: string;
  title?: string;
  description?: string;
  ref?: string;
  resolvedRef?: string;
  unit?: string;
}
export interface InstrumentModule {
  schema: Schema;
  metadata: readonly Metadata[];
  codec: WireCodec;
  example: unknown;
}
export interface SchemaLocation {
  schema: Schema;
  pointer: string;
}
/** Defined canonical properties (JSON module inference may include optional union keys). */
export function propertiesOf(schema: Schema): [string, Schema][] {
  return Object.entries(schema.properties ?? {}).filter(
    (entry): entry is [string, Schema] => entry[1] !== undefined,
  );
}
