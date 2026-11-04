"use client";
import { useState } from "react";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import type { WorkoutLgdRequest } from "@/workers/finstack-contract";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import {
  Outcome,
  typedNumber,
  type OutcomeState,
} from "@/components/finstack/shared/primitives/native-display/native-display";
import {
  collateralHelp,
  collateralTypes,
  costHelp,
  workoutHelp,
} from "./recovery-contracts";
import {
  defaultValue,
  FieldTable,
  options,
  row,
  useRows,
  type Density,
} from "./recovery-fields";

export interface WorkoutLgdCardProps {
  /** The initial workout; remount to load another. */
  defaultTerms: WorkoutLgdRequest;
  /** Receives the workout on Calculate LGD; connect it to the useWorkoutLgd request. */
  onSubmit(terms: WorkoutLgdRequest): void;
  /** The useWorkoutLgd state. */
  state: OutcomeState<modelsWire.WorkoutLgdResult>;
  /** Display text of a finite returned number; omitted shows four decimals. Every digit stays in the cell's title. */
  formatValue?: (value: number) => string;
  density?: Density;
}
/**
 * The workout arguments, each with the WorkoutLgd field it fills and that
 * field's contract description. `ead` is an argument of workoutLgd with no
 * contract field, so its help is the binding's own.
 */
const costFields = [
  ["ead", "ead", "Exposure at default in monetary units; positive."],
  ["directCostPct", "direct_cost_rate", costHelp("direct_cost_rate")],
  ["indirectCostPct", "indirect_cost_rate", costHelp("indirect_cost_rate")],
  ["timeToResolutionYears", "workout_years", workoutHelp("workout_years")],
  ["discountRate", "discount_rate", workoutHelp("discount_rate")],
] as const;
/**
 * A workout LGD from collateral, costs and time, as Rust returned it.
 * Nothing is summed, divided or discounted here.
 */
export function WorkoutLgdCard({
  defaultTerms,
  onSubmit,
  state,
  formatValue = defaultValue,
  density = "compact",
}: WorkoutLgdCardProps) {
  const [costs, setCosts] = useState<Record<string, string>>(() => ({
    ead: String(defaultTerms.ead),
    directCostPct: String(defaultTerms.directCostPct),
    indirectCostPct: String(defaultTerms.indirectCostPct),
    timeToResolutionYears: String(defaultTerms.timeToResolutionYears),
    discountRate: String(defaultTerms.discountRate),
  }));
  const pieces = useRows(() =>
    defaultTerms.collateral.map(([type, bookValue, haircut]) =>
      row({ type, bookValue: String(bookValue), haircut: String(haircut) }),
    ),
  );
  return (
    <section
      aria-label="Workout LGD"
      data-density={density}
      className="@container space-y-4 font-sans text-sm text-foreground"
    >
      <h3 className="text-sm font-medium">Workout LGD</h3>
      <form
        aria-label="Workout terms"
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit({
            ead: typedNumber(costs.ead),
            collateral: pieces.rows.map((piece) => [
              piece.type as modelsWire.WorkoutCollateralType,
              typedNumber(piece.bookValue),
              typedNumber(piece.haircut),
            ]),
            directCostPct: typedNumber(costs.directCostPct),
            indirectCostPct: typedNumber(costs.indirectCostPct),
            timeToResolutionYears: typedNumber(costs.timeToResolutionYears),
            discountRate: typedNumber(costs.discountRate),
          });
        }}
      >
        <div className="grid gap-4 @lg:grid-cols-3">
          {costFields.map(([key, wire, help]) => (
            <DecimalInput
              key={key}
              label={labelFor(wire)}
              help={help}
              value={costs[key] ?? ""}
              onValueChange={(value) =>
                setCosts((prior) => ({ ...prior, [key]: value }))
              }
            />
          ))}
        </div>
        {pieces.rows.map((piece, index) => (
          <fieldset
            key={piece.key}
            aria-label={`Collateral ${index}`}
            className="grid gap-4 border-t border-border pt-4 @lg:grid-cols-4"
          >
            <EnumField
              label={labelFor("collateral_type")}
              help={collateralHelp("collateral_type")}
              layout="select"
              value={piece.type ?? ""}
              onValueChange={(value) => pieces.set(piece.key, "type", value)}
              options={options(collateralTypes)}
            />
            <DecimalInput
              label={labelFor("book_value")}
              help={collateralHelp("book_value")}
              value={piece.bookValue ?? ""}
              onValueChange={(value) =>
                pieces.set(piece.key, "bookValue", value)
              }
            />
            <DecimalInput
              label={labelFor("haircut")}
              help={collateralHelp("haircut")}
              value={piece.haircut ?? ""}
              onValueChange={(value) => pieces.set(piece.key, "haircut", value)}
            />
            <div className="flex items-end">
              <Button
                type="button"
                variant="outline"
                onClick={() => pieces.remove(piece.key)}
              >
                Remove collateral
              </Button>
            </div>
          </fieldset>
        ))}
        <div className="flex flex-wrap gap-3">
          <Button
            type="button"
            variant="outline"
            onClick={() => pieces.add({ type: "", bookValue: "", haircut: "" })}
          >
            Add collateral
          </Button>
          <Button type="submit">Calculate LGD</Button>
        </div>
      </form>
      <Outcome state={state} empty="No workout LGD" working="Calculating…">
        {(result) => (
          <div className="space-y-3">
            <FieldTable
              caption="Workout"
              rows={Object.entries(result)}
              format={formatValue}
              density={density}
            />
            <p className="text-xs text-muted-foreground">
              Net recovery in the unit entered, after costs and discounting; LGD
              and recovery rate are fractions of EAD.
            </p>
          </div>
        )}
      </Outcome>
    </section>
  );
}
