"use client";
import { useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { ImResult, MoneyValue as Money } from "finstack-quant-wasm";
import { Badge } from "@/components/ui/badge";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { ReportSection } from "@/components/finstack/shared/primitives/native-display/native-display";

export interface SimmReportProps {
  /** The returned ImResult of SimmCalculator#calculateFromSensitivities; null while none is available. */
  result: ImResult | null;
  loading?: boolean;
  /** The native error message, shown instead of a report. */
  error?: string;
  /** Display text of a returned detail number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
type SimmComponent = NonNullable<ImResult["simm_detail"]>["components"][number];
type SimmBucket = SimmComponent["buckets"][number];
type SimmWeightedSensitivity = SimmBucket["weighted_sensitivities"][number];
function display(value: unknown, formatValue: (value: number) => string) {
  return typeof value === "number"
    ? formatValue(value)
    : formatRaw(value as string | null | undefined);
}
type BreakdownRow = { component: string; amount: Money };
const breakdownColumns: ColumnDef<{}, BreakdownRow, any>[] = [
  {
    id: "component",
    header: "Component",
    accessorFn: (row) => row.component,
    meta: { className: "text-left" },
  },
  {
    id: "amount",
    header: labelFor("amount"),
    accessorFn: (row) => row.amount.amount,
    cell: ({ row }) => <MoneyValue value={row.original.amount} />,
    meta: { className: "text-right" },
  },
];
const sensitivityFields = [
  "risk_factor",
  "tenor",
  "sensitivity",
  "risk_weight",
  "concentration_factor",
  "weighted_sensitivity",
] as const;
type SensitivityRow = SimmWeightedSensitivity & { index: number };
function sensitivityColumns(
  formatValue: (value: number) => string,
): ColumnDef<{}, SensitivityRow, any>[] {
  return sensitivityFields.map((field) => ({
    id: field,
    header: labelFor(field),
    accessorFn: (row: SensitivityRow) => row[field],
    cell: ({ getValue }: { getValue(): unknown }) =>
      display(getValue(), formatValue),
    meta: {
      className:
        field === "risk_factor" || field === "tenor"
          ? "text-left"
          : "text-right finstack-numeric",
    },
  }));
}
type MarginRow = { riskClass: string; margin: number };
/**
 * Presents a native SIMM initial-margin result: the headline amount with its
 * stamps, an always-shown indicative badge beside the returned approximation
 * flag, the component breakdown, and a drill-down into the returned
 * simm_detail (risk-class margins, then one component's buckets and weighted
 * sensitivities). Every value is shown as returned; nothing is aggregated.
 */
export function SimmReport({
  result,
  loading,
  error,
  ...props
}: SimmReportProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        Calculating SIMM initial margin…
      </p>
    );
  if (!result)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        No SIMM result
      </p>
    );
  return <ReportView result={result} {...props} />;
}
function ReportView({
  result,
  formatValue = String,
  density = "compact",
}: Omit<SimmReportProps, "result" | "loading" | "error"> & {
  result: ImResult;
}) {
  const detail = result.simm_detail ?? null;
  const [selected, setSelected] = useState(0);
  const component = detail?.components[selected];
  const breakdown = Object.entries(result.breakdown ?? {}).map(
    ([key, amount]) => ({ component: key, amount: amount as Money }),
  );
  return (
    <section
      aria-label="SIMM initial margin"
      data-density={density}
      className="space-y-6 font-sans text-sm text-foreground"
    >
      <div className="flex flex-wrap items-center gap-3">
        <MoneyValue value={result.amount} label="Initial margin" prominent />
        <Badge variant="secondary">Indicative</Badge>
        <span aria-label="Approximation" className="text-muted-foreground">
          {result.approximation
            ? "Approximation of ISDA SIMM"
            : "Not flagged as an approximation"}
        </span>
      </div>
      <p className="text-muted-foreground">
        {labelFor(result.methodology)} · as of {result.as_of} · MPOR{" "}
        {String(result.mpor_days)} business days
        {detail ? ` · MPOR scale ${formatValue(detail.mpor_scale)}` : ""}.
        Indicative only: not a reconciliation of a counterparty margin call.
      </p>
      <ReportSection title="Breakdown">
        <FinstackTable
          data={breakdown}
          columns={breakdownColumns}
          getRowId={(row) => row.component}
          caption="Initial margin by SIMM component"
          captionVisibility="sr-only"
          density={density}
          emptyState="No breakdown returned"
        />
      </ReportSection>
      {detail ? (
        <>
          <ReportSection title="Risk class margins">
            <FinstackTable
              data={Object.entries(detail.risk_class_margins).map(
                ([riskClass, margin]) => ({ riskClass, margin }),
              )}
              columns={[
                {
                  id: "riskClass",
                  header: "Risk class",
                  accessorFn: (row: MarginRow) => labelFor(row.riskClass),
                  meta: { className: "text-left" },
                },
                {
                  id: "margin",
                  header: "Margin",
                  accessorFn: (row: MarginRow) => row.margin,
                  cell: ({ getValue }: { getValue(): unknown }) =>
                    display(getValue(), formatValue),
                  meta: { className: "text-right finstack-numeric" },
                },
              ]}
              getRowId={(row) => row.riskClass}
              caption="SIMM margin by risk class"
              captionVisibility="sr-only"
              density={density}
            />
          </ReportSection>
          <ReportSection title="Component detail">
            <div className="min-w-48 max-w-sm">
              <EnumField
                label="Component"
                value={String(selected)}
                onValueChange={(value) => setSelected(Number(value))}
                options={detail.components.map((entry, index) => ({
                  value: String(index),
                  label: entry.component,
                }))}
                layout="select"
              />
            </div>
            {component ? (
              <div className="space-y-4">
                <p className="text-muted-foreground">
                  {labelFor(component.risk_class)} · margin{" "}
                  {formatValue(component.margin)} · historical volatility ratio{" "}
                  {formatValue(component.historical_volatility_ratio)}
                </p>
                {component.buckets.map((bucket: SimmBucket) => (
                  <section
                    key={bucket.bucket}
                    aria-label={`Bucket ${bucket.bucket}`}
                    className="space-y-1"
                  >
                    <h4 className="text-xs font-medium">
                      Bucket {bucket.bucket} · K {formatValue(bucket.k)} ·
                      signed sum {formatValue(bucket.signed_sum)}
                    </h4>
                    <FinstackTable
                      data={bucket.weighted_sensitivities.map((row, index) => ({
                        ...row,
                        index,
                      }))}
                      columns={sensitivityColumns(formatValue)}
                      getRowId={(row) => String(row.index)}
                      caption={`Weighted sensitivities of bucket ${bucket.bucket}`}
                      captionVisibility="sr-only"
                      density={density}
                    />
                  </section>
                ))}
              </div>
            ) : (
              <p role="status">No component detail returned</p>
            )}
          </ReportSection>
        </>
      ) : (
        <p role="status" className="text-muted-foreground">
          No SIMM detail returned
        </p>
      )}
    </section>
  );
}
