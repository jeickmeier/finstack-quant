import workoutLgd from "@/lib/finstack/generated/schemas/workout_lgd.json";
import seniorityCalibration from "@/lib/finstack/generated/schemas/seniority_calibration.json";

type Node = {
  $ref?: string;
  items?: Node;
  prefixItems?: Node[];
  properties?: Record<string, Node>;
  oneOf?: { const?: string; description?: string }[];
};
/** Follows one local `$ref` of a generated bundle. */
function resolve(schema: { $defs?: object }, node: Node): Node {
  if (!node.$ref) return node;
  const name = node.$ref.replace("#/$defs/", "");
  return (schema.$defs as Record<string, Node>)[name];
}
/** The unit variants of a Rust enum contract, in Rust's declaration order. */
const variants = (node: Node) =>
  (node.oneOf ?? []).flatMap((branch) =>
    branch.const === undefined
      ? []
      : [{ value: branch.const, description: branch.description }],
  );
const collateralPiece = resolve(
  workoutLgd,
  (workoutLgd.properties as Record<string, Node>).collateral.items!,
);
/** Rust WorkoutCollateralType wire values, read from the generated WorkoutLgd contract. */
export const collateralTypes = variants(
  resolve(workoutLgd, collateralPiece.properties!.collateral_type),
);
/** Rust SeniorityClass wire values, read from the generated SeniorityCalibration contract. */
export const seniorityClasses = variants(
  resolve(
    seniorityCalibration,
    (seniorityCalibration.properties as Record<string, Node>).classes.items!
      .prefixItems![0],
  ),
);
