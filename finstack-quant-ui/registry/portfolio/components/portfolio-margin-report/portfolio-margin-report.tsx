"use client";
import { useState, type ReactNode } from "react";
import type { ColumnDef, RowData } from "@tanstack/react-table";
import type {
  MoneyValue as Money,
  PortfolioMarginResult,
} from "finstack-quant-wasm";
import { Badge } from "@/components/ui/badge";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { SimmSensitivityTable } from "@/components/finstack/margin/components/simm-sensitivity-table/simm-sensitivity-table";

export interface PortfolioMarginReportProps {
  /** portfolio.portfolioMargin as returned; null while none is available. */
  result: PortfolioMarginResult | null;
  loading?: boolean;
  /** The native error message, shown instead of a report. */
  error?: string;
  density?: "compact" | "comfortable";
}
type NettingSet = PortfolioMarginResult["netting_sets"][number];
type ImCollateral = PortfolioMarginResult["by_csa"][string];
type SetRow = { key: string; set: NettingSet };
type CsaRow = { id: string; csa: ImCollateral };

/** A returned netting-set id as text: its identifiers as returned, the CCP marked for a cleared set. */
function nettingSetLabel(id: NettingSet["netting_set_id"]) {
  return id.kind === "bilateral"
    ? `${id.counterparty_id} · CSA ${id.csa_id}`
    : `CCP ${id.ccp_id}`;
}

const moneyColumn = <T extends RowData>(
  id: string,
  header: string,
  value: (row: T) => Money,
): ColumnDef<{}, T, any> => ({
  id,
  header,
  accessorFn: (row) => value(row).amount,
  cell: ({ row }) => <MoneyValue value={value(row.original)} />,
  meta: { className: "text-right finstack-numeric" },
});

const setColumns: ColumnDef<{}, SetRow, any>[] = [
  {
    id: "set",
    header: "Netting set",
    accessorFn: (row) => nettingSetLabel(row.set.netting_set_id),
    cell: ({ row }) => {
      const id = row.original.set.netting_set_id;
      return (
        <div className="flex flex-col">
          <span>
            {id.kind === "bilateral" ? id.counterparty_id : id.ccp_id}
          </span>
          <span className="text-xs text-muted-foreground">
            {id.kind === "bilateral"
              ? `bilateral · CSA ${id.csa_id}`
              : "cleared"}
          </span>
        </div>
      );
    },
    meta: { className: "text-left" },
  },
  {
    id: "methodology",
    header: "IM method",
    accessorFn: (row) => row.set.im_methodology,
    cell: ({ row }) => (
      <div className="flex flex-col items-start gap-1">
        <span>{row.original.set.im_methodology}</span>
        {row.original.set.is_approximate ? (
          <Badge variant="secondary">Approximate</Badge>
        ) : null}
      </div>
    ),
    meta: { className: "text-left" },
  },
  {
    id: "positions",
    header: "Positions",
    accessorFn: (row) => row.set.position_count,
    meta: { className: "text-right finstack-numeric" },
  },
  moneyColumn("im", "Initial margin", (row: SetRow) => row.set.initial_margin),
  moneyColumn(
    "vm",
    "Variation margin",
    (row: SetRow) => row.set.variation_margin,
  ),
  moneyColumn("total", "Total margin", (row: SetRow) => row.set.total_margin),
];

const csaColumns: ColumnDef<{}, CsaRow, any>[] = [
  {
    id: "csa",
    header: "CSA",
    accessorFn: (row) => row.id,
    meta: { className: "text-left font-mono" },
  },
  moneyColumn(
    "gross",
    "Gross IM",
    (row: CsaRow) => row.csa.gross_initial_margin,
  ),
  moneyColumn(
    "required",
    "Required collateral",
    (row: CsaRow) => row.csa.required_collateral,
  ),
  moneyColumn(
    "current",
    "Current collateral",
    (row: CsaRow) => row.csa.current_collateral,
  ),
  moneyColumn("transfer", "IM transfer", (row: CsaRow) => row.csa.transfer),
  {
    id: "segregated",
    header: "Segregated",
    accessorFn: (row) => (row.csa.segregated ? "yes" : "no"),
    meta: { className: "text-left" },
  },
];

const breakdownColumns: ColumnDef<
  {},
  { component: string; amount: Money },
  any
>[] = [
  {
    id: "component",
    header: "Component",
    accessorFn: (row) => row.component,
    meta: { className: "text-left font-mono" },
  },
  moneyColumn(
    "amount",
    "Initial margin",
    (row: { component: string; amount: Money }) => row.amount,
  ),
];

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="min-w-0 space-y-2">
      <h3 className="text-sm font-medium">{title}</h3>
      {children}
    </section>
  );
}

/**
 * Display of a native portfolio margin run: the returned totals and counts,
 * every netting set in returned order with its approximation flag, IM
 * collateral by CSA, degraded positions, and the selected set's IM breakdown
 * and SIMM sensitivities. Every amount is shown as returned; nothing is
 * summed, netted or converted.
 */
export function PortfolioMarginReport({
  result,
  loading,
  error,
  density = "compact",
}: PortfolioMarginReportProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (!result)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        {loading ? "Calculating portfolio margin…" : "No portfolio margin"}
      </p>
    );
  return <ReportView result={result} density={density} />;
}

function ReportView({
  result,
  density,
}: {
  result: PortfolioMarginResult;
  density: "compact" | "comfortable";
}) {
  const sets = result.netting_sets.map((set) => ({
    key: JSON.stringify(set.netting_set_id),
    set,
  }));
  const [selected, setSelected] = useState(sets[0]?.key);
  const set = sets.find((row) => row.key === selected)?.set;
  // CSA rows follow the netting sets' CSA order; a CSA no set names follows in returned order.
  const csaIds = [
    ...new Set(
      result.netting_sets.flatMap((entry) =>
        entry.csa_id != null && entry.csa_id in result.by_csa
          ? [entry.csa_id]
          : [],
      ),
    ),
  ];
  csaIds.push(
    ...Object.keys(result.by_csa).filter((id) => !csaIds.includes(id)),
  );
  const totals: [string, Money][] = [
    ["Total initial margin", result.total_initial_margin],
    ["Total variation margin", result.total_variation_margin],
    ["Total margin", result.total_margin],
    ["Required IM collateral", result.total_required_im_collateral],
    ["IM transfer", result.total_im_transfer],
    ["Segregated IM", result.total_segregated_im],
  ];
  return (
    <section
      aria-label="Portfolio margin"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <div className="space-y-2">
        <div className="flex flex-wrap items-center gap-3">
          <MoneyValue
            value={result.total_margin}
            label="Total margin"
            prominent
          />
          <Badge variant="secondary">Indicative</Badge>
        </div>
        <p className="text-muted-foreground">
          Initial margin per netting set under each set&apos;s methodology (SIMM
          at Rust&apos;s default version; cleared sets a standalone proxy),
          summed by Rust into {result.base_currency} as of {result.as_of}. Rust
          flags SIMM and cleared sets as approximate. Indicative only: not a
          reconciliation of a counterparty margin call.
        </p>
      </div>
      <dl
        aria-label="Totals"
        className="grid grid-cols-2 gap-x-6 gap-y-3 @2xl:grid-cols-4"
      >
        {totals.map(([label, money]) => (
          <div key={label} className="space-y-1">
            <dt className="text-xs text-muted-foreground">{label}</dt>
            <dd>
              <MoneyValue value={money} />
            </dd>
          </div>
        ))}
        <div className="space-y-1">
          <dt className="text-xs text-muted-foreground">
            Positions in netting sets
          </dt>
          <dd className="finstack-numeric">{result.total_positions}</dd>
        </div>
        <div className="space-y-1">
          <dt className="text-xs text-muted-foreground">
            Positions without margin
          </dt>
          <dd className="finstack-numeric">
            {result.positions_without_margin}
          </dd>
        </div>
      </dl>
      <Section title="Netting sets">
        <FinstackTable
          data={sets}
          columns={setColumns}
          getRowId={(row) => row.key}
          caption="Margin by netting set in returned order; activate a row for its detail"
          captionVisibility="sr-only"
          density={density}
          emptyState="No netting sets returned"
          onRowActivate={(row) => setSelected(row.key)}
        />
      </Section>
      <Section title="IM collateral by CSA">
        <FinstackTable
          data={csaIds.map((id) => ({ id, csa: result.by_csa[id]! }))}
          columns={csaColumns}
          getRowId={(row) => row.id}
          caption="One-way initial margin collateral by CSA"
          captionVisibility="sr-only"
          density={density}
          emptyState="No CSA collateral returned"
        />
      </Section>
      <Section title="Degraded positions">
        {result.degraded_positions.length ? (
          <ul className="space-y-1">
            {result.degraded_positions.map((position) => (
              <li key={position.position_id}>
                <span className="font-mono">{position.position_id}</span>
                <span className="text-muted-foreground">
                  {" "}
                  · {position.message}
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-muted-foreground">No position was degraded.</p>
        )}
      </Section>
      {set && (
        <section
          aria-label={`Netting set ${nettingSetLabel(set.netting_set_id)}`}
          className="space-y-4"
        >
          <h3 className="text-sm font-medium">
            Netting set {nettingSetLabel(set.netting_set_id)}
          </h3>
          <Section title="IM breakdown">
            <FinstackTable
              data={Object.entries(set.im_breakdown).map(
                ([component, amount]) => ({ component, amount }),
              )}
              columns={breakdownColumns}
              getRowId={(row) => row.component}
              caption="Initial margin by component as returned"
              captionVisibility="sr-only"
              density={density}
              emptyState="No IM breakdown returned"
            />
          </Section>
          {/* simm-sensitivity-table is itself the "SIMM sensitivities" region. */}
          <div className="min-w-0 space-y-2">
            <h3 className="text-sm font-medium">SIMM sensitivities</h3>
            {set.sensitivities ? (
              <SimmSensitivityTable
                sensitivities={set.sensitivities}
                density={density}
              />
            ) : (
              <p className="text-muted-foreground">
                No SIMM sensitivities returned for this netting set.
              </p>
            )}
          </div>
        </section>
      )}
    </section>
  );
}
