"use client";
import { useCallback, useMemo, useState, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barY, defineChart } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { CapitalStructureSummary } from "@/workers/finstack-contract";
import { groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import resultMetadata from "@/lib/finstack/generated/meta/capital_structure_summary";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { IdCombobox } from "@/components/finstack/shared/primitives/id-combobox/id-combobox";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { capitalStructureConfigModule } from "./capital-structure-config";
import {
  passThrough,
  UndefinedValue,
} from "@/components/finstack/shared/primitives/native-display/native-display";
import type { CapitalStructureSummaryConfigWire } from "@/lib/finstack/generated/types/capital_structure_summary_config";

type Tranche = CapitalStructureSummary["tranches"][number];
type Money = CapitalStructureSummary["total_debt"];
/**
 * A returned ratio to four decimals, never "-0", as relative-value-table
 * shows its numbers; every digit stays in the cell's title.
 */
const defaultRatio = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 4,
  signDisplay: "negative",
}).format;
/** A returned share of debt (a 0-1 fraction) as a percent, scaled by Intl for display only. */
const defaultShare = new Intl.NumberFormat("en-US", {
  style: "percent",
  maximumFractionDigits: 2,
}).format;
/** Rust's own description of a returned field, from the generated contract. */
function described(definition: string, field: string): string | undefined {
  const suffix = `${definition}/properties/${field}`;
  return resultMetadata.find((entry) => entry.source.endsWith(suffix))
    ?.description;
}
/**
 * A value Rust returned, or Undefined: Rust returns null for a ratio it does
 * not define (a short LTM window, non-positive EBITDA or debt), and a live
 * worker may deliver that as NaN.
 */
function shown<T>(
  value: T | null | undefined,
  format: (value: T) => string,
  reason?: string,
): ReactNode {
  return value == null || (typeof value === "number" && Number.isNaN(value)) ? (
    <UndefinedValue reason={reason} />
  ) : (
    format(value)
  );
}
/** A returned number through the display format, with every digit in its title. */
function shownNumber(
  value: number | null | undefined,
  format: (value: number) => string,
  reason?: string,
): ReactNode {
  return value == null || Number.isNaN(value) ? (
    <UndefinedValue reason={reason} />
  ) : (
    <span title={String(value)}>{format(value)}</span>
  );
}
const amount = (money: Money) => groupDecimal(money.amount);
/** One bar of the maturity wall: a returned bucket, drawn from zero. */
export interface MaturityBar {
  year: string;
  /** The bucket's returned amount, as a chart position. */
  value: number;
  amount: Money;
  share: number | null | undefined;
}
/** The maturity wall exactly as Rust returns it, in its year order. */
export function maturityBars(summary: CapitalStructureSummary): MaturityBar[] {
  return summary.maturity_wall.map((bucket) => ({
    year: String(bucket.year),
    value: Number(bucket.amount.amount),
    amount: bucket.amount,
    share: bucket.share_of_debt,
  }));
}
/** Config fields that name a statement node, offered as the model's node ids. */
const nodeFieldPath = /^(ebitda_node|cash_node)$/;
/** A schema type that is a string or a nullable string. */
const isStringType = (type: unknown) =>
  type === "string" || (Array.isArray(type) && type.includes("string"));
export interface CapitalStructureReportProps {
  /** Initial CapitalStructureSummaryConfig; remount to load another. Omitted starts empty. */
  defaultConfig?: CapitalStructureSummaryConfigWire;
  /** Statement node ids offered for the EBITDA and cash nodes. */
  nodeIds: readonly string[];
  /** The model's period ids, in its order, offered for the reporting period. */
  periodIds: readonly string[];
  /** Receives the structurally valid config JSON on Summarize; Rust validates it. */
  onSubmit(configJson: string): void;
  /** Native statements_analytics.capitalStructureSummary result; null while none is available. */
  summary: CapitalStructureSummary | null | undefined;
  loading?: boolean;
  /** Native error (no capital structure, a node the results lack, mixed currencies), shown as returned. */
  error?: string;
  /** Display text of a returned ratio in turns; defaults to four decimals, every digit in the title. */
  formatRatio?: (value: number) => string;
  /** Display text of a returned share of debt (a decimal fraction); defaults to a percent to two decimals, every digit in the title. */
  formatShare?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Edits a Rust CapitalStructureSummaryConfig with the schema form over the
 * generated contract (the period over the model's periods, the EBITDA and
 * cash nodes as id-comboboxes) and presents the native capital structure
 * summary as returned, above the form: the headline debt, cash, LTM EBITDA,
 * leverage and coverage, the debt stack in Rust's rank-then-declaration order
 * with the figures through each class, and the maturity wall by year. A ratio
 * Rust does not define shows as Undefined beside the LTM periods Rust found.
 * Nothing is summed, netted or divided here.
 */
export function CapitalStructureReport({
  defaultConfig,
  nodeIds,
  periodIds,
  onSubmit,
  summary,
  loading,
  error,
  formatRatio = defaultRatio,
  formatShare = defaultShare,
  density = "compact",
}: CapitalStructureReportProps) {
  const [initial] = useState(() => defaultConfig);
  const field = useCallback<NonNullable<SchemaFormProps["renderField"]>>(
    ({ form, path, label, value, location }) => {
      const help =
        typeof location.schema.description === "string"
          ? location.schema.description
          : undefined;
      if (path === "period")
        return (
          <EnumField
            label={label}
            layout="select"
            value={typeof value === "string" ? value : ""}
            onValueChange={(next) => form.setFieldValue(path, next)}
            options={periodIds.map((id) => ({ value: id, label: id }))}
          />
        );
      return nodeFieldPath.test(path) && isStringType(location.schema.type) ? (
        <IdCombobox
          label={label}
          help={help}
          value={typeof value === "string" ? value : ""}
          onValueChange={(next) => form.setFieldValue(path, next)}
          options={nodeIds}
        />
      ) : undefined;
    },
    [nodeIds, periodIds],
  );
  return (
    <section
      aria-label="Capital structure"
      data-density={density}
      className="@container min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !summary ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading
            ? "Summarizing the capital structure…"
            : "No capital structure summary available."}
        </p>
      ) : (
        <CapitalStructureResults
          summary={summary}
          formatRatio={formatRatio}
          formatShare={formatShare}
          density={density}
        />
      )}
      <SchemaForm
        module={capitalStructureConfigModule}
        defaultValues={initial}
        layout="full"
        explicitNull={false}
        renderField={field}
        validate={passThrough}
        onSubmit={onSubmit}
        submitLabel="Summarize"
      />
    </section>
  );
}
function CapitalStructureResults({
  summary,
  formatRatio,
  formatShare,
  density,
}: {
  summary: CapitalStructureSummary;
  formatRatio: (value: number) => string;
  formatShare: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  const unit = ` (${summary.currency})`;
  const root = (field: string) => described("CapitalStructureSummary", field);
  const tranche = (field: string) =>
    described("CapitalStructureTranche", field);
  const ltmWindow = `${summary.ltm_periods_found} of ${summary.ltm_periods_required} periods found`;
  const headline: readonly (readonly [string, ReactNode, string?])[] = [
    ["total_debt", amount(summary.total_debt)],
    ["cash", shown(summary.cash, amount, root("cash"))],
    ["net_debt", shown(summary.net_debt, amount, root("net_debt"))],
    [
      "ltm_ebitda",
      shown(summary.ltm_ebitda, amount, root("ltm_ebitda")),
      ltmWindow,
    ],
    [
      "total_leverage",
      shownNumber(summary.total_leverage, formatRatio, root("total_leverage")),
    ],
    [
      "net_leverage",
      shownNumber(summary.net_leverage, formatRatio, root("net_leverage")),
    ],
    [
      "total_coverage",
      shownNumber(summary.total_coverage, formatRatio, root("total_coverage")),
    ],
  ];
  const columns: ColumnDef<{}, Tranche, any>[] = [
    {
      id: "id",
      header: labelFor("id"),
      accessorFn: (row) => row.id,
      meta: { className: "", rowHeader: true },
    },
    {
      id: "kind",
      header: labelFor("kind"),
      accessorFn: (row) => labelFor(row.kind),
      meta: { className: "" },
    },
    {
      id: "payment_class",
      header: labelFor("payment_class"),
      accessorFn: (row) => row.payment_class,
      cell: ({ getValue }) =>
        shown(getValue() as string | null, String, tranche("payment_class")),
      meta: { className: "" },
    },
    {
      id: "rank",
      header: labelFor("rank"),
      accessorFn: (row) => row.rank,
      meta: { className: "text-right finstack-numeric" },
    },
    {
      id: "balance",
      header: labelFor("balance"),
      accessorFn: (row) => amount(row.balance),
      meta: { className: "text-right finstack-numeric" },
    },
    {
      id: "share_of_debt",
      header: labelFor("share_of_debt"),
      accessorFn: (row) => row.share_of_debt,
      cell: ({ getValue }) =>
        shownNumber(
          getValue() as number | null,
          formatShare,
          tranche("share_of_debt"),
        ),
      meta: { className: "text-right finstack-numeric" },
    },
    {
      id: "cumulative_debt",
      header: labelFor("cumulative_debt"),
      accessorFn: (row) => amount(row.cumulative_debt),
      meta: { className: "text-right finstack-numeric" },
    },
    {
      id: "leverage",
      header: labelFor("leverage"),
      accessorFn: (row) => row.leverage,
      cell: ({ getValue }) =>
        shownNumber(
          getValue() as number | null,
          formatRatio,
          tranche("leverage"),
        ),
      meta: { className: "text-right finstack-numeric" },
    },
    {
      id: "interest_coverage",
      header: labelFor("interest_coverage"),
      accessorFn: (row) => row.interest_coverage,
      cell: ({ getValue }) =>
        shownNumber(
          getValue() as number | null,
          formatRatio,
          tranche("interest_coverage"),
        ),
      meta: { className: "text-right finstack-numeric" },
    },
    {
      id: "ltm_interest",
      header: labelFor("ltm_interest"),
      accessorFn: (row) => row.ltm_interest,
      cell: ({ getValue }) =>
        shown(getValue() as Money | null, amount, tranche("ltm_interest")),
      meta: { className: "text-right finstack-numeric" },
    },
    {
      id: "maturity",
      header: labelFor("maturity"),
      accessorFn: (row) => row.maturity,
      meta: { className: "finstack-numeric" },
    },
  ];
  const bars = useMemo(() => maturityBars(summary), [summary]);
  const definition = useMemo(
    () =>
      defineChart({
        marks: [
          barY(bars, {
            x: (bar: MaturityBar) => bar.year,
            y1: () => 0,
            y2: (bar: MaturityBar) => bar.value,
            key: (bar: MaturityBar) => bar.year,
            id: "maturity-wall",
          }),
        ],
        scales: {
          x: {
            scale: () =>
              scaleBand<string>()
                .domain(bars.map((bar) => bar.year))
                .padding(0.3),
            axis: { label: "Year of final maturity" },
          },
          y: {
            scale: scaleLinear,
            nice: true,
            axis: { label: `Balance maturing${unit}` },
          },
        },
        tooltip: {
          use: tooltip,
          content: (marks) => ({
            rows: marks.map((m) => ({
              label: m.datum.year,
              value: `${amount(m.datum.amount)} (${
                m.datum.share == null || Number.isNaN(m.datum.share)
                  ? "Undefined"
                  : formatShare(m.datum.share)
              } of debt)`,
            })),
          }),
        },
      }),
    [bars, unit, formatShare],
  );
  return (
    <div className="min-w-0 space-y-4">
      <dl
        aria-label={`Capital structure at ${summary.as_of}${unit}`}
        className="grid grid-cols-1 gap-2 @md:grid-cols-2 @3xl:grid-cols-4"
      >
        {headline.map(([field, value, note]) => (
          <div
            key={field}
            className="rounded-md border border-border bg-card px-3 py-2"
          >
            <dt className="text-xs text-muted-foreground">{labelFor(field)}</dt>
            <dd className="finstack-numeric text-base">{value}</dd>
            {note && <dd className="text-xs text-muted-foreground">{note}</dd>}
          </div>
        ))}
      </dl>
      <FinstackTable
        data={summary.tranches}
        columns={columns}
        getRowId={(row) => row.id}
        // The currency sits in the caption, as normalization-editor does, so the
        // columns stay narrow enough for the gallery width.
        caption={`Debt stack at ${summary.as_of}${unit}`}
        density={density}
      />
      <figure className="min-w-0">
        <figcaption className="text-xs text-muted-foreground">
          Maturity wall: the balance at {summary.as_of} by year of final
          maturity, ignoring later amortization.
        </figcaption>
        <FinstackChart
          definition={definition}
          ariaLabel={`Maturity wall at ${summary.as_of}`}
        />
      </figure>
    </div>
  );
}
