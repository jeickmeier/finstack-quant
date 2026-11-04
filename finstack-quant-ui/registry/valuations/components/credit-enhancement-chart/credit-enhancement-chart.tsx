"use client";
import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { colorLegend, defineChart, lineY, ruleY } from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import { scaleUtc } from "d3-scale";
import type {
  MetricFieldUnit,
  SimulationFieldUnits,
  StructuredCredit,
} from "finstack-quant-wasm";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { utc } from "@/components/finstack/shared/primitives/native-display/native-display";

type SimulationRun = ReturnType<StructuredCredit["runSimulation"]>;
type Period = NonNullable<SimulationRun["diagnostics"]["periods"]>[number];
type EnhancementRow = Period["credit_enhancement"][number] & {
  payment_date: string;
};
const dateScale = { scale: scaleUtc, axis: { label: "Payment date" } };
/** One line per tranche Rust returned; the chart's z channel draws them, and nothing is grouped here. */
function enhancementChart(
  rows: readonly EnhancementRow[],
  unit: MetricFieldUnit,
) {
  return defineChart({
    marks: [
      ruleY([0], { id: "no-enhancement", stroke: "var(--foreground)" }),
      lineY(rows, {
        x: (row: EnhancementRow) => utc(row.payment_date),
        y: (row: EnhancementRow) => row.credit_enhancement,
        z: (row: EnhancementRow) => row.tranche_id,
        color: (row: EnhancementRow) => row.tranche_id,
        key: (row: EnhancementRow) => `${row.tranche_id}|${row.payment_date}`,
        id: "credit-enhancement",
      }),
    ],
    scales: {
      x: dateScale,
      y: {
        scale: scaleLinear,
        nice: true,
        axis: { label: labelFor(unit.field) },
      },
    },
    color: { legend: colorLegend() },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks
          .map((mark) => mark.datum as EnhancementRow | number)
          .filter((datum): datum is EnhancementRow => Object(datum).tranche_id)
          .map((row) => ({
            label: `${row.tranche_id} · ${row.payment_date}`,
            value: formatRaw(row.credit_enhancement),
          })),
      }),
    },
  });
}
function excessSpreadChart(periods: readonly Period[], unit: MetricFieldUnit) {
  return defineChart({
    marks: [
      ruleY([0], { id: "no-excess-spread", stroke: "var(--foreground)" }),
      lineY(periods, {
        x: (period: Period) => utc(period.payment_date),
        y: (period: Period) => period.excess_spread,
        key: (period: Period) => period.payment_date,
        id: "excess-spread",
      }),
    ],
    scales: {
      x: dateScale,
      y: {
        scale: scaleLinear,
        nice: true,
        axis: { label: labelFor(unit.field) },
      },
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks
          .map((mark) => mark.datum as Period | number)
          .filter((datum): datum is Period => Object(datum).payment_date)
          .map((period) => ({
            label: period.payment_date,
            value: formatRaw(period.excess_spread),
          })),
      }),
    },
  });
}
/**
 * One row per period. Rust lists every tranche on every period in TrancheStructure
 * order, so a tranche's column reads its position in the row's own list.
 */
function columns(
  periods: readonly Period[],
  units: SimulationFieldUnits,
): ColumnDef<{}, Period, any>[] {
  return [
    {
      id: "payment_date",
      header: labelFor("payment_date"),
      accessorFn: (row) => row.payment_date,
      meta: { className: "", rowHeader: true },
    },
    {
      id: "excess_spread",
      header: () => (
        <span title={units.excess_spread.source}>
          {labelFor(units.excess_spread.field)} ({units.excess_spread.label})
        </span>
      ),
      accessorFn: (row) => formatRaw(row.excess_spread),
      meta: { className: "text-right finstack-numeric" },
    },
    ...periods[0]!.credit_enhancement.map(
      ({ tranche_id }, position): ColumnDef<{}, Period, any> => ({
        id: tranche_id,
        header: () => (
          <span title={units.credit_enhancement.source}>
            {tranche_id} ({units.credit_enhancement.label})
          </span>
        ),
        accessorFn: (row) =>
          formatRaw(row.credit_enhancement[position]?.credit_enhancement),
        meta: { className: "text-right finstack-numeric" },
      }),
    ),
  ];
}
export interface CreditEnhancementChartProps {
  /** Native StructuredCredit.runSimulation result; null while none is available. */
  run: SimulationRun | null;
  /** Native simulationFieldUnits for the deal currency; null while none is available. */
  units: SimulationFieldUnits | null;
  loading?: boolean;
  /** The native error of the simulation, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}
/**
 * Rust's end-of-period credit enhancement by tranche and realized excess spread by
 * date, then the same values in a table. One period is listed, not charted. Nothing
 * is computed, grouped, joined by date, sorted or classified.
 */
export function CreditEnhancementChart({
  run,
  units,
  loading,
  error,
  density,
}: CreditEnhancementChartProps) {
  const periods = useMemo(() => run?.diagnostics.periods ?? [], [run]);
  const charts = useMemo(
    () =>
      periods[1] && units
        ? {
            enhancement: enhancementChart(
              periods.flatMap(({ payment_date, credit_enhancement }) =>
                credit_enhancement.map((row) => ({ payment_date, ...row })),
              ),
              units.credit_enhancement,
            ),
            excess: excessSpreadChart(periods, units.excess_spread),
          }
        : null,
    [periods, units],
  );
  if (error)
    return (
      <p role="alert" className="text-sm text-error">
        {error}
      </p>
    );
  if (!run)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        {loading ? "Calculating…" : "No simulation returned"}
      </p>
    );
  if (!periods[0])
    return (
      <p role="status" className="text-sm text-muted-foreground">
        The simulation returned no periods
      </p>
    );
  if (!units)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        {loading ? "Calculating…" : "No units returned"}
      </p>
    );
  return (
    <section
      aria-label="Credit enhancement and excess spread"
      aria-busy={loading}
      className="flex min-w-0 flex-col gap-4 font-sans text-base text-foreground"
      data-density={density}
    >
      {charts ? (
        <>
          <FinstackChart
            definition={charts.enhancement}
            ariaLabel="Credit enhancement by tranche and date"
            subtitle={`${units.credit_enhancement.label}, at period end`}
            height={260}
          />
          <FinstackChart
            definition={charts.excess}
            ariaLabel="Excess spread by date"
            subtitle={`${units.excess_spread.label}, realized in each period`}
            height={220}
          />
        </>
      ) : (
        <p role="status" className="m-0 text-sm text-muted-foreground">
          The simulation returned one period, on {periods[0].payment_date}. A
          chart by date needs two, so what it returned is listed instead.
        </p>
      )}
      <FinstackTable
        caption="By payment date"
        data={periods}
        columns={columns(periods, units)}
        getRowId={(row) => String(periods.indexOf(row))}
        density={density}
      />
    </section>
  );
}
