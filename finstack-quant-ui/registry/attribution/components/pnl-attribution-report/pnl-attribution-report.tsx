"use client";
import { useMemo, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barY, defineChart } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { MoneyValue as Money, PnlAttribution } from "finstack-quant-wasm";
import { Badge } from "@/components/ui/badge";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { StampBadge } from "@/components/finstack/valuations/primitives/stamp-badge/stamp-badge";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";

/** The PnlAttribution buckets a report lists, in the Rust struct's field order; `residual` closes them. */
export const attributionFactorFields = [
  "carry",
  "rates_curves_pnl",
  "credit_curves_pnl",
  "inflation_curves_pnl",
  "correlations_pnl",
  "fx_pnl",
  "fx_translation_pnl",
  "vol_pnl",
  "cross_factor_pnl",
  "model_params_pnl",
  "market_scalars_pnl",
  "residual",
] as const;
export type AttributionFactorField = (typeof attributionFactorFields)[number];

export interface PnlAttributionReportProps {
  /** A native PnlAttribution, shown as returned. */
  attribution: PnlAttribution | null | undefined;
  /** Rust explain text (attribution.pnlAttributionExplainText); omitted shows none. */
  explain?: string;
  /** Rust residual check (attribution.pnlAttributionResidualWithinTolerance); omitted shows the returned residual figures without a verdict. */
  residualWithinTolerance?: boolean;
  /** Rust share of total P&L in percent per factor field (attribution.pnlAttributionPctOfTotal); null where Rust returns none. Omitted hides the column. */
  pctOfTotal?: Readonly<Partial<Record<AttributionFactorField, number | null>>>;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}

type FactorRow = {
  field: AttributionFactorField;
  money: Money;
  pct: number | null | undefined;
};
type Step = NonNullable<PnlAttribution["waterfall_steps"]>[number];

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="min-w-0 space-y-2">
      <h3 className="text-sm font-medium">{title}</h3>
      {children}
    </section>
  );
}

/** A returned AttributionMethod: a unit variant name, else its object as JSON text. */
function methodText(method: unknown) {
  return typeof method === "string" ? method : JSON.stringify(method);
}

const factorColumns = (withPct: boolean): ColumnDef<{}, FactorRow, any>[] => [
  {
    id: "factor",
    header: "Factor",
    accessorFn: (row) => labelFor(row.field),
    meta: { className: "text-left" },
  },
  {
    id: "pnl",
    header: "P&L",
    accessorFn: (row) => row.money.amount,
    meta: { className: "text-right finstack-numeric" },
    cell: (context) => <MoneyValue value={context.row.original.money} />,
  },
  ...(withPct
    ? [
        {
          id: "pct",
          header: "% of total",
          accessorFn: (row: FactorRow) => formatRaw(row.pct ?? null),
          meta: { className: "text-right finstack-numeric" },
        } satisfies ColumnDef<{}, FactorRow, any>,
      ]
    : []),
];

const stepColumns: ColumnDef<{}, Step, any>[] = [
  {
    id: "step",
    header: "Step",
    accessorFn: (step) => step.step_index,
    meta: { className: "text-right finstack-numeric" },
  },
  {
    id: "factor",
    header: "Factor",
    accessorFn: (step) => labelFor(step.factor),
    meta: { className: "text-left" },
  },
  {
    id: "before",
    header: "PV before",
    accessorFn: (step) => step.pv_before.amount,
    meta: { className: "text-right finstack-numeric" },
    cell: (context) => (
      <MoneyValue value={context.row.original.pv_before as Money} />
    ),
  },
  {
    id: "after",
    header: "PV after",
    accessorFn: (step) => step.pv_after.amount,
    meta: { className: "text-right finstack-numeric" },
    cell: (context) => (
      <MoneyValue value={context.row.original.pv_after as Money} />
    ),
  },
  {
    id: "pnl",
    header: "Step P&L",
    accessorFn: (step) => step.step_pnl.amount,
    meta: { className: "text-right finstack-numeric" },
    cell: (context) => (
      <MoneyValue value={context.row.original.step_pnl as Money} />
    ),
  },
];

/** Signed bars of each factor's returned P&L; the amounts are plotted, never summed. */
function factorChart(rows: readonly FactorRow[], currency: string) {
  return defineChart({
    marks: [
      barY(rows, {
        x: (row) => labelFor(row.field),
        y: (row) => Number(row.money.amount),
        key: (row) => row.field,
        id: "attribution-factors",
      }),
    ],
    scales: {
      x: {
        scale: () =>
          scaleBand<string>()
            .domain(rows.map((row) => labelFor(row.field)))
            .padding(0.3),
        axis: { label: "Factor" },
      },
      y: {
        scale: scaleLinear,
        nice: true,
        axis: { label: `P&L (${currency})` },
      },
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.map((m) => ({
          label: labelFor(m.datum.field),
          value: `${m.datum.money.currency} ${m.datum.money.amount}`,
        })),
      }),
    },
  });
}

/** Step label: Rust's step index and factor. */
const stepLabel = (step: Step) => `${step.step_index} ${labelFor(step.factor)}`;

/** Floating bars from each returned step's pv_before to its pv_after, in step order; no running total is formed here. */
function waterfallChart(steps: readonly Step[], currency: string) {
  return defineChart({
    marks: [
      barY(steps, {
        x: stepLabel,
        y1: (step) => Number(step.pv_before.amount),
        y2: (step) => Number(step.pv_after.amount),
        key: (step) => String(step.step_index),
        id: "attribution-waterfall",
      }),
    ],
    scales: {
      x: {
        scale: () =>
          scaleBand<string>().domain(steps.map(stepLabel)).padding(0.3),
        axis: { label: "Step (Rust order)" },
      },
      y: {
        scale: scaleLinear,
        nice: true,
        axis: { label: `Running PV (${currency})` },
      },
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.map((m) => ({
          label: stepLabel(m.datum),
          value: `${m.datum.pv_before.amount} → ${m.datum.pv_after.amount}`,
        })),
      }),
    },
  });
}

/** The residual as returned: Rust's verdict when supplied, else its figures with no verdict formed here. */
function ResidualBadge({
  attribution,
  within,
}: {
  attribution: PnlAttribution;
  within: boolean | undefined;
}) {
  const { meta } = attribution;
  return (
    <div
      aria-label="Residual"
      className="flex flex-wrap items-center gap-2 text-xs"
    >
      {attribution.result_invalid ? (
        <Badge variant="destructive">Result invalid</Badge>
      ) : within === undefined ? null : within ? (
        <Badge variant="secondary">Residual within tolerance</Badge>
      ) : (
        <Badge variant="destructive">Residual outside tolerance</Badge>
      )}
      <span className="text-muted-foreground">
        Residual <MoneyValue value={attribution.residual as Money} /> ·{" "}
        {formatRaw(meta.residual_pct)}% of total · tolerance{" "}
        {formatRaw(meta.tolerance_pct)}% or {formatRaw(meta.tolerance_abs)}{" "}
        {attribution.total_pnl.currency}
      </span>
    </div>
  );
}

/**
 * Display of a native single-instrument P&L attribution: the returned total,
 * mark-to-market and endpoint values, every factor bucket with an optional
 * Rust share of total, a signed factor chart, the waterfall drawn only from
 * Rust's `waterfall_steps`, the residual, notes, stamp and JSON. Nothing is
 * summed, differenced or converted.
 */
export function PnlAttributionReport({
  attribution,
  loading,
  error,
  ...props
}: PnlAttributionReportProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (!attribution)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        {loading ? "Attributing P&L…" : "No P&L attribution"}
      </p>
    );
  return <AttributionView attribution={attribution} {...props} />;
}

function AttributionView({
  attribution,
  explain,
  residualWithinTolerance,
  pctOfTotal,
  density,
}: Omit<PnlAttributionReportProps, "attribution" | "loading" | "error"> & {
  attribution: PnlAttribution;
}) {
  const currency = attribution.total_pnl.currency;
  const rows = useMemo(
    (): FactorRow[] =>
      attributionFactorFields.map((field) => ({
        field,
        money: attribution[field] as Money,
        pct: pctOfTotal?.[field],
      })),
    [attribution, pctOfTotal],
  );
  const steps = useMemo(() => attribution.waterfall_steps ?? [], [attribution]);
  const factors = useMemo(() => factorChart(rows, currency), [rows, currency]);
  const waterfall = useMemo(
    () => (steps.length ? waterfallChart(steps, currency) : null),
    [steps, currency],
  );
  const { meta } = attribution;
  return (
    <section
      aria-label="P&L attribution"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <dl className="grid grid-cols-2 gap-3 @2xl:grid-cols-4">
        {(
          [
            ["Total P&L", attribution.total_pnl],
            ["Mark-to-market P&L", attribution.mark_to_market_pnl],
            ["PV at T0", attribution.pv_t0],
            ["PV at T1", attribution.pv_t1],
          ] as const
        ).map(([term, money]) => (
          <div key={term} className="flex flex-col gap-0.5">
            <dt className="text-xs text-muted-foreground">{term}</dt>
            <dd className="finstack-numeric">
              {money ? (
                <MoneyValue value={money as Money} label={term} />
              ) : (
                <span className="text-muted-foreground">Not returned</span>
              )}
            </dd>
          </div>
        ))}
      </dl>
      <p className="text-xs text-muted-foreground">
        Method <span className="font-mono">{methodText(meta.method)}</span> ·{" "}
        {String(meta.t0)} → {String(meta.t1)} · instrument{" "}
        <span className="font-mono">{meta.instrument_id}</span> ·{" "}
        {String(meta.num_repricings)} repricings. Total P&L adds the period's
        cash receipts to the value change; mark-to-market P&L does not.
      </p>
      <ResidualBadge
        attribution={attribution}
        within={residualWithinTolerance}
      />
      <Section title="Factors">
        <FinstackTable
          caption="P&L by factor"
          captionVisibility="sr-only"
          data={rows}
          columns={factorColumns(pctOfTotal !== undefined)}
          getRowId={(row) => row.field}
          density={density}
        />
        <figure className="min-w-0">
          <FinstackChart
            definition={factors}
            ariaLabel="P&L by factor"
            height={220}
          />
        </figure>
      </Section>
      {waterfall && (
        <Section title="Waterfall">
          <figure className="min-w-0">
            <FinstackChart
              definition={waterfall}
              ariaLabel="Running present value by waterfall step"
              height={220}
            />
          </figure>
          <FinstackTable
            caption="Waterfall steps"
            captionVisibility="sr-only"
            data={steps}
            columns={stepColumns}
            getRowId={(step) => String(step.step_index)}
            density={density}
          />
        </Section>
      )}
      {explain && (
        <Section title="Explain">
          <pre className="overflow-x-auto rounded-md bg-muted p-3 font-mono text-xs">
            {explain}
          </pre>
        </Section>
      )}
      {meta.notes.length > 0 && (
        <Section title="Notes">
          <ul className="list-disc space-y-1 pl-5 text-xs text-muted-foreground">
            {meta.notes.map((note, index) => (
              <li key={index}>{note}</li>
            ))}
          </ul>
        </Section>
      )}
      <StampBadge meta={meta as unknown as Record<string, unknown>} />
      <JsonViewer
        label="Attribution JSON"
        text={JSON.stringify(attribution, null, 2)}
        downloadName="pnl-attribution.json"
        density={density}
      />
    </section>
  );
}
