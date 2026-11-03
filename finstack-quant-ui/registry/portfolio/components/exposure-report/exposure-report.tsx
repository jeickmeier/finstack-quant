"use client";
import { useMemo, type ReactNode } from "react";
import type { ColumnDef, RowData } from "@tanstack/react-table";
import { barY, colorLegend, defineChart, group } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type {
  MoneyValue as Money,
  SensitivityMatrixJson,
} from "finstack-quant-wasm";
import type { portfolio } from "finstack-quant-wasm/types";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** portfolio.primitiveExposures as returned: position-aware primitive paths and per-primitive net and gross quantity, value and additive risk, in the base currency. */
export type PortfolioPrimitiveExposureReport =
  portfolio.PortfolioPrimitiveExposureReport;
/** portfolio.computeFactorSensitivities as returned: positions by factors in the base currency. */
export type { SensitivityMatrixJson };
/** The host's exposure choices for the workbench Risk tab; nothing is defaulted by Rust or here. */
export interface ExposureReportInputs {
  /** Additive metric ids Rust reports per primitive (for example "dv01"); omitted requests quantity and value only. */
  readonly metrics?: readonly string[];
  /** FactorDefinition array JSON the book's sensitivity matrix is computed against; omitted shows no matrix. */
  readonly factorsJson?: string;
  /** BumpSizeConfig JSON of that matrix; omitted is Rust's 1 bp / 1 % per factor type. */
  readonly bumpConfigJson?: string;
}
export interface ExposureReportProps {
  /** The native exposure report; null while none is available. */
  report: PortfolioPrimitiveExposureReport | null | undefined;
  loading?: boolean;
  /** The native error of the exposure request, shown as returned. */
  error?: string;
  /** The supplied sensitivity matrix; omitted (with no loading or error) leaves the section out. */
  sensitivities?: SensitivityMatrixJson | null;
  sensitivitiesLoading?: boolean;
  /** The native error of the sensitivity request, shown in its section. */
  sensitivitiesError?: string;
  /** Display text of a returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}

type Aggregate = PortfolioPrimitiveExposureReport["aggregates"][number];
type Path = PortfolioPrimitiveExposureReport["paths"][number];
interface RiskRow {
  instrument_id: string;
  metric: string;
  net: number;
  gross: number;
}
interface ValueBar {
  instrument_id: string;
  side: string;
  amount: number;
}
interface SensitivityRow {
  position_id: string;
  values: readonly number[];
}
type Format = (value: number) => string;

function Value({ value, formatValue }: { value: number; formatValue: Format }) {
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
const id = <T extends RowData>(
  key: string,
  read: (row: T) => string,
): ColumnDef<{}, T, any> => ({
  id: key,
  header: labelFor(key),
  accessorFn: read,
  meta: { className: "text-left" },
  cell: (context) => (
    <span className="font-mono">{read(context.row.original)}</span>
  ),
});
const number = <T extends RowData>(
  key: string,
  read: (row: T) => number,
  formatValue: Format,
): ColumnDef<{}, T, any> => ({
  id: key,
  header: labelFor(key),
  accessorFn: read,
  meta: { className: "text-right" },
  cell: (context) => (
    <Value value={read(context.row.original)} formatValue={formatValue} />
  ),
});
const money = <T extends RowData>(
  key: string,
  read: (row: T) => Money,
): ColumnDef<{}, T, any> => ({
  id: key,
  header: labelFor(key),
  accessorFn: (row) => read(row).amount,
  meta: { className: "text-right" },
  cell: (context) => <MoneyValue value={read(context.row.original)} />,
});
function aggregateColumns(
  formatValue: Format,
): ColumnDef<{}, Aggregate, any>[] {
  return [
    id<Aggregate>("instrument_id", (row) => row.instrument_id),
    id<Aggregate>("instrument_type", (row) => row.instrument_type),
    number<Aggregate>("net_quantity", (row) => row.net_quantity, formatValue),
    number<Aggregate>(
      "gross_quantity",
      (row) => row.gross_quantity,
      formatValue,
    ),
    money<Aggregate>("net_value", (row) => row.net_value),
    money<Aggregate>("gross_value", (row) => row.gross_value),
  ];
}
function riskColumns(formatValue: Format): ColumnDef<{}, RiskRow, any>[] {
  return [
    id<RiskRow>("instrument_id", (row) => row.instrument_id),
    id<RiskRow>("metric", (row) => row.metric),
    number<RiskRow>("net", (row) => row.net, formatValue),
    number<RiskRow>("gross", (row) => row.gross, formatValue),
  ];
}
function pathColumns(formatValue: Format): ColumnDef<{}, Path, any>[] {
  return [
    id<Path>("position_id", (row) => row.position_id),
    id<Path>("path", (row) => row.path.join(" › ")),
    id<Path>("instrument_id", (row) => row.instrument_id),
    number<Path>("quantity", (row) => row.quantity, formatValue),
    money<Path>("value", (row) => row.value),
  ];
}
/** Each primitive's returned net and gross value side by side; plotted, never stacked or summed. */
function valueChart(aggregates: readonly Aggregate[], currency: string) {
  const bars: ValueBar[] = aggregates.flatMap((row) => [
    {
      instrument_id: row.instrument_id,
      side: labelFor("net_value"),
      // Display conversion of the exact returned amount for plotting only.
      amount: Number(row.net_value.amount),
    },
    {
      instrument_id: row.instrument_id,
      side: labelFor("gross_value"),
      amount: Number(row.gross_value.amount),
    },
  ]);
  return defineChart({
    marks: [
      barY(bars, {
        x: (bar: ValueBar) => bar.instrument_id,
        y: (bar: ValueBar) => bar.amount,
        z: (bar: ValueBar) => bar.side,
        key: (bar: ValueBar) => `${bar.instrument_id}|${bar.side}`,
        layout: group(),
        id: "exposure-values",
      }),
    ],
    scales: {
      x: {
        scale: () =>
          scaleBand<string>()
            .domain(aggregates.map((row) => row.instrument_id))
            .padding(0.3),
        axis: { label: "Primitive (returned order)" },
      },
      y: {
        scale: scaleLinear,
        nice: true,
        axis: { label: `Value (${currency})` },
      },
    },
    color: { legend: colorLegend() },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.map((mark) => ({
          label: `${mark.datum.instrument_id} · ${mark.datum.side}`,
          value: String(mark.datum.amount),
        })),
      }),
    },
  });
}
function SensitivitySection({
  matrix,
  loading,
  error,
  formatValue,
  density,
}: {
  matrix: SensitivityMatrixJson | null | undefined;
  loading?: boolean;
  error?: string;
  formatValue: Format;
  density?: "compact" | "comfortable";
}) {
  // One column per returned factor id, index-aligned with each data row.
  const columns = useMemo<ColumnDef<{}, SensitivityRow, any>[]>(
    () =>
      matrix
        ? [
            id<SensitivityRow>("position_id", (row) => row.position_id),
            ...matrix.factor_ids.map(
              (factor, index): ColumnDef<{}, SensitivityRow, any> => ({
                id: `factor:${index}`,
                header: factor,
                accessorFn: (row) => row.values[index],
                meta: { className: "text-right" },
                cell: (context) => (
                  <Value
                    value={context.row.original.values[index]!}
                    formatValue={formatValue}
                  />
                ),
              }),
            ),
          ]
        : [],
    [matrix, formatValue],
  );
  return (
    <Section
      title="Factor sensitivities"
      description="Each position's sensitivity to each supplied factor's bump, in the base currency, as Rust returned it."
    >
      {error ? (
        <p role="alert" className="text-xs text-error">
          {error}
        </p>
      ) : !matrix ? (
        <p role="status" className="text-xs text-muted-foreground">
          {loading ? "Computing sensitivities…" : "No sensitivities"}
        </p>
      ) : (
        <FinstackTable
          caption={`Factor sensitivities (${matrix.base_currency})`}
          data={matrix.position_ids.map((position_id, index) => ({
            position_id,
            values: matrix.data[index] ?? [],
          }))}
          columns={columns}
          getRowId={(row) => row.position_id}
          emptyState="No sensitivities returned"
          density={density}
        />
      )}
    </Section>
  );
}

/**
 * Presents a book's native primitive exposures (portfolio.primitiveExposures)
 * as returned: each primitive's net and gross quantity and value with a chart
 * of the two values, its net and gross additive risk by metric, and the
 * position-aware paths; and, when supplied, the book's factor sensitivity
 * matrix (portfolio.computeFactorSensitivities). Nothing is summed, netted or
 * converted here: Rust returns both net and gross.
 */
export function ExposureReport({
  report,
  loading,
  error,
  sensitivities,
  sensitivitiesLoading,
  sensitivitiesError,
  formatValue = String,
  density,
}: ExposureReportProps) {
  const aggregates = useMemo(
    () => aggregateColumns(formatValue),
    [formatValue],
  );
  const risk = useMemo(() => riskColumns(formatValue), [formatValue]);
  const paths = useMemo(() => pathColumns(formatValue), [formatValue]);
  const view = useMemo(() => {
    if (!report) return null;
    // One row per returned metric key of each primitive, in returned order.
    const rows: RiskRow[] = report.aggregates.flatMap((row) =>
      Object.entries(row.net_measures).map(([metric, net]) => ({
        instrument_id: row.instrument_id,
        metric,
        net,
        gross: row.gross_measures[metric]!,
      })),
    );
    return {
      rows,
      chart: valueChart(report.aggregates, report.base_currency),
    };
  }, [report]);
  const showSensitivities =
    sensitivities != null || sensitivitiesLoading || sensitivitiesError;
  return (
    <section
      aria-label="Primitive exposures"
      data-density={density}
      className="@container min-w-0 max-w-full space-y-6 font-sans text-sm text-foreground"
    >
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !report || !view ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Decomposing exposures…" : "No exposure report"}
        </p>
      ) : (
        <>
          <Section
            title="Net and gross by primitive"
            description={`Each primitive's algebraic (net) and absolute (gross) quantity and value across every position and path, in ${report.base_currency}, ordered by primitive id as Rust returned them.`}
          >
            <FinstackTable
              caption="Net and gross by primitive"
              captionVisibility="sr-only"
              data={report.aggregates}
              columns={aggregates}
              getRowId={(row) => row.instrument_id}
              emptyState="No primitives returned"
              density={density}
            />
            {report.aggregates.length > 0 && (
              <figure className="min-w-0">
                <FinstackChart
                  definition={view.chart}
                  ariaLabel="Net and gross value by primitive"
                  height={240}
                />
              </figure>
            )}
          </Section>
          <Section
            title="Risk by primitive"
            description={`Each requested additive metric's net and gross amount per primitive, in ${report.base_currency}, as returned.`}
          >
            {view.rows.length > 0 ? (
              <FinstackTable
                caption="Risk by primitive"
                captionVisibility="sr-only"
                data={view.rows}
                columns={risk}
                getRowId={(row) => `${row.instrument_id}|${row.metric}`}
                emptyState="No risk returned"
                density={density}
              />
            ) : (
              <p role="status" className="text-xs text-muted-foreground">
                No risk metrics requested
              </p>
            )}
          </Section>
          <Section
            title="Primitive paths"
            description="Each position's path to its primitives before netting, with the position-scaled quantity and value."
          >
            <FinstackTable
              caption="Primitive paths"
              captionVisibility="sr-only"
              data={report.paths}
              columns={paths}
              getRowId={(row) => `${row.position_id}|${row.path.join("|")}`}
              emptyState="No paths returned"
              density={density}
            />
          </Section>
        </>
      )}
      {showSensitivities && (
        <SensitivitySection
          matrix={sensitivities}
          loading={sensitivitiesLoading}
          error={sensitivitiesError}
          formatValue={formatValue}
          density={density}
        />
      )}
    </section>
  );
}
