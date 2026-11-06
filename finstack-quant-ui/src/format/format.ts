import Big from "big.js";
import type { CoreNamespace, MoneyValue } from "finstack-quant-wasm";
import type { ValuationResultWire } from "../generated/types/valuation_result";

/** Returned Rust rounding policy; scales are read by currency, never inferred from names. */
export type RoundingStamp = Pick<
  ValuationResultWire["meta"]["rounding"],
  "mode" | "output_scale_by_currency"
>;
/** Existing native date operations, called in the worker or an initialized WASM host. */
export type DateApi = Pick<CoreNamespace, "createDate" | "dateFromEpochDays">;

/** Display a supplied scalar without assigning a financial unit or producing JSON. */
export function formatRaw(
  value: string | number | bigint | null | undefined,
): string {
  return value == null ? "—" : String(value);
}
/** Group exact decimal digits for the registry's en-US display; no numeric conversion. */
export function groupDecimal(value: string): string {
  // finstack-arith: expands exponent notation to digits; value unchanged
  const normalized = /[eE]/.test(value) ? new Big(value).toFixed() : value;
  const match = /^(-?)(\d+)(?:\.(\d+))?$/.exec(normalized);
  if (!match) throw new TypeError("Expected a decimal string");
  const [, sign, integer, fraction] = match;
  return (
    // finstack-arith: concatenates the sign and grouped digits as strings
    sign +
    new Intl.NumberFormat("en-US").format(BigInt(integer)) +
    (fraction === undefined ? "" : `.${fraction}`)
  );
}
/** Every digit of a native number; non-finite values keep their `NaN` / `Infinity` text. */
export function formatExact(value: number): string {
  return Number.isFinite(value) ? groupDecimal(String(value)) : String(value);
}
/** Native `core.Currency` / `core.Money` surface used for prepared display formatting; injected, never imported at render time. */
export type MoneyApi = Pick<CoreNamespace, "Currency" | "Money">;
/**
 * Display supplied money with its exact amount text; no rounding is applied at this boundary.
 * @param money - Exact major-unit amount string and supplied currency; neither is mutated.
 * @returns Currency-prefixed display text preserving every supplied digit, never a canonical JSON document.
 * @throws TypeError for malformed amount text.
 */
export function formatRawMoney(money: MoneyValue): string {
  return `${money.currency} ${groupDecimal(money.amount)}`;
}
/**
 * Format supplied money with its returned output scale and rounding mode through the native core.
 * @param money - Exact major-unit amount string and supplied currency; neither is mutated.
 * @param stamp - Returned Rust rounding snapshot. Without an explicit currency scale, all supplied digits are preserved — no ISO default is inferred at this boundary.
 * @param native - Initialized native `core` subset with the `Currency` and `Money` constructors; callers prepare text inside a worker or test host, never during renderer execution.
 * @returns Currency-prefixed display text, never a canonical JSON document.
 * @throws FinstackError for amount text that is not exactly representable as a Decimal or for invalid scale/rounding/grouping options.
 * @throws TypeError when a present stamp carries a non-number scale or non-string mode.
 */
export function formatMoney(
  money: MoneyValue,
  stamp: RoundingStamp | undefined,
  native: MoneyApi,
): string {
  const scale = stamp?.output_scale_by_currency[money.currency];
  if (scale === undefined) return formatRawMoney(money);
  if (typeof scale !== "number" || typeof stamp?.mode !== "string")
    throw new TypeError("Invalid returned rounding stamp");
  const currency = new native.Currency(money.currency);
  try {
    const value = native.Money.fromDecimalStr(money.amount, currency);
    try {
      return value.formatWith(scale, true, ",", stamp!.mode);
    } finally {
      value.free();
    }
  } finally {
    currency.free();
  }
}
/**
 * Display a supplied decimal fraction as percent digits.
 * @param value - A decimal fraction as Rust returned it, such as `0.25`.
 * @returns Exact percent digits with no sign appended, such as `25`.
 */
export function formatPercent(value: number): string {
  // finstack-arith: display scaling of a supplied fraction; never returned to Rust
  return new Big(value).times(100).toFixed();
}
/** Add a plus sign only when the caller explicitly requests a supplied signed value. */
export function formatSigned(value: string | number | bigint): string {
  const text = String(value);
  // finstack-arith: reads the sign of a supplied value to prefix a plus
  return new Big(text).gt(0) ? `+${text}` : text;
}
/**
 * Convert ISO date components with Rust's date constructor, without JS calendar arithmetic.
 * @param iso - ISO year-month-day text; Rust validates the calendar date and supported range.
 * @param dates - Initialized native core date functions; browser callers use these inside the worker.
 * @returns Native epoch-day integer, measured from 1970-01-01.
 * @throws TypeError for invalid text; native errors for invalid calendar dates.
 */
export function isoToEpoch(iso: string, dates: DateApi): number {
  const match = /^([+-]?\d{4,6})-(\d{2})-(\d{2})$/.exec(iso);
  if (!match) throw new TypeError("Expected an ISO calendar date");
  return dates.createDate(Number(match[1]), Number(match[2]), Number(match[3]));
}
/** Convert native epoch days to ISO text using Rust's calendar decomposition. */
export function epochToIso(days: number, dates: DateApi): string {
  if (!Number.isInteger(days) || days < -2147483648 || days > 2147483647)
    throw new TypeError("Expected an i32 epoch-day integer");
  const [year, month, day] = dates.dateFromEpochDays(days);
  const y =
    year < 0
      ? // finstack-arith: prints the magnitude of a negative ISO year
        `-${String(-year).padStart(4, "0")}`
      : String(year).padStart(4, "0");
  return `${y}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}
const labelCollator = new Intl.Collator("en-US", { numeric: true });
// Text runs ignore case so that case never outranks a later numeric value.
const runCollator = new Intl.Collator("en-US", { sensitivity: "base" });
/** A coordinate label with Rust's `MetricCoordinate`, which orders it on its axis. */
export interface AxisCoordinate {
  label: string;
  group: string;
  value?: number | null;
}
/**
 * Order coordinates for display by the values Rust supplied.
 * @param left - Coordinate label with its native group and value, as returned.
 * @param right - Coordinate compared with `left`.
 * @returns A negative, zero or positive number for `Array.prototype.sort`: by group
 * with case-insensitive en-US collation, then by value with a number before none,
 * then the label by en-US numeric collation and code-unit order, whatever the host
 * locale. `12m` and `1y` tie on value, so the label decides.
 */
export function compareCoordinates(
  left: AxisCoordinate,
  right: AxisCoordinate,
): number {
  const group = runCollator.compare(left.group, right.group);
  if (group) return group;
  const a = left.value ?? null;
  const b = right.value ?? null;
  if (a !== b) {
    if (a === null) return 1;
    if (b === null) return -1;
    return a < b ? -1 : 1;
  }
  return (
    labelCollator.compare(left.label, right.label) ||
    (left.label < right.label ? -1 : left.label > right.label ? 1 : 0)
  );
}
