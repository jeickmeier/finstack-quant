"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barY, defineChart } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type {
  ApplicationReport,
  MoneyValue as Money,
  ScenarioPnlBatchItem,
} from "finstack-quant-wasm";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { MarketScenarioReport } from "@/components/finstack/scenarios/components/market-scenario-report/market-scenario-report";

/** The returned batch with the book's position order. */
export interface ScenarioPnlTableData {
  /** Position ids in spec order (Portfolio#positionIds); rows follow it. */
  positionIds: readonly string[];
  /** portfolio.scenarioPnlBatch items, in scenario order; columns follow it. */
  items: readonly ScenarioPnlBatchItem[];
}
export interface ScenarioPnlTableProps {
  /** The returned batch; null while none is available. */
  report: ScenarioPnlTableData | null | undefined;
  loading?: boolean;
  /** Native error (the earliest failing scenario's), shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}

type Row = { id: string; kind: "position" | "total"; cells: (Money | null)[] };

/** Position ids beyond the spec's (a scenario that adds positions), in returned key order. */
function extraIds(report: ScenarioPnlTableData) {
  const known = new Set(report.positionIds);
  const extra: string[] = [];
  for (const item of report.items)
    for (const id of Object.keys(item.pnl.by_position))
      if (!known.has(id)) {
        known.add(id);
        extra.push(id);
      }
  return extra;
}

/** Bars of each scenario's returned total; the amounts are plotted, never summed. */
function totalsChart(items: readonly ScenarioPnlBatchItem[]) {
  const currency = items[0]?.pnl.total.currency ?? "";
  return defineChart({
    marks: [
      barY(items, {
        x: (item) => item.scenario_id,
        y: (item) => Number(item.pnl.total.amount),
        key: (item) => item.scenario_id,
        id: "scenario-pnl-totals",
      }),
    ],
    scales: {
      x: {
        scale: () =>
          scaleBand<string>()
            .domain(items.map((item) => item.scenario_id))
            .padding(0.3),
        axis: { label: "Scenario (batch order)" },
      },
      y: {
        scale: scaleLinear,
        nice: true,
        axis: { label: `Portfolio P&L (${currency})` },
      },
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.map((m) => ({
          label: m.datum.scenario_id,
          value: `${m.datum.pnl.total.currency} ${m.datum.pnl.total.amount}`,
        })),
      }),
    },
  });
}

/** One scenario's application report, rendered only while its disclosure is open. */
function ReportDisclosure({
  item,
  density,
}: {
  item: ScenarioPnlBatchItem;
  density?: "compact" | "comfortable";
}) {
  const [open, setOpen] = useState(false);
  return (
    <details onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary className="cursor-pointer text-sm">
        <span className="font-mono">{item.scenario_id}</span> application report
      </summary>
      {open && (
        <div className="pt-2">
          <MarketScenarioReport
            report={item.report as ApplicationReport}
            density={density}
          />
        </div>
      )}
    </details>
  );
}

/**
 * Presents native portfolio scenario P&L: positions (spec order) by
 * scenarios (batch order) as returned by_position, a total row from each
 * scenario's returned `pnl.total`, a bar chart of those totals and each
 * scenario's application report. Nothing is summed or differenced.
 */
export function ScenarioPnlTable({
  report,
  loading,
  error,
  density,
}: ScenarioPnlTableProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (!report)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        {loading ? "Valuing scenarios…" : "No scenario P&L"}
      </p>
    );
  return <TableView report={report} density={density} />;
}

function TableView({
  report,
  density,
}: {
  report: ScenarioPnlTableData;
  density?: "compact" | "comfortable";
}) {
  const extra = useMemo(() => extraIds(report), [report]);
  const rows = useMemo((): Row[] => {
    const position = (id: string): Row => ({
      id,
      kind: "position",
      cells: report.items.map(
        (item) => (item.pnl.by_position[id] as Money | undefined) ?? null,
      ),
    });
    return [
      ...report.positionIds.map(position),
      ...extra.map(position),
      {
        id: "Total",
        kind: "total",
        cells: report.items.map((item) => item.pnl.total as Money),
      },
    ];
  }, [report, extra]);
  const columns = useMemo(
    (): ColumnDef<{}, Row, any>[] => [
      {
        id: "position",
        header: "Position",
        accessorFn: (row) => row.id,
        meta: { className: "text-left" },
        cell: (context) =>
          context.row.original.kind === "total" ? (
            <span className="font-medium">Total</span>
          ) : (
            <span className="font-mono">{context.row.original.id}</span>
          ),
      },
      ...report.items.map((item, index): ColumnDef<{}, Row, any> => ({
        id: `scenario-${index}`,
        header: item.scenario_id,
        accessorFn: (row) => row.cells[index]?.amount ?? null,
        meta: { className: "text-right finstack-numeric" },
        cell: (context) => (
          <MoneyValue value={context.row.original.cells[index]} />
        ),
      })),
    ],
    [report],
  );
  const chart = useMemo(() => totalsChart(report.items), [report]);
  return (
    <section
      aria-label="Portfolio scenario P&L"
      data-density={density}
      className="min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      <FinstackTable
        caption="P&L by position and scenario"
        captionVisibility="sr-only"
        data={rows}
        columns={columns}
        getRowId={(row) => `${row.kind}:${row.id}`}
        emptyState="No scenarios returned"
        density={density}
      />
      <p className="text-xs text-muted-foreground">
        Rows follow the book's position order and columns the batch order. The
        total row is each scenario's returned total in the base currency;
        nothing is summed here.
        {extra.length > 0 &&
          ` Positions not in the book (${extra.join(", ")}) appear as returned.`}
      </p>
      {report.items.length > 0 && (
        <figure className="min-w-0">
          <FinstackChart
            definition={chart}
            ariaLabel="Portfolio P&L by scenario"
            height={220}
          />
        </figure>
      )}
      <section aria-label="Scenario reports" className="space-y-2">
        <h3 className="text-sm font-medium">Scenario reports</h3>
        {report.items.map((item, index) => (
          <ReportDisclosure
            key={`${index}:${item.scenario_id}`}
            item={item}
            density={density}
          />
        ))}
      </section>
    </section>
  );
}
