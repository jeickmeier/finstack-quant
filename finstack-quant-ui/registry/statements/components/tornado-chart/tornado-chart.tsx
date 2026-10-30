"use client";
import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barX, colorLegend, defineChart, group } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { TornadoEntry } from "finstack-quant-wasm";
import { groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

interface Bar {
  parameter: string;
  side: "Downside" | "Upside";
  value: number;
}
/** Every supplied digit, grouped; a non-finite native value shows as itself. */
function exactValue(value: number): string {
  return Number.isFinite(value) ? groupDecimal(String(value)) : String(value);
}
export interface TornadoChartProps {
  /** Native tornado entries; shown in the order Rust returned them. */
  entries: readonly TornadoEntry[];
  /** What the deltas measure, with its unit (for example "Enterprise value change (USD)"). */
  valueLabel: string;
  caption: string;
  /** Display name of a parameter id; defaults to `labelFor`. */
  parameterLabel?: (parameterId: string) => string;
  /** Display text of a returned delta; defaults to every supplied digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Draws native tornado entries as horizontal bars from zero to each
 * returned downside and upside delta, parameters in Rust's order, with the
 * same values in a table. Deltas Rust returns as non-finite are left out of
 * the chart and shown in the table as returned. Nothing is sorted or summed.
 */
export function TornadoChart({
  entries,
  valueLabel,
  caption,
  parameterLabel = labelFor,
  formatValue = exactValue,
  density = "compact",
}: TornadoChartProps) {
  const chart = useMemo(() => {
    const parameters = entries.map((entry) =>
      parameterLabel(entry.parameter_id),
    );
    const bars: Bar[] = entries.flatMap((entry) =>
      (
        [
          ["Downside", entry.downside],
          ["Upside", entry.upside],
        ] as const
      ).flatMap(([side, value]) =>
        Number.isFinite(value)
          ? [{ parameter: parameterLabel(entry.parameter_id), side, value }]
          : [],
      ),
    );
    return {
      bars,
      definition: defineChart({
        marks: [
          barX(bars, {
            x1: 0,
            x2: (bar: Bar) => bar.value,
            y: (bar: Bar) => bar.parameter,
            z: (bar: Bar) => bar.side,
            key: (bar: Bar) => `${bar.parameter}|${bar.side}`,
            layout: group(),
            id: "tornado-deltas",
          }),
        ],
        scales: {
          x: { scale: scaleLinear, nice: true, axis: { label: valueLabel } },
          y: {
            scale: () => scaleBand<string>().domain(parameters).padding(0.3),
            axis: { label: "Parameter (returned order)" },
          },
        },
        color: { legend: colorLegend() },
        tooltip: {
          use: tooltip,
          content: (marks) => ({
            rows: marks.map((m) => ({
              label: `${m.datum.parameter} · ${m.datum.side}`,
              value: formatValue(m.datum.value),
            })),
          }),
        },
      }),
    };
  }, [entries, parameterLabel, formatValue, valueLabel]);
  const columns: ColumnDef<{}, TornadoEntry, any>[] = [
    {
      id: "parameter",
      header: "Parameter",
      accessorFn: (entry) => parameterLabel(entry.parameter_id),
    },
    ...(["downside", "upside"] as const).map(
      (side): ColumnDef<{}, TornadoEntry, any> => ({
        id: side,
        header: labelFor(side),
        accessorFn: (entry) => entry[side],
        meta: { className: "text-right finstack-numeric" },
        cell: (context) => formatValue(context.getValue() as number),
      }),
    ),
  ];
  return (
    <section
      aria-label={caption}
      data-density={density}
      className="min-w-0 space-y-3 font-sans text-sm text-foreground"
    >
      {chart.bars.length > 0 && (
        <figure className="min-w-0">
          <FinstackChart
            definition={chart.definition}
            ariaLabel={caption}
            // finstack-arith: chart height in pixels from the bar count
            height={Math.max(160, 72 * entries.length + 96)}
          />
        </figure>
      )}
      <FinstackTable
        data={[...entries]}
        columns={columns}
        getRowId={(entry) => entry.parameter_id}
        caption={caption}
        captionVisibility="sr-only"
        density={density}
        emptyState="No tornado entries returned"
      />
    </section>
  );
}
