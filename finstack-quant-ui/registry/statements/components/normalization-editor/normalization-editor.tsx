"use client";
import { useCallback, useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barY, colorLegend, defineChart } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { NormalizationResult } from "@/workers/finstack-contract";
import { groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { normalizationConfigModule } from "./normalization-config";

/** Every supplied digit, grouped; a non-finite native value shows as itself. */
function exactValue(value: number): string {
  return Number.isFinite(value) ? groupDecimal(String(value)) : String(value);
}
/** Config fields that name a statement node, offered as the model's node ids. */
const nodeFieldPath = /(^|\.)(target_node|node_id|base_node)$/;
/** One bar of the bridge: a level from zero, or a step between two returned values. */
export interface Bar {
  id: string;
  label: string;
  kind: "Level" | "Adjustment" | "Capped adjustment";
  from: number;
  to: number;
  /** The returned amount the bar stands for: the level, or the capped amount. */
  amount: number;
  /** The returned amount before the cap, for a capped adjustment. */
  raw?: number;
}
/**
 * The bars of one period exactly as Rust returns them: the reported base from
 * zero, each adjustment from the previous step's `adjusted_value` (the base
 * for the first) to its own, and the final value from zero. Every position is
 * a returned value; nothing is added here.
 */
export function bridgeBars(result: NormalizationResult): Bar[] {
  const bars: Bar[] = [
    {
      id: "base_value",
      label: labelFor("base_value"),
      kind: "Level",
      from: 0,
      to: result.base_value,
      amount: result.base_value,
    },
  ];
  let previous = result.base_value;
  for (const step of result.adjustments) {
    bars.push({
      id: `adjustment:${step.adjustment_id}`,
      label: step.name,
      kind: step.is_capped ? "Capped adjustment" : "Adjustment",
      from: previous,
      to: step.adjusted_value,
      amount: step.capped_amount,
      raw: step.is_capped ? step.raw_amount : undefined,
    });
    previous = step.adjusted_value;
  }
  bars.push({
    id: "final_value",
    label: labelFor("final_value"),
    kind: "Level",
    from: 0,
    to: result.final_value,
    amount: result.final_value,
  });
  return bars;
}
/** One table row: the base, an adjustment (by its position, the same in every period) or the final value. */
type Row =
  | { id: string; label: string; kind: "base_value" | "final_value" }
  | { id: string; label: string; kind: "adjustment"; index: number };
export interface NormalizationEditorProps {
  /** Initial NormalizationConfig; remount to load another. Omitted starts empty. */
  defaultConfig?: Record<string, unknown>;
  /** Statement node ids offered for the target, percentage and cap-base nodes. */
  nodeIds: readonly string[];
  /** Native validation returning canonical NormalizationConfig JSON; connect useNormalizationConfigValidator. */
  validate: SchemaFormProps["validate"];
  /** Receives the canonical config on Normalize; connect it to the useNormalize request. */
  onSubmit(configJson: string): void;
  /** Native statements.normalize results, one per period in Rust's order; null while none is available. */
  results: readonly NormalizationResult[] | null | undefined;
  loading?: boolean;
  /** Native error (a missing node, a non-finite base, a negative cap), shown as returned. */
  error?: string;
  /** Display text of a returned number; defaults to every supplied digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Edits a Rust NormalizationConfig (the add-backs and deductions to a target
 * metric such as EBITDA) with the schema form over the generated contract and
 * presents the native statements.normalize results as returned, above the
 * form so the answer leads: for a chosen period, a bridge from the reported
 * base through each adjustment, in declaration order, to the adjusted value,
 * and a table of every adjustment's capped amount by period. Bars run between the values Rust returns for each
 * step (`adjusted_value`); nothing is summed, capped or classified here.
 */
export function NormalizationEditor({
  defaultConfig,
  nodeIds,
  validate,
  onSubmit,
  results,
  loading,
  error,
  formatValue = exactValue,
  density = "compact",
}: NormalizationEditorProps) {
  const [initial] = useState(() => defaultConfig);
  const nodeField = useCallback<NonNullable<SchemaFormProps["renderField"]>>(
    ({ form, path, label, value, location }) =>
      nodeFieldPath.test(path) && location.schema.type === "string" ? (
        <IdCombobox
          label={label}
          help={
            typeof location.schema.description === "string"
              ? location.schema.description
              : undefined
          }
          value={typeof value === "string" ? value : ""}
          onValueChange={(next) => form.setFieldValue(path, next)}
          options={nodeIds}
        />
      ) : undefined,
    [nodeIds],
  );
  return (
    <section
      aria-label="Normalization"
      data-density={density}
      className="@container min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !results ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Normalizing…" : "No normalization available."}
        </p>
      ) : results.length === 0 ? (
        <p role="status" className="text-sm text-muted-foreground">
          The target node has no periods to normalize.
        </p>
      ) : (
        <NormalizationResults
          results={results}
          formatValue={formatValue}
          density={density}
        />
      )}
      <SchemaForm
        module={normalizationConfigModule}
        defaultValues={initial}
        layout="full"
        explicitNull={false}
        renderField={nodeField}
        validate={validate}
        onSubmit={onSubmit}
        submitLabel="Normalize"
      />
    </section>
  );
}
function NormalizationResults({
  results,
  formatValue,
  density,
}: {
  results: readonly NormalizationResult[];
  formatValue: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  // A starting selection the user can change: the first period Rust returns.
  const [chosen, setChosen] = useState<string | null>(null);
  const shown =
    results.find((result) => result.period === chosen) ?? results[0];
  const unit =
    shown.value_type.type === "monetary"
      ? ` (${shown.value_type.currency})`
      : "";
  const caption = `Base to adjusted, ${shown.period}`;
  const bars = useMemo(() => bridgeBars(shown), [shown]);
  const definition = useMemo(
    () =>
      defineChart({
        marks: [
          barY(bars, {
            x: (bar: Bar) => bar.id,
            y1: (bar: Bar) => bar.from,
            y2: (bar: Bar) => bar.to,
            z: (bar: Bar) => bar.kind,
            key: (bar: Bar) => bar.id,
            id: "normalization-bridge",
          }),
        ],
        scales: {
          x: {
            scale: () =>
              scaleBand<string>()
                .domain(bars.map((bar) => bar.id))
                .padding(0.3),
            axis: {
              label: "Declaration order",
              ticks: {
                format: (id: string) =>
                  bars.find((bar) => bar.id === id)?.label ?? id,
              },
            },
          },
          y: {
            scale: scaleLinear,
            nice: true,
            axis: { label: `Value${unit}` },
          },
        },
        color: { legend: colorLegend() },
        tooltip: {
          use: tooltip,
          content: (marks) => ({
            rows: marks.map((m) => ({
              label: `${m.datum.label} · ${m.datum.kind}`,
              value:
                m.datum.raw === undefined
                  ? formatValue(m.datum.amount)
                  : `${formatValue(m.datum.amount)} (raw ${formatValue(m.datum.raw)})`,
            })),
          }),
        },
      }),
    [bars, unit, formatValue],
  );
  const rows: Row[] = [
    { id: "base_value", label: labelFor("base_value"), kind: "base_value" },
    ...results[0].adjustments.map((step, index): Row => ({
      id: `adjustment:${step.adjustment_id}`,
      label: step.name,
      kind: "adjustment",
      index,
    })),
    { id: "final_value", label: labelFor("final_value"), kind: "final_value" },
  ];
  const cell = (row: Row, result: NormalizationResult): string => {
    if (row.kind !== "adjustment") return formatValue(result[row.kind]);
    const step = result.adjustments[row.index];
    return step.is_capped
      ? `${formatValue(step.capped_amount)} (capped; raw ${formatValue(step.raw_amount)})`
      : formatValue(step.capped_amount);
  };
  const columns: ColumnDef<{}, Row, any>[] = [
    {
      id: "item",
      header: "Item",
      accessorFn: (row) => row.label,
      meta: { className: "", rowHeader: true },
    },
    ...results.map((result): ColumnDef<{}, Row, any> => ({
      id: result.period,
      header: result.period,
      accessorFn: (row) => cell(row, result),
      meta: { className: "text-right finstack-numeric" },
    })),
  ];
  return (
    <div className="min-w-0 space-y-4">
      <div className="max-w-xs">
        <EnumField
          label="Bridge period"
          layout="select"
          value={shown.period}
          onValueChange={setChosen}
          options={results.map((result) => ({
            value: result.period,
            label: result.period,
          }))}
        />
      </div>
      <p className="text-xs text-muted-foreground">
        Each adjustment runs from the value before it to the value after it,
        both as Rust returns them; a capped adjustment shows its capped amount
        and the raw amount before the cap.
      </p>
      <figure className="min-w-0">
        <FinstackChart definition={definition} ariaLabel={caption} />
      </figure>
      <FinstackTable
        data={rows}
        columns={columns}
        getRowId={(row) => row.id}
        caption={`Adjustments by period${unit}`}
        captionVisibility="sr-only"
        density={density}
      />
    </div>
  );
}
