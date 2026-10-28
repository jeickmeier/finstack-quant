import Big from "big.js";
import type { CoreNamespace, MoneyValue } from "finstack-quant-wasm";
import type { ValuationResultWire } from "../generated/types/valuation_result";

/** Returned Rust rounding policy; scales are read by currency, never inferred from names. */
export type RoundingStamp = Pick<
  ValuationResultWire["meta"]["rounding"],
  "mode" | "output_scale_by_currency"
>;
/** Presentation conventions explicitly documented by core.Rate; no field classification. */
export type RateUnit = "decimal" | "percent" | "bp";
/** A caller's source-backed choice of wire and display conventions. */
export interface RatePresentation {
  source: string;
  wire: RateUnit;
  display: RateUnit;
}
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
  const normalized = /[eE]/.test(value) ? new Big(value).toFixed() : value;
  const match = /^(-?)(\d+)(?:\.(\d+))?$/.exec(normalized);
  if (!match) throw new TypeError("Expected a decimal string");
  const [, sign, integer, fraction] = match;
  return (
    sign +
    new Intl.NumberFormat("en-US").format(BigInt(integer)) +
    (fraction === undefined ? "" : `.${fraction}`)
  );
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
 * Convert presentation units exactly using the published core.Rate convention.
 * @param value - Decimal text; fractional precision never passes through Number.
 * @param from - Explicit source unit: decimal, percent or basis points.
 * @param to - Explicit destination display unit.
 * @returns Exact decimal text; equivalent units return the original spelling.
 */
export function convertRate(
  value: string,
  from: RateUnit,
  to: RateUnit,
): string {
  if (from === to) return value;
  // core.Rate documents decimal=1, percent=100, bp=10000. This is display scaling only.
  const powers = { decimal: 0, percent: 2, bp: 4 };
  if (!Object.hasOwn(powers, from) || !Object.hasOwn(powers, to))
    throw new TypeError("Unsupported rate presentation unit");
  const shift = powers[to] - powers[from];
  const fraction = /^-?\d+(?:\.(\d+))?(?:[eE]([+-]?\d+))?$/.exec(value);
  if (!fraction)
    throw new TypeError("Expected decimal text for rate presentation");
  return new Big(value)
    .times(`1e${shift}`)
    .toFixed(
      Math.max(
        0,
        (fraction[1]?.length ?? 0) - Number(fraction[2] ?? 0) - shift,
      ),
    );
}
/** Display an explicitly contracted rate, or preserve raw text with unavailable units. */
export function formatRate(
  value: string | number,
  presentation?: RatePresentation,
): { text: string; unit: RateUnit | null } {
  if (!presentation) return { text: String(value), unit: null };
  if (!presentation.source.trim())
    throw new TypeError("Rate presentation requires its canonical source");
  return {
    text: convertRate(
      typeof value === "number" ? new Big(value).toFixed() : value,
      presentation.wire,
      presentation.display,
    ),
    unit: presentation.display,
  };
}
/** Adapt edited display text back to wire units; missing metadata leaves it untouched. */
export function rateToWire(
  value: string,
  presentation?: RatePresentation,
): string {
  if (!presentation) return value;
  if (!presentation.source.trim())
    throw new TypeError("Rate presentation requires its canonical source");
  return convertRate(value, presentation.display, presentation.wire);
}
/** Add a plus sign only when the caller explicitly requests a supplied signed value. */
export function formatSigned(value: string | number | bigint): string {
  const text = String(value);
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
      ? `-${String(-year).padStart(4, "0")}`
      : String(year).padStart(4, "0");
  return `${y}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}
const pureTenor = /^\d+(?:\.\d+)?[my]$/;
const labelToken = /(^-)?(\d+(?:\.\d+)?)([my](?![A-Za-z]))?|\D+/g;
const labelCollator = new Intl.Collator("en-US", { numeric: true });
// Text runs ignore case so that case never outranks a later numeric run.
const runCollator = new Intl.Collator("en-US", { sensitivity: "base" });
/**
 * Split a label into number and text runs. A `-` at the very start of the label signs
 * its first number; any other `-` is text.
 * A number followed by `m` or `y` and then no letter is a tenor in years: `Ny` is N and
 * `Nm` is N/12, as parse_key_rate_label reads them and as vega writes `{expiry}y` rows.
 */
function labelTokens(label: string): (number | string)[] {
  return [...label.matchAll(labelToken)].map(([text, sign, digits, unit]) =>
    digits === undefined
      ? text
      : (sign ? -1 : 1) * (Number(digits) / (unit === "m" ? 12 : 1)),
  );
}
/**
 * Order bucket and coordinate labels by maturity for display.
 * @param left - Label such as `3m`, `10y`, `SURF::0.25y` (a vega row), a curve id or a
 * strike such as `1.05`, exactly as returned.
 * @param right - Label compared with `left`.
 * @returns A negative, zero or positive number for `Array.prototype.sort`. Whole-label
 * `Nm` and `Ny` tenors come first. Labels then compare run by run: numbers by value
 * (a tenor run in years, months being N/12, so `12m` and `1y` tie), text with
 * case-insensitive en-US collation, a number before text, and a label whose runs end
 * first sorts first. Ties
 * fall back to en-US numeric collation of the full label, then code-unit order,
 * whatever the host locale.
 */
export function compareTenorLabels(left: string, right: string): number {
  const leftTenor = pureTenor.test(left);
  if (leftTenor !== pureTenor.test(right)) return leftTenor ? -1 : 1;
  const leftTokens = labelTokens(left);
  const rightTokens = labelTokens(right);
  for (let index = 0; index < leftTokens.length; index++) {
    const a = leftTokens[index];
    const b = rightTokens[index];
    if (b === undefined) return 1;
    const order =
      typeof a === "number"
        ? typeof b === "number"
          ? a - b
          : -1
        : typeof b === "number"
          ? 1
          : runCollator.compare(a, b);
    if (order) return order;
  }
  if (rightTokens.length > leftTokens.length) return -1;
  return (
    labelCollator.compare(left, right) ||
    (left < right ? -1 : left > right ? 1 : 0)
  );
}
