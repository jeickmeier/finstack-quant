import workoutLgd from "@/lib/finstack/generated/schemas/workout_lgd.json";
import seniorityCalibration from "@/lib/finstack/generated/schemas/seniority_calibration.json";
import recoveryClaim from "@/lib/finstack/generated/schemas/recovery_claim.json";

type Node = {
  $ref?: string;
  description?: string;
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
/** A property's Rust doc comment, read from a generated contract; undefined when it has none. */
const descriptions =
  (properties: Record<string, Node>) =>
  (field: string): string | undefined =>
    properties[field]?.description;
/** Rust RecoveryClaim field descriptions, from the generated RecoveryClaim contract. */
export const claimHelp = descriptions(
  recoveryClaim.properties as Record<string, Node>,
);
/** Rust WorkoutLgd field descriptions (`workout_years`, `discount_rate`). */
export const workoutHelp = descriptions(
  workoutLgd.properties as Record<string, Node>,
);
/** Rust WorkoutCosts field descriptions (`direct_cost_rate`, `indirect_cost_rate`). */
export const costHelp = descriptions(
  resolve(workoutLgd, (workoutLgd.properties as Record<string, Node>).costs)
    .properties!,
);
/** Rust CollateralPiece field descriptions (`collateral_type`, `book_value`, `haircut`). */
export const collateralHelp = descriptions(collateralPiece.properties!);
