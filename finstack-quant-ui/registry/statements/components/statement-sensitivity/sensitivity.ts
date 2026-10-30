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
/**
 * The evaluated number at a node and period, as Rust returned it. A missing
 * cell or a non-finite sentinel becomes NaN, which Rust rejects as a base
 * value; nothing is defaulted.
 */
export function evaluatedNumber(
  result: StatementResult,
  nodeId: string,
  periodId: string,
): number {
  const value = result.nodes[nodeId]?.[periodId];
  return typeof value === "number" ? value : Number.NaN;
}
