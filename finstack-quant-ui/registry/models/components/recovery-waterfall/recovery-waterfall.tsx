"use client";
import { useMemo, useState, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { collateralTypes, seniorityClasses } from "./recovery-contracts";

/** The estate and claims the waterfall form submits; the worker's RecoveryWaterfallRequest. */
export interface WaterfallTerms {
  estateValue: number;
  claims: modelsWire.RecoveryClaim[];
}
/** The workout the LGD form submits; the worker's WorkoutLgdRequest. */
export interface WorkoutTerms {
  ead: number;
  collateral: [
    collateralType: modelsWire.WorkoutCollateralType,
    bookValue: number,
    haircut: number,
  ][];
  directCostPct: number;
  indirectCostPct: number;
  timeToResolutionYears: number;
  discountRate: number;
}
/** The class and calibration the statistics form submits; the worker's SeniorityRecoveryRequest. */
export interface SeniorityTerms {
  seniority: modelsWire.SeniorityClass;
  calibrationId: string;
}
/** The BetaRecovery getters under their Rust names; the worker's SeniorityRecoveryStats. */
export interface SeniorityStats {
  mean: number;
  std_dev: number;
  variance: number;
  alpha: number;
  beta_param: number;
  mode: number | undefined;
  mean_lgd: number;
}
/** One analysis's state, from its hook. */
interface AnalysisState<T> {
  result: T | null;
  loading?: boolean;
  /** The native error of this analysis, shown as returned. */
  error?: string;
}
export interface RecoveryWaterfallProps {
  /** The initial estate and claims; remount to load others. */
  defaultWaterfall: WaterfallTerms;
  /** The initial workout; remount to load another. */
  defaultWorkout: WorkoutTerms;
  /** The initial seniority class; the calibration starts at the first supplied id. */
  defaultSeniority: modelsWire.SeniorityClass;
  /** Receives the estate and claims on Allocate; connect it to the useAllocateRecovery request. */
  onWaterfallSubmit(terms: WaterfallTerms): void;
  /** Receives the workout on Calculate LGD; connect it to the useWorkoutLgd request. */
  onWorkoutSubmit(terms: WorkoutTerms): void;
  /** Receives the class and calibration on Show statistics; connect it to the useSeniorityRecoveryStats request. */
  onStatsSubmit(terms: SeniorityTerms): void;
  waterfall: AnalysisState<modelsWire.RecoveryWaterfallResult>;
  workout: AnalysisState<modelsWire.WorkoutLgdResult>;
  stats: AnalysisState<SeniorityStats>;
  /** Rust's calibration ids, default first (useSeniorityCalibrationIds). */
  calibrationIds?: readonly string[] | null;
  /** Display text of a finite returned number; omitted shows four decimals. Every digit stays in the cell's title. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * A returned number to four decimals, never "-0", as relative-value-table
 * shows its numbers; every digit stays in the cell's title.
 */
const defaultValue = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 4,
  signDisplay: "negative",
}).format;
/** Rust's choices with their Rust descriptions. */
const options = (values: readonly { value: string; description?: string }[]) =>
  values.map(({ value, description }) => ({
    value,
    label: labelFor(value),
    description,
  }));
/** Typed text as a number; an empty field is NaN, never 0, so Rust rejects it. */
const typed = (value: string | undefined) =>
  !value || value.trim() === "" ? Number.NaN : Number(value);
type Row = Record<string, string> & { key: string };
const row = (fields: Record<string, string>): Row => ({
  ...fields,
  key: crypto.randomUUID(),
});
const text = (value: number | null) => (value === null ? "" : String(value));
/** Editable rows of field text; each row keeps a stable key across removals. */
function useRows(initial: () => Row[]) {
  const [rows, setRows] = useState(initial);
  return {
    rows,
    set: (key: string, field: string, value: string) =>
      setRows((prior) =>
        prior.map((r) => (r.key === key ? { ...r, [field]: value } : r)),
      ),
    add: (fields: Record<string, string>) =>
      setRows((prior) => [...prior, row(fields)]),
    remove: (key: string) =>
      setRows((prior) => prior.filter((r) => r.key !== key)),
  };
}
function TextField({
  label,
  help,
  value,
  onValueChange,
}: {
  label: string;
  help?: string;
  value: string;
  onValueChange(value: string): void;
}) {
  return (
    <FieldFrame label={label} help={help}>
      {(control) => (
        <Input
          {...control}
          type="text"
          value={value}
          onChange={(event) => onValueChange(event.target.value)}
        />
      )}
    </FieldFrame>
  );
}
const claimFields = [
  ["principal", "Principal", "Allowed principal, in the estate's unit."],
  ["accrued", "Accrued", "Accrued interest allowed in the claim."],
  ["penalties", "Penalties", "Allowed penalties or fees."],
  [
    "collateral_value",
    "Collateral value",
    "Gross value pledged to this claim alone; empty means unsecured.",
  ],
  [
    "collateral_haircut",
    "Collateral haircut",
    "Fraction of the collateral value lost, from 0 to 1.",
  ],
] as const;
/**
 * A credit analyst's view of recovery, as Rust returned it: how an estate is
 * allocated down a stack of claims under absolute priority, a workout LGD from
 * collateral, costs and time, and the historical recovery distribution of a
 * seniority class. Nothing is summed, divided or ranked here.
 */
export function RecoveryWaterfall({
  defaultWaterfall,
  defaultWorkout,
  defaultSeniority,
  onWaterfallSubmit,
  onWorkoutSubmit,
  onStatsSubmit,
  waterfall,
  workout,
  stats,
  calibrationIds,
  formatValue = defaultValue,
  density = "compact",
}: RecoveryWaterfallProps) {
  const [estate, setEstate] = useState(String(defaultWaterfall.estateValue));
  const claims = useRows(() =>
    defaultWaterfall.claims.map((claim) =>
      row({
        id: claim.id,
        seniority: claim.seniority,
        priority: String(claim.priority),
        principal: String(claim.principal),
        accrued: String(claim.accrued),
        penalties: String(claim.penalties),
        collateral_value: text(claim.collateral_value ?? null),
        collateral_haircut: String(claim.collateral_haircut),
      }),
    ),
  );
  const [costs, setCosts] = useState<Record<string, string>>(() => ({
    ead: String(defaultWorkout.ead),
    directCostPct: String(defaultWorkout.directCostPct),
    indirectCostPct: String(defaultWorkout.indirectCostPct),
    timeToResolutionYears: String(defaultWorkout.timeToResolutionYears),
    discountRate: String(defaultWorkout.discountRate),
  }));
  const pieces = useRows(() =>
    defaultWorkout.collateral.map(([type, bookValue, haircut]) =>
      row({ type, bookValue: String(bookValue), haircut: String(haircut) }),
    ),
  );
  const [seniority, setSeniority] = useState<string>(defaultSeniority);
  const [chosenCalibration, setChosenCalibration] = useState<string | null>(
    null,
  );
  // Rust lists its default calibration first.
  const calibrationId = chosenCalibration ?? calibrationIds?.[0] ?? "";
  const costField = (key: string, label: string, help: string) => (
    <DecimalInput
      key={key}
      label={label}
      help={help}
      value={costs[key] ?? ""}
      onValueChange={(value) =>
        setCosts((prior) => ({ ...prior, [key]: value }))
      }
    />
  );
  return (
    <section
      aria-label="Recovery"
      data-density={density}
      className="@container space-y-8 font-sans text-sm text-foreground"
    >
      <section aria-label="Recovery waterfall" className="space-y-4">
        <h3 className="text-sm font-medium">Recovery waterfall</h3>
        <form
          aria-label="Estate and claims"
          className="space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            onWaterfallSubmit({
              estateValue: typed(estate),
              claims: claims.rows.map((claim) => ({
                id: claim.id ?? "",
                seniority: claim.seniority ?? "",
                priority: typed(claim.priority),
                principal: typed(claim.principal),
                accrued: typed(claim.accrued),
                penalties: typed(claim.penalties),
                // An empty collateral value is an unsecured claim, never zero collateral.
                collateral_value: claim.collateral_value?.trim()
                  ? typed(claim.collateral_value)
                  : null,
                collateral_haircut: typed(claim.collateral_haircut),
              })),
            });
          }}
        >
          <div className="max-w-xs">
            <DecimalInput
              label="Estate value"
              help="Distributable estate including pledged collateral, in one monetary unit you choose."
              value={estate}
              onValueChange={setEstate}
            />
          </div>
          {claims.rows.map((claim, index) => (
            <fieldset
              key={claim.key}
              aria-label={`Claim ${claim.id || index}`}
              className="grid gap-4 border-t border-border pt-4 @lg:grid-cols-4"
            >
              <TextField
                label="Claim id"
                value={claim.id ?? ""}
                onValueChange={(value) => claims.set(claim.key, "id", value)}
              />
              <TextField
                label="Seniority"
                help="Your reporting label for the claim; priority decides the order."
                value={claim.seniority ?? ""}
                onValueChange={(value) =>
                  claims.set(claim.key, "seniority", value)
                }
              />
              <DecimalInput
                label="Priority"
                help="Whole number; lower recovers first, equal ranks share pro rata."
                value={claim.priority ?? ""}
                onValueChange={(value) =>
                  claims.set(claim.key, "priority", value)
                }
              />
              {claimFields.map(([field, label, help]) => (
                <DecimalInput
                  key={field}
                  label={label}
                  help={help}
                  value={claim[field] ?? ""}
                  onValueChange={(value) => claims.set(claim.key, field, value)}
                />
              ))}
              <div className="flex items-end">
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => claims.remove(claim.key)}
                >
                  Remove claim
                </Button>
              </div>
            </fieldset>
          ))}
          <div className="flex flex-wrap gap-3">
            <Button
              type="button"
              variant="outline"
              onClick={() =>
                claims.add({
                  id: "",
                  seniority: "",
                  priority: "",
                  principal: "",
                  accrued: "0",
                  penalties: "0",
                  collateral_value: "",
                  collateral_haircut: "0",
                })
              }
            >
              Add claim
            </Button>
            <Button type="submit">Allocate</Button>
          </div>
        </form>
        <Outcome state={waterfall} empty="No waterfall" working="Allocating…">
          {(result) => (
            <div className="space-y-3">
              <AllocationTable
                allocations={result.allocations}
                format={formatValue}
                density={density}
              />
              <FieldTable
                caption="Estate"
                rows={[
                  ["total_distributed", result.total_distributed],
                  ["undistributed_estate", result.undistributed_estate],
                  ["apr_satisfied", result.apr_satisfied],
                ]}
                format={formatValue}
                density={density}
              />
              <p className="text-xs text-muted-foreground">
                Rows in Rust&apos;s allocation order: priority, then entry
                order. Amounts in the unit entered; recovery rate is a fraction
                of the allowed claim.
              </p>
            </div>
          )}
        </Outcome>
      </section>
      <section aria-label="Workout LGD" className="space-y-4">
        <h3 className="text-sm font-medium">Workout LGD</h3>
        <form
          aria-label="Workout terms"
          className="space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            onWorkoutSubmit({
              ead: typed(costs.ead),
              collateral: pieces.rows.map((piece) => [
                piece.type as modelsWire.WorkoutCollateralType,
                typed(piece.bookValue),
                typed(piece.haircut),
              ]),
              directCostPct: typed(costs.directCostPct),
              indirectCostPct: typed(costs.indirectCostPct),
              timeToResolutionYears: typed(costs.timeToResolutionYears),
              discountRate: typed(costs.discountRate),
            });
          }}
        >
          <div className="grid gap-4 @lg:grid-cols-3">
            {costField(
              "ead",
              "Exposure at default",
              "Positive, in the collateral's monetary unit.",
            )}
            {costField(
              "directCostPct",
              "Direct costs",
              "Direct workout costs as a fraction of EAD.",
            )}
            {costField(
              "indirectCostPct",
              "Indirect costs",
              "Indirect workout costs as a fraction of EAD.",
            )}
            {costField(
              "timeToResolutionYears",
              "Time to resolution",
              "Expected workout duration in years.",
            )}
            {costField(
              "discountRate",
              "Discount rate",
              "Annual rate over the workout, as a decimal.",
            )}
          </div>
          {pieces.rows.map((piece, index) => (
            <fieldset
              key={piece.key}
              aria-label={`Collateral ${index}`}
              className="grid gap-4 border-t border-border pt-4 @lg:grid-cols-4"
            >
              <EnumField
                label="Collateral type"
                layout="select"
                value={piece.type ?? ""}
                onValueChange={(value) => pieces.set(piece.key, "type", value)}
                options={options(collateralTypes)}
              />
              <DecimalInput
                label="Book value"
                help="Book value of the piece, same unit as EAD."
                value={piece.bookValue ?? ""}
                onValueChange={(value) =>
                  pieces.set(piece.key, "bookValue", value)
                }
              />
              <DecimalInput
                label="Haircut"
                help="Fraction of book value lost in liquidation, from 0 to 1."
                value={piece.haircut ?? ""}
                onValueChange={(value) =>
                  pieces.set(piece.key, "haircut", value)
                }
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
              onClick={() =>
                pieces.add({
                  type: "",
                  bookValue: "",
                  haircut: "",
                })
              }
            >
              Add collateral
            </Button>
            <Button type="submit">Calculate LGD</Button>
          </div>
        </form>
        <Outcome state={workout} empty="No workout LGD" working="Calculating…">
          {(result) => (
            <div className="space-y-3">
              <FieldTable
                caption="Workout"
                rows={Object.entries(result)}
                format={formatValue}
                density={density}
              />
              <p className="text-xs text-muted-foreground">
                Net recovery in the unit entered, after costs and discounting;
                LGD and recovery rate are fractions of EAD.
              </p>
            </div>
          )}
        </Outcome>
      </section>
      <section aria-label="Seniority recovery" className="space-y-4">
        <h3 className="text-sm font-medium">Recovery by seniority</h3>
        <form
          aria-label="Seniority and calibration"
          className="space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            onStatsSubmit({
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
        <Outcome
          state={stats}
          empty="No recovery statistics"
          working="Loading…"
        >
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
    </section>
  );
}
/** Finite numbers through the host format; words and flags as Rust spells them. */
function shown(value: unknown, format: (value: number) => string) {
  return typeof value === "number" && Number.isFinite(value)
    ? format(value)
    : formatRaw(value as string | number | null | undefined);
}
function Outcome<T>({
  state,
  empty,
  working,
  children,
}: {
  state: AnalysisState<T>;
  empty: string;
  working: string;
  children: (result: T) => ReactNode;
}) {
  if (state.error)
    return (
      <p role="alert" className="text-error">
        {state.error}
      </p>
    );
  if (state.loading)
    return (
      <p role="status" className="text-muted-foreground">
        {working}
      </p>
    );
  if (!state.result)
    return (
      <p role="status" className="text-muted-foreground">
        {empty}
      </p>
    );
  return <>{children(state.result)}</>;
}
const allocationFields = [
  "seniority",
  "priority",
  "total_claim",
  "collateral_recovery",
  "general_recovery",
  "total_recovery",
  "recovery_rate",
  "deficiency",
] as const satisfies readonly (keyof modelsWire.RecoveryAllocation)[];
function AllocationTable({
  allocations,
  format,
  density,
}: {
  allocations: modelsWire.RecoveryAllocation[];
  format: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  const columns = useMemo<ColumnDef<{}, modelsWire.RecoveryAllocation, any>[]>(
    () => [
      {
        id: "id",
        header: labelFor("claim"),
        accessorFn: (allocation) => allocation.id,
        meta: { rowHeader: true },
      },
      ...allocationFields.map(
        (field): ColumnDef<{}, modelsWire.RecoveryAllocation, any> => ({
          id: field,
          header: labelFor(field),
          accessorFn: (allocation) => allocation[field],
          cell: ({ getValue }) => {
            const value = getValue();
            return <span title={String(value)}>{shown(value, format)}</span>;
          },
          meta:
            field === "seniority"
              ? {}
              : { className: "text-right finstack-numeric" },
        }),
      ),
    ],
    [format],
  );
  return (
    <FinstackTable
      data={allocations}
      columns={columns}
      getRowId={(allocation) => allocation.id}
      caption="Recovery by claim"
      density={density}
      emptyState="No claims"
    />
  );
}
interface Field {
  name: string;
  value: unknown;
}
function FieldTable({
  caption,
  rows,
  format,
  density,
}: {
  caption: string;
  rows: [string, unknown][];
  format: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  const columns = useMemo<ColumnDef<{}, Field, any>[]>(
    () => [
      {
        id: "name",
        header: "Field",
        accessorFn: (field) => field.name,
        cell: ({ getValue }) => labelFor(getValue() as string),
        meta: { rowHeader: true },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: (field) => field.value,
        cell: ({ getValue }) => {
          const value = getValue();
          return <span title={String(value)}>{shown(value, format)}</span>;
        },
        meta: { className: "text-right finstack-numeric" },
      },
    ],
    [format],
  );
  return (
    <FinstackTable
      data={rows.map(([name, value]) => ({ name, value }))}
      columns={columns}
      getRowId={(field) => field.name}
      caption={caption}
      density={density}
      emptyState="No fields returned"
    />
  );
}
