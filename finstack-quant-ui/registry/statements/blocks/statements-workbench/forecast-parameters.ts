import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/forecast_parameters.json";
import metadata from "@/lib/finstack/generated/meta/forecast_parameters";

type Node = {
  properties?: Record<string, Node>;
  const?: unknown;
  enum?: unknown[];
  $ref?: string;
};
const methods = schema.oneOf as Node[];
const definitions = schema.$defs as unknown as Record<string, object>;
/**
 * The generated parameter contract of one forecast method: its branch of the
 * Rust ForecastParameters schema, ready for SchemaForm.
 * @param method - The `method` tag of the branch.
 * @returns A form module over that method's parameter object.
 * @throws TypeError when the schema has no branch for `method`.
 */
export function forecastParamsModule(method: string) {
  const branch = methods.find(({ properties }) => {
    const tag = properties?.method;
    return (tag?.const ?? tag?.enum?.[0]) === method;
  });
  const reference = branch?.properties?.params?.$ref;
  if (!reference) throw new TypeError(`No forecast method ${method}`);
  const params = {
    ...definitions[reference.split("/").at(-1)!],
    $defs: definitions,
  };
  return {
    schema: params,
    metadata,
    codec: createWireCodec(params),
    example: undefined,
  };
}
