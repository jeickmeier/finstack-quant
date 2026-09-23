"use client";
import { useMemo } from "react";
import { defineChart, dot } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { whenSelected } from "@tanstack/charts/selection";
import { tooltip } from "@tanstack/charts/tooltip";
import type { StatementResultJson } from "finstack-quant-wasm";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import {
  FinstackChart,
  chartSelection,
} from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import {
  adaptStatementResult,
  statementCellKey,
} from "../statement-grid/projection";

interface Point {
  nodeId: string;
  periodId: string;
  value: number;
}

/** Plot finite native node observations in the supplied period order. */
export function StatementChart({
  model,
  result,
  nodeId,
  link,
}: {
  model: FinancialModelSpecWire;
  result: StatementResultJson;
  nodeId: string;
  link?: LinkedSelection;
}) {
  const presentation = useMemo(() => {
    try {
      const checked = adaptStatementResult(result);
      const values = checked.nodes[nodeId];
      const points: Point[] = model.periods.flatMap((period) => {
        const value = values?.[period.id];
        return typeof value === "number" && Number.isFinite(value)
          ? [{ nodeId, periodId: period.id, value }]
          : [];
      });
      const kind = checked.node_value_types?.[nodeId];
      return {
        points,
        unit: kind?.type === "monetary" ? kind.currency : "Scalar",
      };
    } catch (error) {
      return { error: error instanceof Error ? error.message : String(error) };
    }
  }, [model, result, nodeId]);
  const definition = useMemo(() => {
    const points = presentation.points ?? [];
    const selection = link
      ? chartSelection<Point, string, number>(link, (p) =>
          statementCellKey(p.nodeId, p.periodId),
        )
      : undefined;
    const channels = {
      x: (p: Point) => p.periodId,
      y: (p: Point) => p.value,
      key: (p: Point) => statementCellKey(p.nodeId, p.periodId),
      r: 5,
    };
    return defineChart({
      marks: [
        dot(points, { ...channels, id: "statement-values" }),
        ...(selection
          ? [
              whenSelected(
                dot(points, {
                  ...channels,
                  id: "selected-statement-value",
                  r: 9,
                  fill: "none",
                  stroke: "var(--primary)",
                  strokeWidth: 2,
                }),
                selection,
              ),
            ]
          : []),
      ],
      scales: {
        x: { scale: scaleBand, axis: { label: "Supplied period" } },
        y: {
          scale: scaleLinear,
          nice: true,
          axis: { label: presentation.unit ?? "Native value" },
        },
      },
      selection,
      tooltip: {
        use: tooltip,
        content: (marks) => ({
          rows: marks.map((mark) => ({
            label: `${mark.datum.nodeId} · ${mark.datum.periodId}`,
            value: `${mark.datum.value} ${presentation.unit ?? ""}`.trim(),
          })),
        }),
      },
    });
  }, [presentation, link]);
  if (presentation.error)
    return (
      <p role="alert">Statement chart unavailable: {presentation.error}</p>
    );
  if (!presentation.points?.length)
    return <p role="status">No finite returned values for {nodeId}</p>;
  return (
    <section aria-label={`${nodeId} statement chart`}>
      <FinstackChart
        definition={definition}
        ariaLabel={`${nodeId} by supplied period`}
        title={model.nodes[nodeId]?.name ?? nodeId}
        subtitle="Returned native values; exact monetary amounts remain in the statement grid"
        height={280}
      />
    </section>
  );
}
