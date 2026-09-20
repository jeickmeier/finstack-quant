import { isLosslessNumber, parse } from "lossless-json";

const optionalNumbers = [
  "rate",
  "survival_probability",
  "conditional_default_prob",
  "inflation_index_ratio",
  "prepayment_smm",
  "beginning_balance",
  "ending_balance",
] as const;
/** Presentation-only adapter for CashflowRow in valuations/common_impl/cashflow_export.rs. */
export interface CashflowRow {
  date: string;
  amount: string;
  currency: string;
  kind: string;
  accrual_factor: string;
  year_fraction: string;
  discount_factor: string;
  discount_curve_id: string;
  pv: string;
  rate?: string;
  reset_date?: string;
  survival_probability?: string;
  conditional_default_prob?: string;
  inflation_index_ratio?: string;
  prepayment_smm?: string;
  beginning_balance?: string;
  ending_balance?: string;
}
export interface CashflowSchedule {
  instrument_id: string;
  currency: string;
  model: string;
  as_of: string;
  flows: CashflowRow[];
  total_pv: string;
  reconciles_with_base_value: boolean;
}
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new TypeError("Expected a cashflow object");
  return value as Record<string, unknown>;
}
function text(value: unknown, field: string): string {
  if (typeof value !== "string" || !value.trim())
    throw new TypeError(`Missing or invalid ${field}`);
  return value;
}
function number(value: unknown, field: string): string {
  if (!isLosslessNumber(value))
    throw new TypeError(`Missing or invalid ${field}`);
  return value.value;
}
/** Read native numeric tokens without Number conversion, rounding, or valuation calculations. */
export function readCashflows(json: string): CashflowSchedule {
  const envelope = record(parse(json));
  if (!Array.isArray(envelope.flows))
    throw new TypeError("Missing cashflow rows");
  if (typeof envelope.reconciles_with_base_value !== "boolean")
    throw new TypeError("Missing or invalid reconciliation status");
  return {
    instrument_id: text(envelope.instrument_id, "instrument_id"),
    currency: text(envelope.currency, "currency"),
    model: text(envelope.model, "model"),
    as_of: text(envelope.as_of, "as_of"),
    total_pv: number(envelope.total_pv, "total_pv"),
    reconciles_with_base_value: envelope.reconciles_with_base_value,
    flows: envelope.flows.map((value) => {
      const row = record(value);
      return {
        date: text(row.date, "date"),
        currency: text(row.currency, "currency"),
        kind: text(row.kind, "kind"),
        amount: number(row.amount, "amount"),
        accrual_factor: number(row.accrual_factor, "accrual_factor"),
        year_fraction: number(row.year_fraction, "year_fraction"),
        discount_factor: number(row.discount_factor, "discount_factor"),
        discount_curve_id: text(row.discount_curve_id, "discount_curve_id"),
        pv: number(row.pv, "pv"),
        ...(row.reset_date == null
          ? {}
          : { reset_date: text(row.reset_date, "reset_date") }),
        ...Object.fromEntries(
          optionalNumbers.flatMap((field) =>
            row[field] == null ? [] : [[field, number(row[field], field)]],
          ),
        ),
      };
    }),
  };
}
