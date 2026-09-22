import { isLosslessNumber, parse } from "lossless-json";
import { z } from "zod";
import schema from "@/lib/finstack/generated/schemas/instrument_cashflow.json";
import type { InstrumentCashflowWire } from "@/lib/finstack/generated/types/instrument_cashflow";
import { converterSchema } from "@/lib/finstack/schema.mjs";

const validator = z.fromJSONSchema(converterSchema(schema));
type Tokens<T> = T extends number
  ? string
  : T extends null
    ? undefined
    : T extends (infer V)[]
      ? Tokens<V>[]
      : T extends object
        ? { [K in keyof T]: Tokens<T[K]> }
        : T;
export type CashflowSchedule = Tokens<InstrumentCashflowWire>;
export type CashflowRow = CashflowSchedule["flows"][number];

function numbers(
  value: unknown,
  token: (text: string) => unknown,
  omitNull = false,
): unknown {
  if (value === null && omitNull) return undefined;
  if (isLosslessNumber(value)) return token(value.value);
  if (Array.isArray(value))
    return value.map((item) => numbers(item, token, omitNull));
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [
        key,
        numbers(item, token, omitNull),
      ]),
    );
  return value;
}
/** Validate the Rust schema on a numeric shadow; preserve original tokens for display. */
export function readCashflows(json: string): CashflowSchedule {
  const value = parse(json);
  validator.parse(numbers(value, Number));
  // Native optional numbers may be null or omitted. Both display as unavailable.
  return numbers(value, (text) => text, true) as CashflowSchedule;
}
