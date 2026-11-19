import schema from "@/lib/finstack/generated/schemas/sensitivity_result.json";
import type { StatementResult } from "finstack-quant-wasm";

type Node = {
  $ref?: string;
  properties?: Record<string, Node>;
  oneOf?: { const?: string }[];
};
/** Follows one local `$ref` of the generated SensitivityResult bundle. */
function resolve(node: Node): Node {
  if (!node.$ref) return node;
  const name = node.$ref.replace("#/$defs/", "");
  return (schema.$defs as Record<string, Node>)[name];
}
/** Rust SensitivityMode wire values, in Rust's declaration order, read from the generated contract. */
export const sensitivityModes: readonly string[] = (
  resolve(
    resolve((schema.properties as Record<string, Node>).config).properties!
      .mode,
  ).oneOf ?? []
).flatMap((branch) => (branch.const === undefined ? [] : [branch.const]));
/** The float view of a returned cell, or undefined when the cell has no value. */
export function evaluatedValue(
  result: StatementResult,
  nodeId: string,
  periodId: string,
): number | undefined {
  const value = result.cells[nodeId]?.[periodId]?.value;
  if (value === undefined || value === null) return undefined;
  if (typeof value === "number") return value;
  return Number(value.amount);
}
/** A returned cell as a number; a cell without a value is NaN, which Rust rejects. */
export function evaluatedNumber(
  result: StatementResult,
  nodeId: string,
  periodId: string,
): number {
  return evaluatedValue(result, nodeId, periodId) ?? Number.NaN;
}
