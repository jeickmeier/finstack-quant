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
 * The workout terms under their WorkoutLgd field names, each with its contract
 * description. `ead` is the evaluate argument with no contract field, so its
 * help is the binding's own.
 */
const termFields = [
  ["ead", "Exposure at default in monetary units; positive."],
  ["direct_cost_rate", costHelp("direct_cost_rate")],
  ["indirect_cost_rate", costHelp("indirect_cost_rate")],
  ["workout_years", workoutHelp("workout_years")],
  ["discount_rate", workoutHelp("discount_rate")],
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
  const [terms, setTerms] = useState<Record<string, string>>(() => {
    const { model } = defaultTerms;
    return {
      ead: String(defaultTerms.ead),
      direct_cost_rate: String(model.costs.direct_cost_rate),
      indirect_cost_rate: String(model.costs.indirect_cost_rate),
      workout_years: String(model.workout_years),
      discount_rate: String(model.discount_rate),
    };
  });
  const pieces = useRows(() =>
    defaultTerms.model.collateral.map((piece) =>
      row({
        type: piece.collateral_type,
        bookValue: String(piece.book_value),
        haircut: String(piece.haircut),
      }),
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
            ead: typedNumber(terms.ead),
            model: {
              collateral: pieces.rows.map((piece) => ({
                collateral_type: piece.type as modelsWire.WorkoutCollateralType,
                book_value: typedNumber(piece.bookValue),
                haircut: typedNumber(piece.haircut),
              })),
              costs: {
                direct_cost_rate: typedNumber(terms.direct_cost_rate),
                indirect_cost_rate: typedNumber(terms.indirect_cost_rate),
              },
              workout_years: typedNumber(terms.workout_years),
              discount_rate: typedNumber(terms.discount_rate),
            },
          });
        }}
      >
        <div className="grid gap-4 @lg:grid-cols-3">
          {termFields.map(([key, help]) => (
            <DecimalInput
              key={key}
              label={labelFor(key)}
              help={help}
              value={terms[key] ?? ""}
              onValueChange={(value) =>
                setTerms((prior) => ({ ...prior, [key]: value }))
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
