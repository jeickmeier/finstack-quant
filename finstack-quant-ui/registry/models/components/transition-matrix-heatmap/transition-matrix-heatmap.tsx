"use client";
import { useMemo, useState, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { colorLegend, defineChart, lineY } from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import { formatRaw } from "@/lib/finstack/format/format";
import {
  FinstackChart,
  heatmap,
} from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import {
  heatmapDomain,
  type HeatmapColors,
} from "@/components/finstack/shared/chart/finstack-chart/heatmap";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { ReportSection } from "@/components/finstack/shared/primitives/native-display/native-display";

/** A native models.credit.migration.TransitionMatrix as the worker's migrationMatrix returns it. */
export interface MigrationMatrixData {
  /** RatingScale#labels: row and column order. */
  labels: readonly string[];
  /** RatingScale#defaultState index; null when the scale has none. */
  defaultState: number | null;
  /** Horizon in years the probabilities cover. */
  horizon: number;
  /** TransitionMatrix#toRows: one row per origin state, in scale order. */
  rows: readonly (readonly number[])[];
  /** TransitionMatrix#defaultProbabilities; null when the scale has no default state. */
  defaultProbabilities: readonly number[] | null;
}
/** The worker's migrationProjection: the regularized generator and projected default probabilities. */
export interface MigrationProjectionData {
  labels: readonly string[];
  defaultState: number | null;
  generator: {
    /** GeneratorMatrix#toRows: intensities per year, one row per origin state. */
    rows: readonly (readonly number[])[];
    roundTripError: number;
    regularizationL1: number;
  };
  /** models.credit.migration.project per requested horizon, in request order. */
  projections: readonly {
    horizon: number;
    defaultProbabilities: readonly number[] | null;
  }[];
}
export interface TransitionMatrixHeatmapProps {
  /** The returned transition matrix; null while none is available. */
  matrix: MigrationMatrixData | null;
  loading?: boolean;
  /** The native matrix error, shown instead of every section. */
  error?: string;
  /** The returned generator and projections; null or omitted while none is available. */
  projection?: MigrationProjectionData | null;
  projectionLoading?: boolean;
  /** The native projection error, shown inline in place of the generator and cumulative sections. */
  projectionError?: string;
  /** Initial presentation of each section. */
  initialView?: "figure" | "table";
  /** Display text of a returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
interface Cell {
  from: string;
  to: string;
  value: number;
}
interface PdPoint {
  state: string;
  horizon: number;
  value: number;
}
const probabilityDomain: HeatmapColors = {
  palette: "sequential",
  domain: [0, 1],
};
function cells(
  labels: readonly string[],
  rows: readonly (readonly number[])[],
) {
  return rows.flatMap((row, i) =>
    row.map((value, j) => ({ from: labels[i]!, to: labels[j]!, value })),
  );
}
function matrixHeatmap(
  data: readonly Cell[],
  colors: HeatmapColors,
  valueLabel: string,
  formatValue: (value: number) => string,
) {
  return heatmap({
    data,
    x: (c) => c.to,
    y: (c) => c.from,
    value: (c) => c.value,
    key: (c) => `${c.from}|${c.to}`,
    xLabel: "To",
    yLabel: "From",
    valueLabel,
    formatValue,
    ...colors,
  });
}
type MatrixRow = { from: string; values: readonly (number | undefined)[] };
/** A first label column, then one numeric column per header, in the supplied order. */
function matrixColumns(
  firstHeader: string,
  headers: readonly string[],
  formatValue: (value: number) => string,
): ColumnDef<{}, MatrixRow, any>[] {
  return [
    {
      id: "from",
      header: firstHeader,
      accessorFn: (row: MatrixRow) => row.from,
      meta: { className: "text-left" },
    },
    ...headers.map((header, j) => ({
      id: `column-${j}`,
      header,
      accessorFn: (row: MatrixRow) => row.values[j],
      cell: ({ getValue }: { getValue(): unknown }) => {
        const value = getValue() as number | undefined;
        return value === undefined ? formatRaw(value) : formatValue(value);
      },
      meta: { className: "text-right finstack-numeric" },
    })),
  ];
}
function Status({ children }: { children: ReactNode }) {
  return (
    <p role="status" className="text-muted-foreground">
      {children}
    </p>
  );
}
/**
 * Presents a native rating transition matrix, its regularized generator and
 * the cumulative default probabilities Rust projects from it, as heatmaps and
 * a term-structure figure or as tables. Every value is drawn as returned; the
 * generator's round-trip error and regularization are stamped beside it.
 */
export function TransitionMatrixHeatmap({
  matrix,
  loading,
  error,
  ...props
}: TransitionMatrixHeatmapProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        Building transition matrix…
      </p>
    );
  if (!matrix)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        No transition matrix
      </p>
    );
  return <MatrixView matrix={matrix} {...props} />;
}
function MatrixView({
  matrix,
  projection,
  projectionLoading,
  projectionError,
  initialView = "figure",
  formatValue = String,
  density = "compact",
}: Omit<TransitionMatrixHeatmapProps, "matrix" | "loading" | "error"> & {
  matrix: MigrationMatrixData;
}) {
  const [view, setView] = useState(initialView);
  const labels = matrix.labels;
  const defaultLabel =
    matrix.defaultState === null ? null : labels[matrix.defaultState];
  const transition = useMemo(
    () =>
      matrixHeatmap(
        cells(labels, matrix.rows),
        probabilityDomain,
        `Probability over ${matrix.horizon}y (decimal)`,
        formatValue,
      ),
    [labels, matrix, formatValue],
  );
  const generator = useMemo(() => {
    if (!projection) return null;
    const data = cells(projection.labels, projection.generator.rows);
    const colors = heatmapDomain(data.map((c) => c.value));
    return colors
      ? matrixHeatmap(data, colors, "Intensity per year", formatValue)
      : null;
  }, [projection, formatValue]);
  const cumulative = useMemo(() => {
    if (!projection) return null;
    const points: PdPoint[] = projection.projections.flatMap((p) =>
      (p.defaultProbabilities ?? []).map((value, i) => ({
        state: projection.labels[i]!,
        horizon: p.horizon,
        value,
      })),
    );
    if (!points.length) return null;
    return defineChart({
      marks: [
        lineY(points, {
          x: (p: PdPoint) => p.horizon,
          y: (p: PdPoint) => p.value,
          z: (p: PdPoint) => p.state,
          key: (p: PdPoint) => `${p.state}|${p.horizon}`,
          id: "cumulative-pd",
        }),
      ],
      scales: {
        x: { scale: scaleLinear, axis: { label: "Horizon (years)" } },
        y: {
          scale: scaleLinear,
          domain: [0, 1],
          axis: { label: "Cumulative default probability (decimal)" },
        },
      },
      color: { legend: colorLegend() },
      tooltip: {
        use: tooltip,
        content: (marks) => ({
          rows: marks.map((m) => ({
            label: `${m.datum.state} · ${m.datum.horizon}y`,
            value: formatValue(m.datum.value),
          })),
        }),
      },
    });
  }, [projection, formatValue]);
  const columns = useMemo(
    () => matrixColumns("From", labels, formatValue),
    [labels, formatValue],
  );
  const transitionRows = labels.map((from, i) => ({
    from,
    values: matrix.rows[i] ?? [],
  }));
  return (
    <section
      aria-label="Rating migration"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <div className="flex flex-wrap items-end gap-4">
        <div className="min-w-48">
          <EnumField
            label="View"
            value={view}
            onValueChange={(value) => setView(value as "figure" | "table")}
            options={[
              { value: "figure", label: "Figure" },
              { value: "table", label: "Table" },
            ]}
            layout="radio"
          />
        </div>
        <p className="text-muted-foreground">
          {labels.length} states · horizon {matrix.horizon} years · default
          state {defaultLabel ?? "none"}
        </p>
      </div>
      <ReportSection title={`Transition probabilities over ${matrix.horizon}y`}>
        {view === "figure" ? (
          <FinstackChart
            {...transition}
            ariaLabel="Transition probability heatmap"
            height={360}
          />
        ) : (
          <FinstackTable
            data={transitionRows}
            columns={columns}
            getRowId={(row) => row.from}
            caption="Transition probabilities, origin state by destination state"
            captionVisibility="sr-only"
            density={density}
          />
        )}
      </ReportSection>
      {projectionError ? (
        <p role="alert" className="text-error">
          {projectionError}
        </p>
      ) : projectionLoading ? (
        <Status>Projecting from the generator…</Status>
      ) : !projection ? (
        <Status>No generator projection</Status>
      ) : (
        <>
          <ReportSection title="Generator (intensity per year)">
            <p
              aria-label="Generator fit"
              className="text-xs text-muted-foreground"
            >
              Round-trip error{" "}
              {formatValue(projection.generator.roundTripError)} ·
              regularization L1{" "}
              {formatValue(projection.generator.regularizationL1)}
            </p>
            {view === "figure" ? (
              generator ? (
                <FinstackChart
                  {...generator}
                  ariaLabel="Generator intensity heatmap"
                  height={360}
                />
              ) : (
                <Status>No range of generator intensities to colour</Status>
              )
            ) : (
              <FinstackTable
                data={projection.labels.map((from, i) => ({
                  from,
                  values: projection.generator.rows[i] ?? [],
                }))}
                columns={matrixColumns("From", projection.labels, formatValue)}
                getRowId={(row) => row.from}
                caption="Generator intensities per year, origin state by destination state"
                captionVisibility="sr-only"
                density={density}
              />
            )}
          </ReportSection>
          <ReportSection title="Cumulative default probability">
            {!cumulative ? (
              <Status>The scale has no default state</Status>
            ) : view === "figure" ? (
              <FinstackChart
                definition={cumulative}
                ariaLabel="Cumulative default probability by horizon"
                height={300}
              />
            ) : (
              <FinstackTable
                data={projection.labels.map((state, i) => ({
                  from: state,
                  values: projection.projections.map(
                    (p) => p.defaultProbabilities?.[i],
                  ),
                }))}
                columns={matrixColumns(
                  "State",
                  projection.projections.map((p) => `${p.horizon}y`),
                  formatValue,
                )}
                getRowId={(row) => row.from}
                caption="Cumulative default probability by state and horizon"
                captionVisibility="sr-only"
                density={density}
              />
            )}
          </ReportSection>
        </>
      )}
    </section>
  );
}
