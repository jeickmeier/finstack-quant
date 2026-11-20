"use client";
import { useState } from "react";
import type { GoalSeekResult } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { GoalSeekRequest } from "@/workers/finstack-contract";
import { formatExact, groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { typedNumber } from "@/components/finstack/shared/primitives/native-display/native-display";

/** The solve as submitted, without the model and update flag the caller supplies. */
export type GoalSeekSubmission = Omit<
  GoalSeekRequest,
  "modelJson" | "marketJson" | "updateModel"
>;
/** Initial form values; every omitted field starts empty. */
export type GoalSeekDefaults = Partial<GoalSeekSubmission>;
const text = (value: number | undefined) =>
  value === undefined ? "" : String(value);
const amount = (value: GoalSeekSubmission["targetValue"] | undefined) =>
  typeof value === "object" ? value.amount : text(value);
const display = (value: GoalSeekSubmission["targetValue"]) =>
  typeof value === "number"
    ? formatExact(value)
    : `${groupDecimal(value.amount)} ${value.currency}`;
export interface GoalSeekFormProps {
  /** Model node ids offered for the target and driver nodes. */
  nodeIds: readonly string[];
  /** Model period ids, in model order. */
  periods: readonly string[];
  /** Initial values; remount to load others. Omitted starts empty. */
  defaultValues?: GoalSeekDefaults;
  /** Receives the solve on Solve; Rust validates every value. */
  onSubmit(submission: GoalSeekSubmission): void;
  /** Native GoalSeekResult of the submitted solve; null while none is available. */
  result: GoalSeekResult | null | undefined;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  /**
   * Writes the returned model (the input model with the solved driver value)
   * back; returns an error message or null. Omitted hides Apply.
   */
  onApply?: (
    model: NonNullable<GoalSeekResult["model"]>,
  ) => Promise<string | null>;
  /** Why Apply is unavailable for the result on screen, such as a model changed since the solve. */
  applyBlocked?: string;
  disabled?: boolean;
  density?: "compact" | "comfortable";
}
/**
 * Edits the arguments of statements_analytics.goalSeek: target node, period
 * and value, driver node and period, dated observation cut, and explicit tolerances and search bracket. Solve is
 * explicit and passes the arguments to the caller; the native GoalSeekResult
 * is shown as returned. Apply is a second, explicit action that hands the
 * returned model to the caller (for example the workbench's commit); there
 * is no undo. Nothing is calculated or defaulted here.
 */
export function GoalSeekForm({
  nodeIds,
  periods,
  defaultValues,
  onSubmit,
  result,
  loading,
  error,
  onApply,
  applyBlocked,
  disabled,
  density = "compact",
}: GoalSeekFormProps) {
  const [targetNode, setTargetNode] = useState(defaultValues?.targetNode ?? "");
  const [targetPeriod, setTargetPeriod] = useState(
    defaultValues?.targetPeriod ?? "",
  );
  const [targetValue, setTargetValue] = useState(
    amount(defaultValues?.targetValue),
  );
  const [targetTolerance, setTargetTolerance] = useState(
    amount(defaultValues?.targetTolerance),
  );
  const [currency, setCurrency] = useState(
    typeof defaultValues?.targetValue === "object"
      ? defaultValues.targetValue.currency
      : "",
  );
  const [asOf, setAsOf] = useState(defaultValues?.asOf ?? "");
  const [observedThrough, setObservedThrough] = useState(
    defaultValues?.observedThrough ?? "",
  );
  const [driverTolerance, setDriverTolerance] = useState(
    text(defaultValues?.driverTolerance),
  );
  const [maxIterations, setMaxIterations] = useState(
    text(defaultValues?.maxIterations),
  );
  const [driverNode, setDriverNode] = useState(defaultValues?.driverNode ?? "");
  const [driverPeriod, setDriverPeriod] = useState(
    defaultValues?.driverPeriod ?? "",
  );
  const [lower, setLower] = useState(text(defaultValues?.bounds?.[0]));
  const [upper, setUpper] = useState(text(defaultValues?.bounds?.[1]));
  const [applied, setApplied] = useState<{
    pending: boolean;
    message: string | null;
  } | null>(null);
  const periodOptions = periods.map((id) => ({ value: id, label: id }));
  const submit = () => {
    setApplied(null);
    onSubmit({
      targetNode,
      targetPeriod,
      targetValue: currency
        ? { amount: targetValue, currency }
        : typedNumber(targetValue),
      targetTolerance: currency
        ? { amount: targetTolerance, currency }
        : typedNumber(targetTolerance),
      driverNode,
      driverPeriod,
      driverTolerance: typedNumber(driverTolerance),
      maxIterations: typedNumber(maxIterations),
      asOf,
      observedThrough,
      bounds: [typedNumber(lower), typedNumber(upper)],
    });
  };
  const apply = async () => {
    if (!onApply || !result?.model) return;
    setApplied({ pending: true, message: null });
    const message = await onApply(result.model);
    setApplied({ pending: false, message });
  };
  return (
    <section
      aria-label="Goal seek"
      data-density={density}
      className="@container min-w-0 space-y-4 font-sans text-sm text-foreground"
    >
      <fieldset className="grid gap-4 rounded-md border border-border p-3 @lg:grid-cols-3">
        <legend className="px-1 text-sm font-medium">Target</legend>
        <IdCombobox
          label="Target node"
          value={targetNode}
          onValueChange={setTargetNode}
          options={nodeIds}
        />
        <EnumField
          label="Target period"
          layout="select"
          value={targetPeriod}
          onValueChange={setTargetPeriod}
          options={periodOptions}
        />
        <DecimalInput
          label="Target value"
          help="Value the target node must reach, in its own units."
          value={targetValue}
          onValueChange={setTargetValue}
        />
        <DecimalInput
          label="Target tolerance"
          help="Absolute permitted difference from the target, in its stated units."
          value={targetTolerance}
          onValueChange={setTargetTolerance}
        />
        <label className="grid gap-2">
          Target currency
          <Input
            value={currency}
            onChange={(event) => setCurrency(event.target.value)}
            placeholder="Leave empty for a scalar"
          />
        </label>
      </fieldset>
      <fieldset className="grid gap-4 rounded-md border border-border p-3 @lg:grid-cols-3">
        <legend className="px-1 text-sm font-medium">Driver</legend>
        <IdCombobox
          label="Driver node"
          value={driverNode}
          onValueChange={setDriverNode}
          options={nodeIds}
        />
        <EnumField
          label="Driver period"
          layout="select"
          value={driverPeriod}
          onValueChange={setDriverPeriod}
          options={periodOptions}
        />
        <div className="grid grid-cols-2 gap-3">
          <DecimalInput
            label="Lower bound"
            help="Search bracket in the driver node's units."
            value={lower}
            onValueChange={setLower}
          />
          <DecimalInput
            label="Upper bound"
            help="Search bracket in the driver node's units."
            value={upper}
            onValueChange={setUpper}
          />
        </div>
        <DecimalInput
          label="Driver tolerance"
          help="Positive root tolerance in the driver's units."
          value={driverTolerance}
          onValueChange={setDriverTolerance}
        />
        <DecimalInput
          label="Maximum iterations"
          help="Positive solver iteration budget."
          value={maxIterations}
          onValueChange={setMaxIterations}
        />
      </fieldset>
      <fieldset className="grid gap-4 rounded-md border border-border p-3 @lg:grid-cols-2">
        <legend className="px-1 text-sm font-medium">Information cut</legend>
        <label className="grid gap-2">
          Valuation date
          <Input
            type="date"
            value={asOf}
            onChange={(event) => setAsOf(event.target.value)}
          />
        </label>
        <label className="grid gap-2">
          Observed through
          <Input
            type="date"
            value={observedThrough}
            onChange={(event) => setObservedThrough(event.target.value)}
          />
        </label>
      </fieldset>
      <Button type="button" onClick={submit} disabled={disabled}>
        Solve
      </Button>
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !result ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Solving…" : "No goal-seek solution available."}
        </p>
      ) : (
        <div className="space-y-3">
          <dl
            aria-label="Returned goal-seek solution"
            className="grid max-w-xl grid-cols-[minmax(0,1fr)_auto] gap-x-6 gap-y-1"
          >
            {(
              [
                ["solved_value", display(result.solved_value)],
                ["achieved_value", display(result.achieved_value)],
                ["residual", display(result.residual)],
                ["target_tolerance", display(result.target_tolerance)],
                ["evaluations", String(result.evaluations)],
              ] as const
            ).map(([name, value]) => (
              <div key={name} className="contents">
                <dt className="text-muted-foreground">{labelFor(name)}</dt>
                <dd className="finstack-numeric text-right">{value}</dd>
              </div>
            ))}
          </dl>
          {onApply && (
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground">
                Apply writes the solved value into the driver node at the driver
                period of the model and re-evaluates it. There is no undo.
              </p>
              <Button
                type="button"
                variant="outline"
                disabled={
                  !result.model ||
                  applyBlocked !== undefined ||
                  applied?.pending === true
                }
                onClick={() => void apply()}
              >
                Apply to model
              </Button>
              {!result.model && (
                <p className="text-xs text-muted-foreground">
                  This result carries no model (solved without updating it).
                </p>
              )}
              {applied && !applied.pending ? (
                applied.message ? (
                  <p role="alert" className="text-sm text-error">
                    {applied.message}
                  </p>
                ) : (
                  <p role="status" className="text-sm">
                    Applied to the model.
                  </p>
                )
              ) : (
                applyBlocked && (
                  <p className="text-xs text-muted-foreground">
                    {applyBlocked}
                  </p>
                )
              )}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
