"use client";
import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barY, colorLegend, defineChart } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { BridgeChart } from "@/workers/finstack-contract";
import { formatExact } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** One returned value of the bridge: a level, a driver's contribution or the residual. */
interface Bar {
  id: string;
  label: string;
  kind: "Level" | "Driver contribution" | "Unexplained";
  value: number;
}
export interface VarianceBridgeChartProps {
  /** Native BridgeChart; steps are shown in Rust's order. */
  bridge: BridgeChart;
  /** Display name of a node id; defaults to `labelFor`. */
  metricLabel?: (nodeId: string) => string;
  /** Display text of a returned value; defaults to every supplied digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Draws a native variance bridge as returned: the baseline value, one signed
 * bar from zero per driver contribution in Rust's order, the unexplained
 * residual and the comparison value, with the same values in a table. Each
 * bar starts at zero; nothing is accumulated, so this is not a waterfall
 * (Rust returns driver deltas in driver units, which need not sum to the
 * target's variance). Non-finite values are left out of the chart.
 */
export function VarianceBridgeChart({
  bridge,
  metricLabel = labelFor,
  formatValue = formatExact,
  density = "compact",
}: VarianceBridgeChartProps) {
  const caption = `${metricLabel(bridge.target_metric)} bridge, ${bridge.period}`;
  const bars: Bar[] = useMemo(
    () => [
      {
        id: "baseline",
        label: bridge.baseline_label,
        kind: "Level",
        value: bridge.baseline_value,
      },
      ...bridge.steps.map((step, index): Bar => ({
        id: `step-${index}`,
        label: metricLabel(step.driver),
        kind: "Driver contribution",
        value: step.contribution,
      })),
      {
        id: "unexplained",
        label: labelFor("unexplained"),
        kind: "Unexplained",
        value: bridge.unexplained ?? Number.NaN,
      },
      {
        id: "comparison",
        label: bridge.comparison_label,
        kind: "Level",
        value: bridge.comparison_value,
      },
    ],
    [bridge, metricLabel],
  );
  const definition = useMemo(() => {
    const drawn = bars.filter((bar) => Number.isFinite(bar.value));
    return defineChart({
      marks: [
        barY(drawn, {
          x: (bar: Bar) => bar.id,
          y1: 0,
          y2: (bar: Bar) => bar.value,
          z: (bar: Bar) => bar.kind,
          key: (bar: Bar) => bar.id,
          id: "variance-bridge",
        }),
      ],
      scales: {
        x: {
          scale: () =>
            scaleBand<string>()
              .domain(bars.map((bar) => bar.id))
              .padding(0.3),
          axis: {
            label: "Returned order",
            ticks: {
              format: (id: string) =>
                bars.find((bar) => bar.id === id)?.label ?? id,
            },
          },
        },
        y: {
          scale: scaleLinear,
          nice: true,
          axis: { label: metricLabel(bridge.target_metric) },
        },
      },
      color: { legend: colorLegend() },
      tooltip: {
        use: tooltip,
        content: (marks) => ({
          rows: marks.map((m) => ({
            label: `${m.datum.label} · ${m.datum.kind}`,
            value: formatValue(m.datum.value),
          })),
        }),
      },
    });
  }, [bars, bridge.target_metric, metricLabel, formatValue]);
  const columns: ColumnDef<{}, Bar, any>[] = [
    {
      id: "item",
      header: "Item",
      accessorFn: (bar) => bar.label,
      meta: { className: "", rowHeader: true },
    },
    { id: "kind", header: "Kind", accessorFn: (bar) => bar.kind },
    {
      id: "value",
      header: "Value",
      accessorFn: (bar) => formatValue(bar.value),
      meta: { className: "text-right finstack-numeric" },
    },
  ];
  return (
    <section
      aria-label={caption}
      data-density={density}
      className="min-w-0 space-y-3 font-sans text-sm text-foreground"
    >
      <p className="text-xs text-muted-foreground">
        Driver contributions are Rust's raw deltas in driver units, each drawn
        from zero; they need not add up to the change in{" "}
        {metricLabel(bridge.target_metric)}, and the unexplained bar is Rust's
        residual.
      </p>
      <figure className="min-w-0">
        <FinstackChart definition={definition} ariaLabel={caption} />
      </figure>
      <FinstackTable
        data={bars}
        columns={columns}
        getRowId={(bar) => bar.id}
        caption={caption}
        captionVisibility="sr-only"
        density={density}
      />
    </section>
  );
}
