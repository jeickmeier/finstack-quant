"use client";
import { useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { StatementResult } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type {
  RunSensitivityRequest,
  StatementSensitivity,
} from "@/workers/finstack-contract";
import { groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { TornadoChart } from "@/components/finstack/statements/components/tornado-chart/tornado-chart";
import { evaluatedNumber, sensitivityModes } from "./sensitivity";

/** The run as submitted, without the model the caller supplies. */
export type StatementSensitivitySubmission = Omit<
  RunSensitivityRequest,
  "modelJson"
>;
/** One parameter row as the caller seeds it; the base value is always read from the evaluated result. */
export interface StatementSensitivityParameter {
  nodeId: string;
  periodId: string;
  /** Percentage moves of the base value, such as `[-10, 10]`. */
  pctRange: readonly number[];
}
/** Initial form values; every omitted field starts empty. */
export interface StatementSensitivityDefaults {
  mode?: string;
  targetMetric?: string;
  period?: string;
  parameters?: readonly StatementSensitivityParameter[];
}
interface Row {
  nodeId: string;
  periodId: string;
  /** Comma-separated percentage moves as typed. */
  moves: string;
}
interface ScenarioRow {
  id: string;
  label: string;
  parameters: string;
  value: unknown;
}
/** Every supplied digit, grouped; a non-finite native value shows as itself. */
function exactValue(value: number): string {
  return Number.isFinite(value) ? groupDecimal(String(value)) : String(value);
}
/** Typed moves as numbers; an empty entry is NaN, never 0, so Rust rejects it. */
function moves(text: string): number[] {
  return (text.trim() === "" ? [""] : text.split(",")).map((item) =>
    item.trim() === "" ? Number.NaN : Number(item),
  );
}
/** `revenue@2025Q4` as "Revenue · 2025Q4"; the key is Rust's `node_id@period_id`. */
function parameterLabel(key: string): string {
  const at = key.lastIndexOf("@");
  return at < 0
    ? labelFor(key)
    : // finstack-arith: string offset past the @ separator
      `${labelFor(key.slice(0, at))} · ${key.slice(at + 1)}`;
}
/** A returned cell as text: grouped digits for a number, the sentinel or a dash otherwise. */
function cellText(value: unknown): string {
  if (typeof value === "number") return exactValue(value);
  return value === undefined ? "—" : String(value);
}
export interface StatementSensitivityProps {
  /** Evaluated result of the current model; each parameter's base value is read from it. */
  result: StatementResult;
  /** Model node ids offered for the target metric and the parameters. */
  nodeIds: readonly string[];
  /** Model period ids, in model order. */
  periods: readonly string[];
  /** Initial values; remount to load others. Omitted starts empty. */
  defaultValues?: StatementSensitivityDefaults;
  /** Receives the run on Run sensitivity; Rust validates every value. */
  onSubmit(submission: StatementSensitivitySubmission): void;
  /** Native result and tornado of the submitted run; null while none is available. */
  data: StatementSensitivity | null | undefined;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  disabled?: boolean;
  density?: "compact" | "comfortable";
}
/**
 * Edits a native sensitivity run of a statement model: the SensitivityMode,
 * the target metric and period, and each varied node and period with its
 * percentage moves (statements_analytics.parameterSpecWithPercentages, base
 * value read from the evaluated cell). Presents the native tornado through
 * TornadoChart and the target metric in every returned scenario, in Rust's
 * order. Nothing is calculated here.
 */
export function StatementSensitivity({
  result,
  nodeIds,
  periods,
  defaultValues,
  onSubmit,
  data,
  loading,
  error,
  disabled,
  density = "compact",
}: StatementSensitivityProps) {
  const [mode, setMode] = useState(defaultValues?.mode ?? "");
  const [targetMetric, setTargetMetric] = useState(
    defaultValues?.targetMetric ?? "",
  );
  const [period, setPeriod] = useState(defaultValues?.period ?? "");
  const [rows, setRows] = useState<Row[]>(() =>
    (defaultValues?.parameters?.length
      ? defaultValues.parameters
      : [{ nodeId: "", periodId: "", pctRange: [] }]
    ).map((parameter) => ({
      nodeId: parameter.nodeId,
      periodId: parameter.periodId,
      moves: parameter.pctRange.join(", "),
    })),
  );
  // The target metric and period of the run on screen, for its scenario table.
  const [shown, setShown] = useState({
    targetMetric: defaultValues?.targetMetric ?? "",
    period: defaultValues?.period ?? "",
  });
  const update = (index: number, patch: Partial<Row>) =>
    setRows((prior) =>
      prior.map((row, at) => (at === index ? { ...row, ...patch } : row)),
    );
  const submit = () => {
    setShown({ targetMetric, period });
    onSubmit({
      mode,
      targetMetric,
      period,
      parameters: rows.map((row) => ({
        nodeId: row.nodeId,
        periodId: row.periodId,
        baseValue: evaluatedNumber(result, row.nodeId, row.periodId),
        pctRange: moves(row.moves),
      })),
    });
  };
  const periodOptions = periods.map((id) => ({ value: id, label: id }));
  const scenarios: ScenarioRow[] = data
    ? [
        ...(data.result.baseline
          ? [
              {
                id: "baseline",
                label: "Baseline",
                parameters: "Unperturbed",
                value:
                  data.result.baseline.nodes[shown.targetMetric]?.[
                    shown.period
                  ],
              },
            ]
          : []),
        ...data.result.scenarios.map((scenario, index) => ({
          id: String(index),
          // finstack-arith: 1-based ordinal for a label
          label: `Scenario ${index + 1}`,
          parameters: Object.entries(scenario.parameter_values)
            .map(
              ([key, value]) => `${parameterLabel(key)} = ${exactValue(value)}`,
            )
            .join("; "),
          value: scenario.results.nodes[shown.targetMetric]?.[shown.period],
        })),
      ]
    : [];
  const columns: ColumnDef<{}, ScenarioRow, any>[] = [
    {
      id: "scenario",
      header: "Scenario",
      accessorFn: (row) => row.label,
      meta: { className: "", rowHeader: true },
    },
    {
      id: "parameters",
      header: "Varied parameter value",
      accessorFn: (row) => row.parameters,
    },
    {
      id: "value",
      header: `${labelFor(shown.targetMetric)} · ${shown.period}`,
      accessorFn: (row) => cellText(row.value),
      meta: { className: "text-right finstack-numeric" },
    },
  ];
  return (
    <section
      aria-label="Sensitivity analysis"
      data-density={density}
      className="@container min-w-0 space-y-4 font-sans text-sm text-foreground"
    >
      <div className="grid gap-4 @lg:grid-cols-3">
        <EnumField
          label="Mode"
          layout="radio"
          value={mode}
          onValueChange={setMode}
          options={sensitivityModes.map((value) => ({
            value,
            label: labelFor(value),
          }))}
        />
        <IdCombobox
          label="Target metric"
          help="Node tracked in every scenario and ranked by the tornado."
          value={targetMetric}
          onValueChange={setTargetMetric}
          options={nodeIds}
        />
        <EnumField
          label="Tornado period"
          layout="select"
          value={period}
          onValueChange={setPeriod}
          options={periodOptions}
        />
      </div>
      <fieldset className="space-y-3 rounded-md border border-border p-3">
        <legend className="px-1 text-sm font-medium">Varied parameters</legend>
        {rows.map((row, index) => {
          const base = row.nodeId
            ? result.nodes[row.nodeId]?.[row.periodId]
            : undefined;
          return (
            <div
              key={index}
              role="group"
              // finstack-arith: 1-based ordinal for a label
              aria-label={`Parameter ${index + 1}`}
              className="grid items-end gap-3 @lg:grid-cols-[minmax(0,1fr)_10rem_minmax(0,1fr)_auto]"
            >
              <IdCombobox
                label="Node"
                value={row.nodeId}
                onValueChange={(nodeId) => update(index, { nodeId })}
                options={nodeIds}
              />
              <EnumField
                label="Period"
                layout="select"
                value={row.periodId}
                onValueChange={(periodId) => update(index, { periodId })}
                options={periodOptions}
              />
              <FieldFrame
                label="Percentage moves"
                help={`Comma-separated percent moves of the evaluated base value (${cellText(base)}); -10 is a 10% decrease.`}
              >
                {(control) => (
                  <Input
                    {...control}
                    className="font-mono"
                    value={row.moves}
                    onChange={(event) =>
                      update(index, { moves: event.target.value })
                    }
                  />
                )}
              </FieldFrame>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={rows.length === 1}
                onClick={() =>
                  setRows((prior) => prior.filter((_, at) => at !== index))
                }
              >
                Remove
              </Button>
            </div>
          );
        })}
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() =>
            setRows((prior) => [
              ...prior,
              { nodeId: "", periodId: "", moves: "" },
            ])
          }
        >
          Add parameter
        </Button>
      </fieldset>
      <Button type="button" onClick={submit} disabled={disabled}>
        Run sensitivity
      </Button>
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !data ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading
            ? "Running the sensitivity…"
            : "No sensitivity run available."}
        </p>
      ) : (
        <>
          {data.tornado ? (
            <TornadoChart
              entries={data.tornado}
              valueLabel={`${labelFor(shown.targetMetric)} change at ${shown.period}`}
              caption="Sensitivity tornado"
              parameterLabel={parameterLabel}
              density={density}
            />
          ) : (
            <p className="text-sm text-muted-foreground">
              No tornado: Rust ranks only one-at-a-time runs, and a full-grid
              run combines the parameters.
            </p>
          )}
          <FinstackTable
            data={scenarios}
            columns={columns}
            getRowId={(row) => row.id}
            caption="Returned sensitivity scenarios"
            density={density}
            emptyState="No scenarios returned"
          />
        </>
      )}
    </section>
  );
}
