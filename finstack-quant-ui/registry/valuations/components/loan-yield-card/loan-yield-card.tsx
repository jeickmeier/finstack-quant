"use client";
import {
  MetricValuesTable,
  type MetricValuesTableProps,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";

/** The ids the card shows, in display order: yields, spread, cost, the call and default risk. */
export const loanYieldMetrics = [
  "ytm",
  "ytw",
  "ytc",
  "yt2y",
  "yt3y",
  "yt4y",
  "discount_margin",
  "all_in_rate",
  "oid_eir_rate",
  "oid_eir_amortization",
  "embedded_option_value",
  "recovery01",
  "jump_to_default",
] as const;
export type LoanYieldCardProps = Omit<MetricValuesTableProps, "label" | "note">;
/**
 * Yields, discount margin, all-in rate, OID, the call value and the default risk
 * of a loan, as Rust returned them by metric id. An id Rust rejects shows its
 * native error in its own row and leaves the others. Nothing is classified,
 * summed or derived.
 */
export function LoanYieldCard(props: LoanYieldCardProps) {
  return (
    <MetricValuesTable
      {...props}
      label="Loan yields"
      note={
        <>
          Values as Rust returned them. Yields and discount margin solve to a
          quoted clean price when the instrument carries one, otherwise to the
          model price. The call value is callable less straight at one spread,
          so it is negative when the borrower&apos;s call costs the lender.
          Recovery01 and jump to default apply to a hazard-priced loan.
        </>
      }
    />
  );
}
