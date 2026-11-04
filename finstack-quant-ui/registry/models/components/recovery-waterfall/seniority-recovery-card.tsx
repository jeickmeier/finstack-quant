"use client";
import { useState } from "react";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import type {
  SeniorityRecoveryRequest,
  SeniorityRecoveryStats,
} from "@/workers/finstack-contract";
import { Button } from "@/components/ui/button";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import {
  Outcome,
  type OutcomeState,
} from "@/components/finstack/shared/primitives/native-display/native-display";
import { seniorityClasses } from "./recovery-contracts";
import {
  defaultValue,
  FieldTable,
  options,
  type Density,
} from "./recovery-fields";

export interface SeniorityRecoveryCardProps {
  /** The initial seniority class; the calibration starts at the first supplied id. */
  defaultSeniority: modelsWire.SeniorityClass;
  /** Rust's calibration ids, default first (useSeniorityCalibrationIds). */
  calibrationIds?: readonly string[] | null;
  /** Receives the class and calibration on Show statistics; connect it to the useSeniorityRecoveryStats request. */
  onSubmit(request: SeniorityRecoveryRequest): void;
  /** The useSeniorityRecoveryStats state. */
  state: OutcomeState<SeniorityRecoveryStats>;
  /** Display text of a finite returned number; omitted shows four decimals. Every digit stays in the cell's title. */
  formatValue?: (value: number) => string;
  density?: Density;
}
/** The historical recovery distribution of a seniority class under one Rust calibration, as returned. */
export function SeniorityRecoveryCard({
  defaultSeniority,
  calibrationIds,
  onSubmit,
  state,
  formatValue = defaultValue,
  density = "compact",
}: SeniorityRecoveryCardProps) {
  const [seniority, setSeniority] = useState<string>(defaultSeniority);
  const [chosenCalibration, setChosenCalibration] = useState<string | null>(
    null,
  );
  // Rust lists its default calibration first.
  const calibrationId = chosenCalibration ?? calibrationIds?.[0] ?? "";
  return (
    <section
      aria-label="Seniority recovery"
      data-density={density}
      className="@container space-y-4 font-sans text-sm text-foreground"
    >
      <h3 className="text-sm font-medium">Recovery by seniority</h3>
      <form
        aria-label="Seniority and calibration"
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit({
            seniority: seniority as modelsWire.SeniorityClass,
            calibrationId,
          });
        }}
      >
        <div className="grid gap-4 @lg:grid-cols-2">
          <EnumField
            label="Seniority class"
            layout="select"
            value={seniority}
            onValueChange={setSeniority}
            options={options(seniorityClasses)}
          />
          <EnumField
            label="Calibration"
            layout="select"
            value={calibrationId}
            onValueChange={setChosenCalibration}
            options={options(
              (calibrationIds ?? []).map((value) => ({ value })),
            )}
          />
        </div>
        <Button type="submit">Show statistics</Button>
      </form>
      <Outcome state={state} empty="No recovery statistics" working="Loading…">
        {(result) => (
          <div className="space-y-3">
            <FieldTable
              caption="Beta recovery"
              rows={Object.entries(result)}
              format={formatValue}
              density={density}
            />
            <p className="text-xs text-muted-foreground">
              Historical Beta distribution of the recovery rate, as a fraction
              of par; a calibration without the class shows Rust&apos;s error.
            </p>
          </div>
        )}
      </Outcome>
    </section>
  );
}
