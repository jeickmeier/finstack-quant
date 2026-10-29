import { z } from "zod";
import schema from "@/lib/finstack/generated/schemas/instrument_cashflow.json";
import type { InstrumentCashflowWire } from "@/lib/finstack/generated/types/instrument_cashflow";
import { converterSchema } from "@/lib/finstack/schema.mjs";

const validator = z.fromJSONSchema(converterSchema(schema));
export type CashflowSchedule = InstrumentCashflowWire;
export type CashflowRow = CashflowSchedule["flows"][number];
/** Validate a supplied native envelope against the Rust schema; the object is returned unchanged. */
export function readCashflows(cashflows: CashflowSchedule): CashflowSchedule {
  validator.parse(cashflows);
  return cashflows;
}
