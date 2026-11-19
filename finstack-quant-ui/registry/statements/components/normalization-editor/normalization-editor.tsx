"use client";
import { useCallback, useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barY, colorLegend, defineChart } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { NormalizationResult } from "@/workers/finstack-contract";
import { formatExact } from "@/lib/finstack/format/format";
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
import type { NormalizationConfigWire } from "@/lib/finstack/generated/types/normalization_config";

/** Config fields that name an existing statement node, offered as the model's node ids. */
const nodeFieldPath = /(^|\.)(target_node|node_id|base_node)$/;
/** A returned amount: a number for a scalar target, exact tagged money for a monetary one. */
type Amount = NonNullable<NormalizationResult["final_value"]>;
/** The float of a returned amount, for a chart position or display; money's decimal text is parsed, not rounded. */
export function amountNumber(amount: Amount): number {
  return typeof amount === "number" ? amount : Number(amount.amount);
}
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
  // A period Rust returns as unavailable has no base-to-adjusted bridge.
  if (result.base_value == null || result.final_value == null) return [];
  const base = amountNumber(result.base_value);
  const bars: Bar[] = [
    {
      id: "base_value",
      label: labelFor("base_value"),
      kind: "Level",
      from: 0,
      to: base,
      amount: base,
    },
  ];
  let previous = base;
  for (const step of result.adjustments ?? []) {
    const adjusted = amountNumber(step.adjusted_value);
    bars.push({
      id: `adjustment:${step.adjustment_id}`,
      label: step.name,
      kind: step.is_capped ? "Capped adjustment" : "Adjustment",
      from: previous,
      to: adjusted,
      amount: amountNumber(step.capped_amount),
      raw: step.is_capped ? amountNumber(step.raw_amount) : undefined,
    });
    previous = adjusted;
  }
  const final = amountNumber(result.final_value);
  bars.push({
    id: "final_value",
    label: labelFor("final_value"),
    kind: "Level",
    from: 0,
    to: final,
    amount: final,
  });
  return bars;
}
/** One table row: the base, an adjustment (by its position, the same in every period) or the final value. */
type Row =
  | { id: string; label: string; kind: "base_value" | "final_value" }
  | { id: string; label: string; kind: "adjustment"; index: number };
export interface NormalizationEditorProps {
  /** Initial NormalizationConfig; remount to load another. Omitted starts empty. */
  defaultConfig?: NormalizationConfigWire;
  /** Statement node ids offered for the target, percentage and cap-base nodes. */
  nodeIds: readonly string[];
  /** Native validation returning canonical NormalizationConfig JSON; connect useNormalizationConfigValidator. */
  validate: SchemaFormProps["validate"];
  /** Receives the canonical config on Normalize; connect it to the useNormalize request. */
  onSubmit(configJson: string): void;
  /** Native statements.normalize results, one per period in Rust's order; null while none is available. */
  results: readonly NormalizationResult[] | null | undefined;
  loading?: boolean;
  /** Native error (a missing node, mismatched units, a negative cap), shown as returned. */
  error?: string;
  /** Display text of a returned number; defaults to every supplied digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Edits a Rust NormalizationConfig (the ordered add-backs and deductions from a
 * reported metric such as EBITDA to a separately named adjusted metric) with
 * the schema form over the generated contract and
 * presents the native statements.normalize results as returned, above the
 * form so the answer leads: for a chosen period, a bridge from the reported
 * base through each adjustment, in declaration order, to the adjusted value,
 * and a table of every adjustment's capped amount by period. Bars run between the values Rust returns for each
 * step (`adjusted_value`); nothing is summed, capped or classified here. A
 * period Rust returns without an adjusted value shows its reason instead of a
 * bridge.
 */
export function NormalizationEditor({
  defaultConfig,
  nodeIds,
  validate,
  onSubmit,
  results,
  loading,
  error,
  formatValue = formatExact,
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
    ...(
      results.find((result) => result.adjustments?.length)?.adjustments ?? []
    ).map((step, index): Row => ({
      id: `adjustment:${step.adjustment_id}`,
      label: step.name,
      kind: "adjustment",
      index,
    })),
    { id: "final_value", label: labelFor("final_value"), kind: "final_value" },
  ];
  const cell = (row: Row, result: NormalizationResult): string => {
    if (row.kind !== "adjustment") {
      const value = result[row.kind];
      return value == null ? "Unavailable" : formatValue(amountNumber(value));
    }
    const step = result.adjustments?.[row.index];
    if (!step) return "Unavailable";
    const applied = formatValue(amountNumber(step.capped_amount));
    return step.is_capped
      ? `${applied} (capped; raw ${formatValue(amountNumber(step.raw_amount))})`
      : applied;
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
      {bars.length === 0 ? (
        <p role="status" className="text-sm text-muted-foreground">
          {shown.period} has no adjusted value:{" "}
          {labelFor(shown.unavailable ?? "missing_input")}.
        </p>
      ) : (
        <figure className="min-w-0">
          <FinstackChart definition={definition} ariaLabel={caption} />
        </figure>
      )}
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
