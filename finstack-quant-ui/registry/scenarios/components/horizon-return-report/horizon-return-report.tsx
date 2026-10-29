"use client";
import { useState, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type {
  ApplicationReport,
  HorizonReport,
  MoneyValue as Money,
  PnlAttribution,
} from "finstack-quant-wasm";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { PnlAttributionReport } from "@/components/finstack/attribution/components/pnl-attribution-report/pnl-attribution-report";
import { MarketScenarioReport } from "@/components/finstack/scenarios/components/market-scenario-report/market-scenario-report";

export interface HorizonReturnReportProps {
  /** Native scenarios.computeHorizonReturn report, shown as returned. */
  report: HorizonReport | null | undefined;
  /** Rust explain text (scenarios.horizonResultExplainText); omitted shows none. */
  explain?: string;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}

type Contribution = { factor: string; value: number | null | undefined };

/** A returned decimal fraction as returned; Rust's null (undefined) is never shown as zero. */
function decimal(value: number | null | undefined) {
  return value == null ? "not defined" : formatRaw(value);
}

const contributionColumns: ColumnDef<{}, Contribution, any>[] = [
  {
    id: "factor",
    header: "Factor",
    accessorFn: (row) => labelFor(row.factor),
    meta: { className: "text-left" },
  },
  {
    id: "contribution",
    header: "Contribution (decimal of initial value)",
    accessorFn: (row) => decimal(row.value),
    meta: { className: "text-right finstack-numeric" },
  },
];

function Tile({ term, children }: { term: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-xs text-muted-foreground">{term}</dt>
      <dd className="finstack-numeric">{children}</dd>
    </div>
  );
}

/** The horizon's scenario report, rendered only while its disclosure is open. */
function ScenarioReportDisclosure({
  report,
  density,
}: {
  report: ApplicationReport;
  density?: "compact" | "comfortable";
}) {
  const [open, setOpen] = useState(false);
  return (
    <details
      className="space-y-2"
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary className="cursor-pointer text-sm font-medium">
        Scenario application report
      </summary>
      {open && <MarketScenarioReport report={report} density={density} />}
    </details>
  );
}

/**
 * Display of a native horizon total return: Rust's summary (total and
 * annualized return, factor contributions; an undefined value reads "not
 * defined", never zero), the horizon and endpoint values, the explain text,
 * the factor attribution and the scenario application report. Nothing is
 * computed here.
 */
export function HorizonReturnReport({
  report,
  explain,
  loading,
  error,
  density,
}: HorizonReturnReportProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (!report)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        {loading ? "Computing horizon return…" : "No horizon return"}
      </p>
    );
  const { summary } = report;
  const contributions: Contribution[] = Object.entries(
    summary.factor_contributions,
  ).map(([factor, value]) => ({ factor, value }));
  return (
    <section
      aria-label="Horizon return report"
      data-density={density}
      aria-busy={loading}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <dl className="grid grid-cols-2 gap-3 @2xl:grid-cols-5">
        <Tile term="Total return (decimal)">
          {decimal(summary.total_return)}
        </Tile>
        <Tile term="Annualized return (decimal)">
          {decimal(summary.annualized_return)}
        </Tile>
        <Tile term="Horizon (calendar days)">
          {report.horizon_days == null
            ? "no time roll"
            : String(report.horizon_days)}
        </Tile>
        <Tile term="Initial value">
          <MoneyValue
            value={report.initial_value as Money}
            label="Initial value"
          />
        </Tile>
        <Tile term="Terminal value">
          <MoneyValue
            value={report.terminal_value as Money}
            label="Terminal value"
          />
        </Tile>
      </dl>
      <section aria-label="Factor contributions" className="space-y-2">
        <h3 className="text-sm font-medium">Factor contributions</h3>
        <FinstackTable
          caption="Factor contributions"
          captionVisibility="sr-only"
          data={contributions}
          columns={contributionColumns}
          getRowId={(row) => row.factor}
          emptyState="No factor contributions returned"
          density={density}
        />
        <p className="text-xs text-muted-foreground">
          Each factor's P&L as a decimal fraction of the initial value, in{" "}
          {summary.currency}, in the order Rust returned them.
        </p>
      </section>
      {explain && (
        <section aria-label="Explain" className="space-y-2">
          <h3 className="text-sm font-medium">Explain</h3>
          <pre className="overflow-x-auto rounded-md bg-muted p-3 font-mono text-xs">
            {explain}
          </pre>
        </section>
      )}
      <section aria-label="Attribution" className="space-y-2">
        <h3 className="text-sm font-medium">Attribution</h3>
        <PnlAttributionReport
          attribution={report.attribution as PnlAttribution}
          density={density}
        />
      </section>
      <ScenarioReportDisclosure
        report={report.scenario_report as ApplicationReport}
        density={density}
      />
    </section>
  );
}
