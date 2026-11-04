"use client";
import { useMemo, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barY, defineChart } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type {
  models as modelsWire,
  portfolio as portfolioWire,
} from "finstack-quant-wasm/types";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import {
  ReportSection,
  shownValue,
} from "@/components/finstack/shared/primitives/native-display/native-display";

/** A native result or the native error of that one call. */
type Outcome<T> =
  { ok: true; value: T } | { ok: false; error: { message: string } };
/** The native report the worker's positionRisk returns. */
export interface PositionRiskData {
  /** The returned PositionRiskDecomposition (losses negative). */
  decomposition: modelsWire.PositionRiskDecomposition;
  /** The returned RiskBudgetResult or its native error; null without a budget. */
  budget: Outcome<modelsWire.RiskBudgetResult> | null;
  /** The returned StressAttribution or its native error; null for a parametric source. */
  stress: Outcome<modelsWire.StressAttribution> | null;
}
/** A book's native factor sensitivities and their decomposition (the useFactorRisk result). */
export interface FactorRiskData {
  /** portfolio.computeFactorSensitivities: position-by-factor rows in the base currency. */
  sensitivities: portfolioWire.SensitivityMatrixJson;
  /** models.factor.risk.decomposeFactors of those sensitivities. */
  decomposition: modelsWire.RiskDecomposition;
}
export interface PortfolioRiskReportProps {
  /** The returned position report; null while none is available. */
  report: PositionRiskData | null;
  /** A book's returned factor sensitivities and decomposition, shown as the Factor contributions section. */
  factors?: FactorRiskData | null;
  /** The native error of the factor chain, shown in that section as returned. */
  factorsError?: string;
  loading?: boolean;
  /** The native error of the whole report, shown as returned. */
  error?: string;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
type Format = (value: number) => string;
/** One column per returned field, labelled from its Rust name; position ids stay text. */
function fieldColumns<Row extends object>(
  fields: readonly (keyof Row & string)[],
  format: Format,
): ColumnDef<{}, Row, any>[] {
  return fields.map((field) => ({
    id: field,
    header: labelFor(field),
    accessorFn: (row) => row[field],
    cell: ({ getValue }) =>
      typeof getValue() === "string"
        ? (getValue() as string)
        : shownValue(getValue(), format),
    meta: {
      className:
        typeof field === "string" && field.endsWith("_id")
          ? "text-left"
          : "text-right finstack-numeric",
    },
  }));
}
function Tile({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section
      aria-label={label}
      className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
    >
      <h4 className="text-xs font-medium text-muted-foreground">{label}</h4>
      <p className="finstack-numeric text-base">{children}</p>
    </section>
  );
}
function Failure({ outcome }: { outcome: { error: { message: string } } }) {
  return (
    <p role="alert" className="text-xs text-error">
      {outcome.error.message}
    </p>
  );
}
/** Bars of each returned component VaR, in returned order; nothing summed or stacked. */
function componentChart(rows: readonly modelsWire.PositionVarContribution[]) {
  return defineChart({
    marks: [
      barY(rows, {
        x: (row) => row.position_id,
        y: (row) => row.component_var,
        key: (row) => row.position_id,
        id: "component-var",
      }),
    ],
    scales: {
      x: {
        scale: () =>
          scaleBand<string>()
            .domain(rows.map((row) => row.position_id))
            .padding(0.3),
        axis: { label: "Position (returned order)" },
      },
      y: {
        scale: scaleLinear,
        nice: true,
        axis: { label: "Component VaR (losses negative)" },
      },
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.map((m) => ({
          label: m.datum.position_id,
          value: String(m.datum.component_var),
        })),
      }),
    },
  });
}
/**
 * Presents a native models.factor.risk position report: portfolio VaR and ES
 * with the Euler residual stamp, component-VaR bars, the VaR and ES
 * contribution tables, the risk budget (has_breach and per-position
 * utilization as returned) and, for a historical source, the tail stress
 * attribution. Values are shown as Rust returned them (losses negative, in the
 * units of the supplied weights or P&L); nothing is summed, flagged or
 * re-sorted here.
 */
export function PortfolioRiskReport({
  report,
  factors,
  factorsError,
  loading,
  error,
  formatValue = String,
  density = "compact",
}: PortfolioRiskReportProps) {
  const format = formatValue;
  const varColumns = useMemo(
    () =>
      fieldColumns<modelsWire.PositionVarContribution>(
        [
          "position_id",
          "component_var",
          "relative_var",
          "marginal_var",
          "incremental_var",
        ],
        format,
      ),
    [format],
  );
  const esColumns = useMemo(
    () =>
      fieldColumns<modelsWire.PositionEsContribution>(
        ["position_id", "component_es", "relative_es", "marginal_es"],
        format,
      ),
    [format],
  );
  const budgetColumns = useMemo(
    () =>
      fieldColumns<modelsWire.PositionBudgetEntry>(
        [
          "position_id",
          "actual_component_var",
          "target_component_var",
          "utilization",
          "excess",
        ],
        format,
      ),
    [format],
  );
  const stressColumns = useMemo(
    () =>
      fieldColumns<modelsWire.StressPositionEntry>(
        [
          "position_id",
          "avg_tail_pnl",
          "pct_of_tail_loss",
          "worst_scenario_pnl",
        ],
        format,
      ),
    [format],
  );
  const decomposition = report?.decomposition;
  const chart = useMemo(
    () =>
      decomposition ? componentChart(decomposition.var_contributions) : null,
    [decomposition],
  );
  if (error)
    return (
      <p role="alert" className="text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="text-muted-foreground">
        Calculating position risk…
      </p>
    );
  const factorSection =
    factors || factorsError ? (
      <FactorSection
        factors={factors ?? null}
        error={factorsError}
        format={format}
        density={density}
      />
    ) : null;
  if (!report || !decomposition)
    return factorSection ? (
      <section
        aria-label="Portfolio risk report"
        data-density={density}
        className="@container space-y-6 font-sans text-sm text-foreground"
      >
        {factorSection}
      </section>
    ) : (
      <p role="status" className="text-muted-foreground">
        No position risk report
      </p>
    );
  const { budget, stress } = report;
  return (
    <section
      aria-label="Portfolio risk report"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <div className="grid grid-cols-1 gap-2 @lg:grid-cols-3">
        <Tile label={labelFor("portfolio_var")}>
          {shownValue(decomposition.portfolio_var, format)}
        </Tile>
        <Tile label={labelFor("portfolio_es")}>
          {shownValue(decomposition.portfolio_es, format)}
        </Tile>
        <Tile label={labelFor("confidence")}>
          {shownValue(decomposition.confidence, format)}
        </Tile>
        <Tile label={labelFor("method")}>{decomposition.method}</Tile>
        <Tile label={labelFor("n_positions")}>
          {formatRaw(decomposition.n_positions)}
        </Tile>
        <Tile label={labelFor("euler_residual")}>
          {shownValue(decomposition.euler_residual, format)}
        </Tile>
      </div>
      <p className="text-xs text-muted-foreground">
        Values as Rust returned them: losses negative, in the units of the
        supplied weights or P&amp;L; relative shares as fractions. Rust reports
        no Euler residual for a historical decomposition.
      </p>
      <ReportSection title="Component VaR">
        {chart && (
          <figure className="min-w-0">
            <FinstackChart
              definition={chart}
              ariaLabel="Component VaR by position"
              height={220}
            />
          </figure>
        )}
        <FinstackTable
          data={decomposition.var_contributions}
          columns={varColumns}
          getRowId={(row) => row.position_id}
          caption="VaR contributions by position"
          captionVisibility="sr-only"
          density={density}
          emptyState="No VaR contributions returned"
        />
      </ReportSection>
      <ReportSection title="ES contributions">
        <FinstackTable
          data={decomposition.es_contributions}
          columns={esColumns}
          getRowId={(row) => row.position_id}
          caption="Expected shortfall contributions by position"
          captionVisibility="sr-only"
          density={density}
          emptyState="No ES contributions returned"
        />
      </ReportSection>
      {budget && (
        <ReportSection title="Risk budget">
          {budget.ok ? (
            <>
              <div className="grid grid-cols-1 gap-2 @lg:grid-cols-3">
                <Tile label={labelFor("has_breach")}>
                  {String(budget.value.has_breach)}
                </Tile>
                <Tile label={labelFor("total_overbudget")}>
                  {shownValue(budget.value.total_overbudget, format)}
                </Tile>
              </div>
              <FinstackTable
                data={budget.value.positions}
                columns={budgetColumns}
                getRowId={(row) => row.position_id}
                caption="Risk budget by position"
                captionVisibility="sr-only"
                density={density}
                emptyState="No budget rows returned"
              />
            </>
          ) : (
            <Failure outcome={budget} />
          )}
        </ReportSection>
      )}
      {stress && (
        <ReportSection title="Stress attribution">
          {stress.ok ? (
            <StressView
              stress={stress.value}
              columns={stressColumns}
              format={format}
              density={density}
            />
          ) : (
            <Failure outcome={stress} />
          )}
        </ReportSection>
      )}
      {factorSection}
    </section>
  );
}
function StressView({
  stress,
  columns,
  format,
  density,
}: {
  stress: modelsWire.StressAttribution;
  columns: ColumnDef<{}, modelsWire.StressPositionEntry, any>[];
  format: Format;
  density: "compact" | "comfortable";
}) {
  // One P&L column per returned position id, index-aligned as Rust stores them.
  const tailColumns = useMemo<
    ColumnDef<{}, modelsWire.TailScenarioBreakdown, any>[]
  >(
    () => [
      {
        id: "scenario_index",
        header: labelFor("scenario_index"),
        accessorFn: (row) => row.scenario_index,
        cell: ({ getValue }) => formatRaw(getValue() as number),
        meta: { className: "text-right finstack-numeric" },
      },
      {
        id: "portfolio_pnl",
        header: labelFor("portfolio_pnl"),
        accessorFn: (row) => row.portfolio_pnl,
        cell: ({ getValue }) => shownValue(getValue(), format),
        meta: { className: "text-right finstack-numeric" },
      },
      ...stress.position_ids.map(
        (id, index): ColumnDef<{}, modelsWire.TailScenarioBreakdown, any> => ({
          id: `position:${index}`,
          header: id,
          accessorFn: (row) => row.position_pnls[index],
          cell: ({ getValue }) => shownValue(getValue(), format),
          meta: { className: "text-right finstack-numeric" },
        }),
      ),
    ],
    [stress.position_ids, format],
  );
  return (
    <>
      <div className="grid grid-cols-1 gap-2 @lg:grid-cols-3">
        <Tile label={labelFor("var_threshold")}>
          {shownValue(stress.var_threshold, format)}
        </Tile>
        <Tile label={labelFor("n_tail_scenarios")}>
          {formatRaw(stress.n_tail_scenarios)}
        </Tile>
      </div>
      <FinstackTable
        data={stress.position_contributions}
        columns={columns}
        getRowId={(row) => row.position_id}
        caption="Tail contributions by position (largest driver first)"
        density={density}
        emptyState="No tail contributions returned"
      />
      <FinstackTable
        data={stress.tail_scenarios}
        columns={tailColumns}
        getRowId={(row) => String(row.scenario_index)}
        caption="Tail scenarios (worst first)"
        density={density}
        emptyState="No tail scenarios returned"
      />
    </>
  );
}
interface SensitivityRow {
  position_id: string;
  values: readonly number[];
}
/** The returned factor decomposition and the sensitivity matrix it decomposed, as returned. */
function FactorSection({
  factors,
  error,
  format,
  density,
}: {
  factors: FactorRiskData | null;
  error?: string;
  format: Format;
  density: "compact" | "comfortable";
}) {
  const factorColumns = useMemo(
    () =>
      fieldColumns<modelsWire.FactorContribution>(
        ["factor_id", "absolute_risk", "relative_risk", "marginal_risk"],
        format,
      ),
    [format],
  );
  const positionColumns = useMemo(
    () =>
      fieldColumns<modelsWire.PositionFactorContribution>(
        ["position_id", "factor_id", "risk_contribution"],
        format,
      ),
    [format],
  );
  const residualColumns = useMemo(
    () =>
      fieldColumns<modelsWire.PositionResidualContribution>(
        ["position_id", "residual_variance", "source"],
        format,
      ),
    [format],
  );
  const sensitivities = factors?.sensitivities;
  // One column per returned factor id, index-aligned with each data row.
  const sensitivityColumns = useMemo<ColumnDef<{}, SensitivityRow, any>[]>(
    () =>
      sensitivities
        ? [
            {
              id: "position_id",
              header: labelFor("position_id"),
              accessorFn: (row) => row.position_id,
              meta: { className: "text-left" },
            },
            ...sensitivities.factor_ids.map(
              (id, index): ColumnDef<{}, SensitivityRow, any> => ({
                id: `factor:${index}`,
                header: id,
                accessorFn: (row) => row.values[index],
                cell: ({ getValue }) => shownValue(getValue(), format),
                meta: { className: "text-right finstack-numeric" },
              }),
            ),
          ]
        : [],
    [sensitivities, format],
  );
  if (error || !factors)
    return (
      <ReportSection title="Factor contributions">
        <p role="alert" className="text-xs text-error">
          {error ?? "No factor decomposition"}
        </p>
      </ReportSection>
    );
  const { decomposition } = factors;
  const measure = decomposition.measure;
  return (
    <ReportSection title="Factor contributions">
      <div className="grid grid-cols-1 gap-2 @lg:grid-cols-3">
        <Tile label={labelFor("total_risk")}>
          {shownValue(decomposition.total_risk, format)}
        </Tile>
        <Tile label={labelFor("measure")}>
          {typeof measure === "string" ? measure : JSON.stringify(measure)}
        </Tile>
        <Tile label={labelFor("residual_risk")}>
          {shownValue(decomposition.residual_risk, format)}
        </Tile>
      </div>
      <FinstackTable
        data={decomposition.factor_contributions}
        columns={factorColumns}
        getRowId={(row) => row.factor_id}
        caption="Risk by factor"
        density={density}
        emptyState="No factor contributions returned"
      />
      <FinstackTable
        data={decomposition.position_factor_contributions}
        columns={positionColumns}
        getRowId={(row) => `${row.position_id}:${row.factor_id}`}
        caption="Risk by position and factor"
        density={density}
        emptyState="No position contributions returned"
      />
      {decomposition.position_residual_contributions.length > 0 && (
        <FinstackTable
          data={decomposition.position_residual_contributions}
          columns={residualColumns}
          getRowId={(row) => row.position_id}
          caption="Residual risk by position"
          density={density}
          emptyState="No residual contributions returned"
        />
      )}
      <FinstackTable
        data={factors.sensitivities.position_ids.map((position_id, index) => ({
          position_id,
          values: factors.sensitivities.data[index] ?? [],
        }))}
        columns={sensitivityColumns}
        getRowId={(row) => row.position_id}
        caption={`Factor sensitivities (${factors.sensitivities.base_currency})`}
        density={density}
        emptyState="No sensitivities returned"
      />
      <p className="text-xs text-muted-foreground">
        Values as Rust returned them under the returned measure (variance and
        volatility positive, VaR and ES losses negative); sensitivities in the
        book&apos;s base currency per factor bump.
      </p>
    </ReportSection>
  );
}
