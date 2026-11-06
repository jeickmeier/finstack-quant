"use client";
import { useMemo, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barY, defineChart } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { portfolio } from "finstack-quant-wasm/types";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** portfolio.brinsonLinked as returned: each period's Brinson-Fachler effects, the Carino-linked effects by sector and by effect, and the compounded portfolio and benchmark returns. */
export type CarinoLinkedAttribution = portfolio.CarinoLinkedAttribution;
/** One sector's start-of-period weights and period returns on both sides, as decimals. */
export type SectorPeriod = portfolio.SectorPeriod;
/** The host's attribution periods; nothing is defaulted. */
export interface BrinsonReportInputs {
  /** One array of SectorPeriod rows per period, oldest first; every period lists the same sectors in the same order, and each side's weights sum to one. */
  readonly periods: readonly (readonly SectorPeriod[])[];
  /** The host's name of each period, in the same order (for example its end date); omitted shows the period's position. */
  readonly periodLabels?: readonly string[];
}
export interface BrinsonReportProps {
  /** The native result; null while none is available. */
  report: CarinoLinkedAttribution | null | undefined;
  /** The host's name of each returned period, in order; omitted shows "Period 1", "Period 2" and so on. */
  periodLabels?: readonly string[];
  loading?: boolean;
  /** The native error of the request, shown as returned. */
  error?: string;
  /** Display text of a returned decimal; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}

type SectorEffect = CarinoLinkedAttribution["linked_sectors"][number];
const effects = ["allocation", "selection", "interaction", "total"] as const;
/** One-based period number shown when the host supplies no label. */
// finstack-arith: display ordinal of a period, never sent to Rust
const ordinal = (index: number) => index + 1;

function Value({
  value,
  formatValue,
}: {
  value: number;
  formatValue: (value: number) => string;
}) {
  return (
    <span className="finstack-numeric" title={String(value)}>
      {formatValue(value)}
    </span>
  );
}
function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section aria-label={title} className="min-w-0 space-y-2">
      <h3 className="text-sm font-medium">{title}</h3>
      {description && (
        <p className="text-xs text-muted-foreground">{description}</p>
      )}
      {children}
    </section>
  );
}
function effectColumns(
  formatValue: (value: number) => string,
): ColumnDef<{}, SectorEffect, any>[] {
  return [
    {
      id: "sector",
      header: labelFor("sector"),
      accessorFn: (row) => row.sector,
      meta: { className: "text-left" },
      cell: (context) => (
        <span className="font-mono">{context.row.original.sector}</span>
      ),
    },
    ...effects.map((effect): ColumnDef<{}, SectorEffect, any> => ({
      id: effect,
      header: labelFor(effect),
      accessorFn: (row) => row[effect],
      meta: { className: "text-right" },
      cell: (context) => (
        <Value value={context.row.original[effect]} formatValue={formatValue} />
      ),
    })),
  ];
}
/** Bars of each sector's returned linked total; the values are plotted, never summed or stacked. */
function linkedChart(sectors: readonly SectorEffect[]) {
  return defineChart({
    marks: [
      barY(sectors, {
        x: (row) => row.sector,
        y: (row) => row.total,
        key: (row) => row.sector,
        id: "brinson-linked-totals",
      }),
    ],
    scales: {
      x: {
        scale: () =>
          scaleBand<string>()
            .domain(sectors.map((row) => row.sector))
            .padding(0.3),
        axis: { label: "Sector (supplied order)" },
      },
      y: {
        scale: scaleLinear,
        nice: true,
        axis: { label: "Linked effect (decimal)" },
      },
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.map((mark) => ({
          label: mark.datum.sector,
          value: String(mark.datum.total),
        })),
      }),
    },
  });
}

/**
 * Presents native multi-period Brinson-Fachler attribution
 * (portfolio.brinsonLinked) as returned: the compounded portfolio and
 * benchmark returns, the Carino-linked allocation, selection, interaction and
 * total of each sector with the linked total of each effect, a chart of the
 * sectors' linked totals, and every period's sector effects with that period's
 * returns and totals. Every figure is a decimal Rust returned; nothing is
 * summed, differenced or linked here.
 */
export function BrinsonReport({
  report,
  periodLabels,
  loading,
  error,
  formatValue = String,
  density,
}: BrinsonReportProps) {
  const columns = useMemo(() => effectColumns(formatValue), [formatValue]);
  const chart = useMemo(
    () => (report ? linkedChart(report.linked_sectors) : null),
    [report],
  );
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (!report)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        {loading ? "Linking attribution…" : "No Brinson attribution"}
      </p>
    );
  const value = (number: number) => (
    <Value value={number} formatValue={formatValue} />
  );
  return (
    <section
      aria-label="Brinson attribution"
      data-density={density}
      className="@container min-w-0 max-w-full space-y-6 font-sans text-sm text-foreground"
    >
      <Section
        title="Compounded returns"
        description="Each side's period returns compounded geometrically over every period, as decimals. The linked effects below sum to their difference."
      >
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
          <dt>{labelFor("portfolio_return_compounded")}</dt>
          <dd>{value(report.portfolio_return_compounded)}</dd>
          <dt>{labelFor("benchmark_return_compounded")}</dt>
          <dd>{value(report.benchmark_return_compounded)}</dd>
        </dl>
      </Section>
      <Section
        title="Linked effects by sector"
        description="Carino-smoothed Brinson-Fachler effects of each sector over every period, as decimals of active return. The footer is Rust's linked total of each effect."
      >
        <FinstackTable
          caption="Linked effects by sector"
          captionVisibility="sr-only"
          data={report.linked_sectors}
          columns={columns}
          getRowId={(row) => row.sector}
          totals={{
            sector: "Linked total",
            allocation: value(report.linked_allocation),
            selection: value(report.linked_selection),
            interaction: value(report.linked_interaction),
          }}
          emptyState="No sectors returned"
          density={density}
        />
        {chart && report.linked_sectors.length > 0 && (
          <figure className="min-w-0">
            <FinstackChart
              definition={chart}
              ariaLabel="Linked effect by sector"
              height={220}
            />
          </figure>
        )}
      </Section>
      {report.periods.map((period, index) => {
        const name = periodLabels?.[index] ?? `Period ${ordinal(index)}`;
        return (
          <Section key={index} title={`${name} attribution`}>
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 @lg:grid-cols-[auto_1fr_auto_1fr_auto_1fr]">
              <dt>{labelFor("portfolio_return")}</dt>
              <dd>{value(period.portfolio_return)}</dd>
              <dt>{labelFor("benchmark_return")}</dt>
              <dd>{value(period.benchmark_return)}</dd>
              <dt>{labelFor("total_excess_return")}</dt>
              <dd>{value(period.total_excess_return)}</dd>
            </dl>
            <FinstackTable
              caption={`${name} effects by sector`}
              captionVisibility="sr-only"
              data={period.sectors}
              columns={columns}
              getRowId={(row) => row.sector}
              totals={{
                sector: "Period total",
                allocation: value(period.total_allocation),
                selection: value(period.total_selection),
                interaction: value(period.total_interaction),
                total: value(period.total_excess_return),
              }}
              emptyState="No sectors returned"
              density={density}
            />
          </Section>
        );
      })}
    </section>
  );
}
