// Type declarations for the finstack-quant-wasm namespaced facade.
// Shapes follow `wasm-bindgen` JS names in `src/api/**` (see Rust `js_name`).
// The raw `pkg/finstack_quant_wasm.d.ts` emitted by wasm-bindgen is intentionally
// not the package root contract: it exposes a flat module, while `index.js`
// publishes a namespaced facade. Keep this file as the facade declaration and
// use generated `types/generated/*` files only for JSON envelope shapes.
//
// Building a MarketContext from quotes (canonical path):
//
//   import { valuations } from 'finstack-quant-wasm/exports/valuations.js';
//   import type { CalibrationEnvelope } from 'finstack-quant-wasm';
//   const envelope: CalibrationEnvelope = {
//     schema: 'finstack_quant.calibration',
//     plan: { id: 'usd_curves', quote_sets: {...}, steps: [...], settings: {} },
//     market_data: [],   // flat id-addressable quotes/snapshots
//     prior_market: [],  // optional pre-built curves/surfaces
//   };
//   const result = valuations.calibrate(envelope);  // CalibrationResultEnvelope
//   const marketJson = JSON.stringify(result.result.final_market);
//
// `result.result.final_market` is the materialized MarketContextState ready
// for any downstream pricing / scenario / attribution call that takes a
// market_json argument. Always check the per-step report
// (`result.result.step_reports`) and the plan summary
// (`result.result.report`) to confirm the curves actually fit before using
// the market downstream.
//
// `validateCalibrationJson` is a fast pre-flight check that canonicalizes
// the envelope without solving — use it to surface schema errors early.
//
// Phase 4 diagnostics: errors thrown by `calibrate`,
// `validateCalibrationJson`, `dryRun`, and `dependencyGraphJson` have:
//   - name: 'CalibrationEnvelopeError'
//   - cause: structured EnvelopeError payload (object with `kind` etc.)
// Standard try/catch exposes both via `e.name` and `e.cause`.

export type {
  AccrualConfigJson,
  CashFlowJson,
  CashFlowMetaJson,
  CashFlowScheduleJson,
  DatedFlowJson,
  MoneyJson,
  NotionalJson,
} from "./types/generated/CashflowSchedule";
//
// WASM ownership: every wasm-bindgen class exposed below owns a wasm heap
// allocation. Call `free()` when a handle is no longer needed. On runtimes
// that define `Symbol.dispose`, wasm-bindgen also installs
// `instance[Symbol.dispose] === instance.free`. Plain JSON results, arrays,
// and namespace functions do not need manual disposal.

/**
 * Inputs accepted by the wasm-bindgen web initializer.
 */
export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

/**
 * Initialized WebAssembly exports.
 */
export type InitOutput = WebAssembly.Exports;

/**
 * Initialize the package's WebAssembly module.
 * @example
 * ```typescript
 * import init from "finstack-quant-wasm";
 * const wasm = await init();
 * void wasm;
 * ```
 * @param moduleOrPath - Optional module source: a URL, Response, WebAssembly.Module, or Promise accepted by wasm-bindgen initialization.
 * @returns Returns a Promise that resolves to `InitOutput`.
 * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
 */
export default function init(
  moduleOrPath?:
    | { module_or_path: InitInput | Promise<InitInput> }
    | InitInput
    | Promise<InitInput>
): Promise<InitOutput>;

// --- Calibration envelope types (generated from Rust via ts-rs) ---
import type { CalibrationEnvelope } from './types/generated/CalibrationEnvelope';
import type { CalibrationResultEnvelope } from './types/generated/CalibrationResultEnvelope';

export type { CalibrationEnvelope, CalibrationResultEnvelope };
export type { CalibrationPlan } from './types/generated/CalibrationPlan';
export type { CalibrationStep } from './types/generated/CalibrationStep';
export type { StepParams } from './types/generated/StepParams';
export type { MarketDatum } from './types/generated/MarketDatum';
export type { PriorMarketObject } from './types/generated/PriorMarketObject';
export type { CalibrationResult } from './types/generated/CalibrationResult';
export type { CalibrationReport } from './types/generated/CalibrationReport';

// --- core -----------------------------------------------------------------

/**
 * Lifecycle contract for a WebAssembly-backed value that owns a wasm heap allocation.
 */
export interface WasmOwned {
  /**
   * Release the underlying wasm heap allocation. Do not use this handle afterward.
   */
  free(): void;
}

// wasm-bindgen emits these as classes. Interface merging adds their generated
// `free()` contract without duplicating methods. At runtime, wasm-bindgen also
// installs `[Symbol.dispose]` as an alias of `free` when the host defines that
// symbol; it is intentionally omitted here so ES2020 consumers do not require
// the `esnext.disposable` TypeScript library.
/**
 * Stateful performance analytics engine over a panel of ticker price (or return) series.
 *
 * Dates are ISO-8601 values in ascending order. Numeric inputs are row-major
 * with one row per date and one column per ticker. Scalar rates and returns
 * use decimal fractions; numeric outputs are Float64Array values in ticker
 * order unless the method documents an object or matrix shape.
 *
 * Invalid dates, shapes, frequencies, tickers, and confidence levels are
 * returned as rejected JsValue errors.
 */
export interface Performance extends WasmOwned {}
/**
 * Calibrated credit factor hierarchy artifact.
 *
 * Produced by [`CreditCalibrator`] or loaded from JSON via
 * [`CreditFactorModel.fromJson`]. Immutable once constructed.
 */
export interface CreditFactorModel extends WasmOwned {}
/**
 * Deterministic calibrator that produces a [`CreditFactorModel`].
 *
 * Configuration and inputs are passed as JSON strings.
 */
export interface CreditCalibrator extends WasmOwned {}
/**
 * Snapshot of all hierarchy-level factor values at a single date.
 *
 * Produced by [`decompose_levels`]. Pass to [`decompose_period`] to compute
 * period-over-period changes.  The full data is available via `toJson`.
 */
export interface LevelsAtDate extends WasmOwned {}
/**
 * Component-wise difference between two [`LevelsAtDate`] snapshots.
 *
 * Produced by [`decompose_period`].
 */
export interface PeriodDecomposition extends WasmOwned {}
/**
 * Vol-forecast view over a calibrated `CreditFactorModel`.
 *
 * `VolHorizon::Custom` is intentionally **not** exposed.
 */
export interface FactorCovarianceForecast extends WasmOwned {}
/**
 * Opaque handle wrapping a parsed [`MarketContext`].
 *
 * Construct once from JSON, then pass to `priceInstrumentWithMarket`,
 * `priceInstrumentWithMetricsAndMarket`, etc.  Eliminates the per-call
 * market-parse overhead in bulk-pricing and Greeks-sweep loops.
 *
 * @example
 * ```javascript
 * const market = new valuations.Market(marketJson);
 * for (const instr of instruments) {
 *   const result = valuations.instruments.priceInstrumentWithMarket(instr, market, "2025-06-15", "default");
 * }
 * ```
 */
export interface Market extends WasmOwned {}
/**
 * Handle to a built [`finstack_quant_portfolio::Portfolio`] that can be reused
 * across WASM calls without re-parsing and rebuilding from the spec.
 *
 * `Portfolio::from_spec` parses positions, builds indices, and validates
 * invariants; for pipelines that call both `valuePortfolio` and
 * `aggregateFullCashflows` on the same portfolio, holding this handle
 * avoids paying that cost twice.
 */
export interface Portfolio extends WasmOwned {}

/**
 * ISO-4217 currency code wrapper for JavaScript.
 *
 * Currencies parse from three-letter alphabetic codes (case-insensitive).
 * They expose the alphabetic code, the ISO numeric code, and the number of
 * decimal places (minor units) for the currency.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * usd.code;     // "USD"
 * usd.numeric;  // 840
 * usd.decimals; // 2
 * ```
 */
export interface Currency extends WasmOwned {
  /**
   * Three-letter ISO-4217 alphabetic code.
   *
   * @returns The uppercase alphabetic code (e.g. `"USD"`).
   */
  readonly code: string;
  /**
   * ISO-4217 numeric code.
   *
   * @returns Numeric code (e.g. `840` for USD, `978` for EUR).
   */
  readonly numeric: number;
  /**
   * Number of decimal places (minor units) for this currency.
   *
   * @returns Decimal-place count (e.g. `2` for USD, `0` for JPY).
   */
  readonly decimals: number;
  /**
   * Human-readable code (same as `code`).
   *
   * @returns The uppercase alphabetic ISO-4217 code.
   */
  toString(): string;
  /**
   * Serialize to a JSON string.
   *
   * @returns A JSON string (the ISO-4217 alphabetic code in quotes).
   * @throws If serialization fails (should not happen for valid `Currency`).
   */
  toJson(): string;
}

/**
 * ISO-4217 currency code wrapper for JavaScript.
 *
 * Currencies parse from three-letter alphabetic codes (case-insensitive).
 * They expose the alphabetic code, the ISO numeric code, and the number of
 * decimal places (minor units) for the currency.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * usd.code;     // "USD"
 * usd.numeric;  // 840
 * usd.decimals; // 2
 * ```
 */
export interface CurrencyConstructor {
  /**
   * Parse a case-insensitive ISO-4217 alphabetic currency code.
   *
   * @param code - Three-letter ISO-4217 code (e.g. `"USD"`, `"eur"`,
   * `"GBP"`). Leading and trailing whitespace is trimmed.
   * @returns Constructed `Currency`.
   * @throws If `code` is not a recognized ISO-4217 alphabetic code.
   *
   * @example
   * ```javascript
   * const eur = new core.Currency("eur"); // case-insensitive
   * eur.code; // "EUR"
   * ```
   */
  new (code: string): Currency;
  /**
   * Deserialize from a JSON string produced by `Currency.toJson`.
   *
   * @param json - A JSON string containing a quoted ISO-4217 code.
   * @returns The parsed `Currency`.
   * @throws If `json` is malformed or contains an unknown code.
   */
  fromJson(json: string): Currency;
}

/**
 * Currency-tagged monetary amount.
 *
 * Money values pin a numeric amount to a [`Currency`]. Arithmetic
 * (`add`, `sub`) refuses to mix currencies; scalar multiplication and
 * division preserve the currency.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * const total = new core.Money(1_000_000, usd);
 * const fee   = new core.Money(50, usd);
 * const net   = total.sub(fee);                 // Money { amount: 999950, currency: USD }
 * const tax   = net.mulScalar(0.07);            // 7% of net
 * console.log(net.toString(), tax.toString());  // "USD 999950.00", "USD 69996.50"
 * ```
 */
export interface Money extends WasmOwned {
  /**
   * Numeric amount in major units as `f64`.
   *
   * The Rust core stores money as `Decimal`; this getter exposes the finite
   * JavaScript number view for interop.
   *
   * @returns Amount in major units (e.g. dollars, not cents).
   */
  readonly amount: number;
  /**
   * Currency of this amount.
   *
   * @returns The [`Currency`] this amount is tagged with.
   */
  readonly currency: Currency;
  /**
   * Lossless amount as a decimal string (e.g. `"1234.56"`).
   *
   * Renders the internal Rust `Decimal` directly, so no `f64` round-trip
   * occurs. Parse with a JavaScript decimal library for exact arithmetic.
   *
   * @returns The exact decimal amount as a string.
   */
  amountDecimal(): string;
  /**
   * Convert using an already-resolved positive FX rate.
   * @param target - Target Currency for the converted monetary amount.
   * @param rate - FX conversion rate expressed as target-currency units per source-currency unit.
   * @returns Returns the resulting `Money` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  convertAtRate(target: Currency, rate: number): Money;
  /**
   * Add two amounts.
   *
   * @param other - Another `Money` value.
   * @returns Sum, in the same currency.
   * @throws If `other.currency` differs from `this.currency`, or the
   * operation is not representable as a `Decimal`.
   *
   * @example
   * ```javascript
   * const usd = new core.Currency("USD");
   * const a = new core.Money(10, usd);
   * const b = new core.Money(5, usd);
   * a.add(b).amount;  // 15
   * ```
   */
  add(other: Money): Money;
  /**
   * Subtract two amounts.
   *
   * @param other - Another `Money` value.
   * @returns Difference, in the same currency.
   * @throws If `other.currency` differs from `this.currency`, or the
   * operation is not representable as a `Decimal`.
   */
  sub(other: Money): Money;
  /**
   * Multiply by a scalar.
   *
   * @param factor - Dimensionless multiplier (must be finite).
   * @returns Scaled amount, in the same currency.
   * @throws If `factor` is non-finite or the result is not representable.
   */
  mulScalar(factor: number): Money;
  /**
   * Divide by a scalar.
   *
   * @param divisor - Dimensionless divisor (must be finite and non-zero).
   * @returns Scaled amount, in the same currency.
   * @throws If `divisor` is zero, non-finite, or the result is not representable.
   */
  divScalar(divisor: number): Money;
  /**
   * Negate the monetary amount.
   *
   * @returns Negated amount in the same currency.
   * @throws If the negation is not representable as a `Decimal`.
   */
  negate(): Money;
  /**
   * Default string representation (e.g. `"USD 10.00"`).
   *
   * @returns Formatted amount with currency code.
   */
  toString(): string;
}

/**
 * Currency-tagged monetary amount.
 *
 * Money values pin a numeric amount to a [`Currency`]. Arithmetic
 * (`add`, `sub`) refuses to mix currencies; scalar multiplication and
 * division preserve the currency.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * const total = new core.Money(1_000_000, usd);
 * const fee   = new core.Money(50, usd);
 * const net   = total.sub(fee);                 // Money { amount: 999950, currency: USD }
 * const tax   = net.mulScalar(0.07);            // 7% of net
 * console.log(net.toString(), tax.toString());  // "USD 999950.00", "USD 69996.50"
 * ```
 */
export interface MoneyConstructor {
  /**
   * Creates a new money value without implicit currency-minor-unit rounding.
   *
   * WASM accepts a JavaScript `number` only. Its finite numeric value is
   * converted to Rust `Decimal` and stored without currency-minor-unit
   * rounding; precision already absent from the input `number` cannot be
   * recovered. Formatting does not mutate the stored amount.
   *
   * @param amount - Numeric amount in major units (must be finite).
   * @param currency - ISO-4217 Currency object that tags the amount and controls arithmetic compatibility.
   * @returns The constructed `Money`.
   * @throws If `amount` is non-finite (NaN, ±∞) or cannot be represented as a `Decimal`.
   *
   * @example
   * ```javascript
   * const usd = new core.Currency("USD");
   * const m = new core.Money(1234.56, usd);
   * m.amount;          // 1234.56
   * m.currency.code;   // "USD"
   * ```
   */
  new (amount: number, currency: Currency): Money;
}

/**
 * Interest or discount rate stored as a decimal (e.g. `0.05` is 5%).
 *
 * Conventions:
 * - **Decimal**: `0.05` represents 5%.
 * - **Percent**: `5.0` represents 5%.
 * - **Basis points**: `500` represents 5% (1 bp = 0.01%).
 *
 * Use the `fromPercent` or `fromBps` factories to avoid scaling errors
 * when working with quoted rates.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const r = core.Rate.fromBps(250);     // 2.5% as 250 bps
 * r.asDecimal;  // 0.025
 * r.asPercent;  // 2.5
 * r.asBps;      // 250
 * ```
 */
export interface Rate extends WasmOwned {
  /**
   * Rate as a decimal (e.g. `0.05` for 5%).
   *
   * @returns Decimal rate.
   */
  readonly asDecimal: number;
  /**
   * Rate as a percent (e.g. `5.0` for 5%).
   *
   * @returns Percent rate.
   */
  readonly asPercent: number;
  /**
   * Rate in basis points, rounded to the nearest integer (e.g. `500` for 5%).
   *
   * @returns Rate in bps.
   */
  readonly asBps: number;
}

/**
 * Interest or discount rate stored as a decimal (e.g. `0.05` is 5%).
 *
 * Conventions:
 * - **Decimal**: `0.05` represents 5%.
 * - **Percent**: `5.0` represents 5%.
 * - **Basis points**: `500` represents 5% (1 bp = 0.01%).
 *
 * Use the `fromPercent` or `fromBps` factories to avoid scaling errors
 * when working with quoted rates.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const r = core.Rate.fromBps(250);     // 2.5% as 250 bps
 * r.asDecimal;  // 0.025
 * r.asPercent;  // 2.5
 * r.asBps;      // 250
 * ```
 */
export interface RateConstructor {
  /**
   * Create a rate from a decimal value.
   *
   * @param decimal - Rate as a decimal (e.g. `0.05` for 5%).
   * @returns The constructed `Rate`.
   * @throws If `decimal` is non-finite (NaN, ±∞).
   *
   * @example
   * ```javascript
   * const r = new core.Rate(0.05);  // 5%
   * r.asPercent;  // 5
   * ```
   */
  new (decimal: number): Rate;
  /**
   * Create a rate from a percent figure.
   *
   * @param pct - Percent value (e.g. `5.0` for 5%).
   * @returns The constructed `Rate`.
   * @throws If `pct` is non-finite.
   *
   * @example
   * ```javascript
   * const r = core.Rate.fromPercent(5.0);
   * r.asDecimal;  // 0.05
   * ```
   */
  fromPercent(pct: number): Rate;
  /**
   * Create a rate from basis points.
   *
   * The canonical Rust `Rate::from_bps` takes an integer (`i32`) number
   * of basis points. Because JavaScript numbers are `f64`, this binding
   * accepts a float and rounds it to the nearest integer basis point
   * before delegating (banker-free half-away rounding via `Bps`).
   * Fractional inputs therefore lose sub-bp precision; use
   * `new Rate(decimal)` or `Rate.fromPercent` for sub-bp rates.
   *
   * @param bps - Rate in basis points (e.g. `500` for 5%). Rounded to the
   * nearest integer bp.
   * @returns The constructed `Rate`.
   * @throws If `bps` is non-finite.
   *
   * @example
   * ```javascript
   * const r = core.Rate.fromBps(250);  // 2.5%
   * r.asDecimal;  // 0.025
   * ```
   */
  fromBps(bps: number): Rate;
}

/**
 * Basis points (1 bp = 0.01%, 10_000 bps = 100%).
 *
 * Stored as integer bps internally; constructors round to the nearest bp.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const spread = new core.Bps(125);
 * spread.asDecimal();  // 0.0125
 * spread.asBps();      // 125
 * ```
 */
export interface Bps extends WasmOwned {
  /**
   * Value as a decimal (e.g. 25 bp → 0.0025).
   *
   * @returns Decimal equivalent.
   */
  asDecimal(): number;
  /**
   * Value in whole basis points.
   *
   * @returns Integer bps.
   */
  asBps(): number;
}

/**
 * Basis points (1 bp = 0.01%, 10_000 bps = 100%).
 *
 * Stored as integer bps internally; constructors round to the nearest bp.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const spread = new core.Bps(125);
 * spread.asDecimal();  // 0.0125
 * spread.asBps();      // 125
 * ```
 */
export interface BpsConstructor {
  /**
   * Create basis points from a floating value.
   *
   * @param value - Value in basis points (e.g. `25` for 25 bps). Rounded
   * to the nearest integer bp.
   * @returns The constructed `Bps`.
   * @throws If `value` is non-finite.
   */
  new (value: number): Bps;
}

/**
 * Percentage stored in percent points (`5.0` means 5%).
 *
 * Use this when you want the API to be explicit that the value is in
 * percent (rather than decimal). Equivalent to `Rate` for arithmetic.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const p = new core.Percentage(5.0);
 * p.asDecimal();  // 0.05
 * p.asPercent();  // 5
 * ```
 */
export interface Percentage extends WasmOwned {
  /**
   * Value as a decimal (5% → 0.05).
   *
   * @returns Decimal equivalent.
   */
  asDecimal(): number;
  /**
   * Value in percent points.
   *
   * @returns Percent value.
   */
  asPercent(): number;
}

/**
 * Percentage stored in percent points (`5.0` means 5%).
 *
 * Use this when you want the API to be explicit that the value is in
 * percent (rather than decimal). Equivalent to `Rate` for arithmetic.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const p = new core.Percentage(5.0);
 * p.asDecimal();  // 0.05
 * p.asPercent();  // 5
 * ```
 */
export interface PercentageConstructor {
  /**
   * Create a percentage.
   *
   * @param value - Value in percent (e.g. `5.0` for 5%).
   * @returns The constructed `Percentage`.
   * @throws If `value` is non-finite.
   */
  new (value: number): Percentage;
}

/**
 * Day-count convention for computing year fractions and day counts.
 *
 * Dates are represented as **epoch days** (`i32`, days since 1970-01-01).
 * Use `createDate` to convert from a `(year, month, day)` triple.
 *
 * Available conventions and their factories:
 * - `act_360` → `DayCount.act360`
 * - `act_365f` → `DayCount.act365f`
 * - `30_360` → `DayCount.thirty360`
 * - `30e_360` → `DayCount.thirtyE360`
 * - `30e_360_isda` → `DayCount.thirtyE360Isda`
 * - `act_act` (ISDA) → `DayCount.actAct`
 * - `act_act_isma` (ICMA) → `DayCount.actActIsma`
 * - `bus_252` → `DayCount.bus252`
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const dc = core.DayCount.act365f();
 * const start = core.createDate(2025, 1, 15);
 * const end   = core.createDate(2025, 7, 15);
 * const yf    = dc.yearFraction(start, end);
 * // yf ≈ 0.4959 (181 / 365)
 * ```
 */
export interface DayCount extends WasmOwned {
  /**
   * Compute the year fraction between two dates given as epoch days.
   *
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01.
   * @returns Year fraction (`>= 0` if `end >= start`).
   * @throws If either date is out of representable range.
   *
   * Act/Act ISMA and Bus/252 require explicit frequency/calendar context.
   * This method throws for those conventions; call
   * `DayCount.yearFractionWithContext` with a configured `DayCountContext`.
   *
   * @example
   * ```javascript
   * const dc = core.DayCount.act360();
   * const start = core.createDate(2025, 1, 15);
   * const end   = core.createDate(2025, 4, 15);
   * dc.yearFraction(start, end); // 90 / 360 = 0.25
   * ```
   */
  yearFraction(startEpochDays: number, endEpochDays: number): number;
  /**
   * Compute a signed year fraction, preserving the start/end orientation.
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  signedYearFraction(startEpochDays: number, endEpochDays: number): number;
  /**
   * Compute the year fraction with explicit convention context.
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01.
   * @param ctx - DayCountContext supplying calendar, frequency, coupon-period, and termination metadata.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  yearFractionWithContext(
    startEpochDays: number,
    endEpochDays: number,
    ctx: DayCountContext
  ): number;
  /**
   * Count the calendar days between two dates (epoch days).
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01.
   * @returns Returns the requested integer count.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  calendarDays(startEpochDays: number, endEpochDays: number): bigint;
  /**
   * Convention name.
   * @returns Returns the requested string representation or JSON payload.
   */
  toString(): string;
}

/**
 * Day-count convention for computing year fractions and day counts.
 *
 * Dates are represented as **epoch days** (`i32`, days since 1970-01-01).
 * Use `createDate` to convert from a `(year, month, day)` triple.
 *
 * Available conventions and their factories:
 * - `act_360` → `DayCount.act360`
 * - `act_365f` → `DayCount.act365f`
 * - `30_360` → `DayCount.thirty360`
 * - `30e_360` → `DayCount.thirtyE360`
 * - `30e_360_isda` → `DayCount.thirtyE360Isda`
 * - `act_act` (ISDA) → `DayCount.actAct`
 * - `act_act_isma` (ICMA) → `DayCount.actActIsma`
 * - `bus_252` → `DayCount.bus252`
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const dc = core.DayCount.act365f();
 * const start = core.createDate(2025, 1, 15);
 * const end   = core.createDate(2025, 7, 15);
 * const yf    = dc.yearFraction(start, end);
 * // yf ≈ 0.4959 (181 / 365)
 * ```
 */
export interface DayCountConstructor {
  /**
   * Parse a day-count convention from its string name.
   *
   * @param name - Convention name (e.g. `"act_360"`, `"30_360"`, `"act_act"`).
   * Underscored snake_case is canonical.
   * @returns The parsed `DayCount`.
   * @throws If `name` is not a recognized day-count convention.
   */
  new (name: string): DayCount;
  /**
   * Create a `DayCount` value using the act360 convention.
   * @returns Returns the resulting `DayCount` value or WebAssembly handle.
   */
  act360(): DayCount;
  /**
   * Actual/365 Fixed.
   * @returns Returns the resulting `DayCount` value or WebAssembly handle.
   */
  act365f(): DayCount;
  /**
   * Actual/365L (ICMA Rule 251). Annual periods (or periods without
   * frequency context) use denominator 366 exactly when February 29 falls
   * in `(start, end]`; non-annual periods use 366 exactly when the end
   * date's year is a leap year. Otherwise the denominator is 365. This is
   * not ACT/ACT AFB.
   * @returns Returns the resulting `DayCount` value or WebAssembly handle.
   */
  act365l(): DayCount;
  /**
   * 30/360 US (Bond Basis).
   * @returns Returns the resulting `DayCount` value or WebAssembly handle.
   */
  thirty360(): DayCount;
  /**
   * 30E/360 (Eurobond Basis).
   * @returns Returns the resulting `DayCount` value or WebAssembly handle.
   */
  thirtyE360(): DayCount;
  /**
   * Create a `DayCount` value using the thirty e360 isda convention.
   * @returns Returns the resulting `DayCount` value or WebAssembly handle.
   */
  thirtyE360Isda(): DayCount;
  /**
   * Actual/Actual (ISDA).
   * @returns Returns the resulting `DayCount` value or WebAssembly handle.
   */
  actAct(): DayCount;
  /**
   * Actual/Actual (ICMA/ISMA).
   * @returns Returns the resulting `DayCount` value or WebAssembly handle.
   */
  actActIsma(): DayCount;
  /**
   * Create a `DayCount` value using the bus252 convention.
   * @returns Returns the resulting `DayCount` value or WebAssembly handle.
   */
  bus252(): DayCount;
}

/**
 * Optional context for day-count conventions that need market metadata.
 */
export interface DayCountContext extends WasmOwned {
  /**
   * Return a copy with the calendar used by Bus/252.
   * @param calendarCode - Registered holiday-calendar identifier used by the Bus/252 convention.
   * @returns Returns the resulting `DayCountContext` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  withCalendar(calendarCode: string): DayCountContext;
  /**
   * Return a copy with the coupon frequency used by Act/Act ISMA.
   * @param frequency - Coupon-frequency Tenor required by Actual/Actual ICMA calculations.
   * @returns Returns the resulting `DayCountContext` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  withFrequency(frequency: Tenor): DayCountContext;
  /**
   * Return a copy with the business-day basis used by Bus/252.
   * @param busBasis - Business-day denominator for Bus/252, normally 252.
   * @returns Returns the resulting `DayCountContext` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  withBusBasis(busBasis: number): DayCountContext;
  /**
   * Return a copy with the reference coupon period (epoch days) used by
   * Act/Act ICMA. Errors when either date is out of range or
   * `start >= end`.
   * @param startEpochDays - Reference coupon-period start as days since 1970-01-01.
   * @param endEpochDays - Reference coupon-period end as days since 1970-01-01.
   * @returns Returns the resulting `DayCountContext` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  withCouponPeriod(startEpochDays: number, endEpochDays: number): DayCountContext;
  /**
   * Return a copy indicating whether the accrual end is the instrument's
   * termination date (required by 30E/360 ISDA February-end handling).
   * @param value - Whether the accrual end is the contractual termination date for 30E/360 ISDA.
   * @returns Returns the resulting `DayCountContext` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  withEndIsTerminationDate(value: boolean): DayCountContext;
}

/**
 * Optional context for day-count conventions that need market metadata.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const factory: DayCountContextConstructor = core.DayCountContext;
 * void factory;
 * ```
 */
export interface DayCountContextConstructor {
  /**
   * Create an empty day-count context.
   * @returns Returns the resulting `DayCountContext` value or WebAssembly handle.
   */
  new (): DayCountContext;
}

/**
 * A financial tenor such as `3M`, `1Y`, or `2W`.
 *
 * Tenors carry a numeric count and a unit (days, weeks, months, years).
 * Parse from strings or use the named-period factories (`Tenor.daily`,
 * `Tenor.weekly`, `Tenor.monthly`, `Tenor.quarterly`, `Tenor.semiAnnual`,
 * `Tenor.annual`).
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const t = new core.Tenor("3M");
 * t.toString();        // "3M"
 * t.toYearsSimple();   // 0.25
 *
 * const annual = core.Tenor.annual();
 * annual.toString();   // "1Y"
 * ```
 */
export interface Tenor extends WasmOwned {
  /**
   * Count exposed by this `Tenor` value.
   */
  readonly count: number;
  /**
   * Approximate length in years (simple estimate, no calendar).
   * @returns Returns the computed numeric result in the units described above.
   */
  toYearsSimple(): number;
  /**
   * Tenor string representation.
   * @returns Returns the requested string representation or JSON payload.
   */
  toString(): string;
}

/**
 * A financial tenor such as `3M`, `1Y`, or `2W`.
 *
 * Tenors carry a numeric count and a unit (days, weeks, months, years).
 * Parse from strings or use the named-period factories (`Tenor.daily`,
 * `Tenor.weekly`, `Tenor.monthly`, `Tenor.quarterly`, `Tenor.semiAnnual`,
 * `Tenor.annual`).
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const t = new core.Tenor("3M");
 * t.toString();        // "3M"
 * t.toYearsSimple();   // 0.25
 *
 * const annual = core.Tenor.annual();
 * annual.toString();   // "1Y"
 * ```
 */
export interface TenorConstructor {
  /**
   * Parse a tenor string.
   *
   * @param s - Tenor string. Accepted forms include `"3M"`, `"1Y"`,
   * `"2W"`, `"7D"`, `"6M"`, `"10Y"`. Whitespace is permitted.
   * @returns The parsed `Tenor`.
   * @throws If `s` cannot be parsed (unknown unit, missing count).
   */
  new (s: string): Tenor;
  /**
   * Create a `Tenor` value using the daily convention.
   * @returns Returns the resulting `Tenor` value or WebAssembly handle.
   */
  daily(): Tenor;
  /**
   * Create a `Tenor` value using the weekly convention.
   * @returns Returns the resulting `Tenor` value or WebAssembly handle.
   */
  weekly(): Tenor;
  /**
   * Create a `Tenor` value using the monthly convention.
   * @returns Returns the resulting `Tenor` value or WebAssembly handle.
   */
  monthly(): Tenor;
  /**
   * 3-month (quarterly) tenor.
   * @returns Returns the resulting `Tenor` value or WebAssembly handle.
   */
  quarterly(): Tenor;
  /**
   * 6-month (semi-annual) tenor.
   * @returns Returns the resulting `Tenor` value or WebAssembly handle.
   */
  semiAnnual(): Tenor;
  /**
   * 12-month (annual) tenor.
   * @returns Returns the resulting `Tenor` value or WebAssembly handle.
   */
  annual(): Tenor;
}

/**
 * TypeScript type that constrains the accepted discount curve validation mode values.
 */
export type DiscountCurveValidationMode = 'market_standard' | 'negative_rate_friendly';

/**
 * Discount factor curve for present-value calculations.
 *
 * Built from `(time, discount_factor)` pillars where `time` is a year
 * fraction from `baseDate` and `df` is the price today of $1 paid at that
 * time. Defaults reflect the most common practitioner convention
 * (Hagan-West monotone-convex interpolation, flat-forward extrapolation,
 * Act/365 fixed day-count).
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * // OIS-style USD curve, base-date 2025-01-02, three pillars.
 * const curve = new core.DiscountCurve(
 *   "USD-OIS",
 *   "2025-01-02",
 *   [0.0, 1.0, 1.0, 0.95, 5.0, 0.78],
 *   "monotone_convex",
 *   "flat_forward",
 *   "act_365f",
 * );
 * curve.df(2.5);          // discount factor at 2.5y
 * curve.zero(2.5);        // continuously-compounded zero rate at 2.5y
 * ```
 */
export interface DiscountCurve extends WasmOwned {
  /**
   * Curve identifier.
   */
  readonly id: string;
  /**
   * Base date as ISO string.
   */
  readonly baseDate: string;
  /**
   * Discount factor at year fraction `t`.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  df(t: number): number;
  /**
   * Continuously-compounded zero rate at year fraction `t`.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  zero(t: number): number;
  /**
   * Continuously-compounded forward rate between `t1` and `t2`.
   * @param t1 - Earlier curve time in years used as the start of the forward interval.
   * @param t2 - Later curve time in years used as the end of the forward interval.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  forward(t1: number, t2: number): number;
}

/**
 * Discount factor curve for present-value calculations.
 *
 * Built from `(time, discount_factor)` pillars where `time` is a year
 * fraction from `baseDate` and `df` is the price today of $1 paid at that
 * time. Defaults reflect the most common practitioner convention
 * (Hagan-West monotone-convex interpolation, flat-forward extrapolation,
 * Act/365 fixed day-count).
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * // OIS-style USD curve, base-date 2025-01-02, three pillars.
 * const curve = new core.DiscountCurve(
 *   "USD-OIS",
 *   "2025-01-02",
 *   [0.0, 1.0, 1.0, 0.95, 5.0, 0.78],
 *   "monotone_convex",
 *   "flat_forward",
 *   "act_365f",
 * );
 * curve.df(2.5);          // discount factor at 2.5y
 * curve.zero(2.5);        // continuously-compounded zero rate at 2.5y
 * ```
 */
export interface DiscountCurveConstructor {
  /**
   * Construct from an array of `[time, df]` pairs.
   *
   * @param id - Curve identifier (e.g. `"USD-OIS"`). Used as the lookup
   * key inside a `MarketContext`.
   * @param baseDate - ISO-8601 date string (`"YYYY-MM-DD"`). All `time`
   * values are interpreted as year fractions from this date under
   * `dayCount`.
   * @param knots - Flat `[t0, df0, t1, df1, …]` array. `t` in years,
   * `df` strictly positive. Length must be even.
   * @param interp - Interpolation style (default `"monotone_convex"`).
   * One of `"linear"`, `"log_linear"`, `"monotone_convex"`,
   * `"cubic_hermite"`, `"piecewise_quadratic_forward"`.
   * @param extrapolation - Extrapolation policy (default
   * `"flat_forward"`). One of `"flat_zero"`, `"flat_forward"`, `"nan"`.
   * @param dayCount - Day-count convention (defaults to curve-ID inference).
   * @param validationMode - Rust validation preset: `"market_standard"`
   * (default) or `"negative_rate_friendly"`.
   * @param forwardFloor - Required minimum implied forward when using
   * `"negative_rate_friendly"`.
   * @returns The constructed `DiscountCurve`.
   * @throws If `knots` length is odd, the date is malformed, the
   * interpolation style is unknown, or any `df` is non-positive.
   */
  new (
    id: string,
    baseDate: string,
    knots: NumericArray,
    interp?: string,
    extrapolation?: string,
    dayCount?: string,
    validationMode?: DiscountCurveValidationMode,
    forwardFloor?: number | null
  ): DiscountCurve;
  /**
   * Construct a flat continuously-compounded discount curve.
   * @param id - Stable identifier used to name and retrieve the supplied domain object.
   * @param baseDate - ISO-8601 curve base date from which time coordinates are measured.
   * @param continuousRate - Flat continuously compounded zero rate expressed as a decimal.
   * @returns Returns the resulting `DiscountCurve` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  flat(id: string, baseDate: string, continuousRate: number): DiscountCurve;
}

/**
 * Forward rate curve for a floating-rate index with a fixed tenor.
 */
export interface ForwardCurve extends WasmOwned {
  /**
   * Curve identifier.
   */
  readonly id: string;
  /**
   * Base date as ISO string.
   */
  readonly baseDate: string;
  /**
   * Contractual projection boundaries, or `null` for legacy tenor stepping.
   */
  readonly projectionGrid: Float64Array | null;
  /**
   * Business days from fixing to spot.
   */
  readonly resetLag: number;
  /**
   * Forward rate at year fraction `t`.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rate(t: number): number;
  /**
   * Discount-factor-implied simple forward over `(t1, t2)`.
   * @param t1 - Earlier curve time in years used as the start of the forward interval.
   * @param t2 - Later curve time in years used as the end of the forward interval.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rateBetween(t1: number, t2: number): number;
}

/**
 * Forward rate curve for a floating-rate index with a fixed tenor.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const factory: ForwardCurveConstructor = core.ForwardCurve;
 * void factory;
 * ```
 */
export interface ForwardCurveConstructor {
  /**
   * Construct from an array of `[time, rate]` pairs.
   *
   * @param id - Curve identifier.
   * @param tenor - Index tenor in years.
   * @param baseDate - ISO date string.
   * @param knots - Flat `[t0, rate0, t1, rate1, …]` array.
   * @param dayCount - Day-count convention (defaults to curve-ID inference).
   * @param interp - Interpolation style (default ``"linear"``).
   * @param extrapolation - Extrapolation policy (default ``"flat_forward"``).
   * @param projectionGrid - Optional contractual reset/end boundaries.
   * @param resetLag - Optional fixing-to-spot lag in business days; omit for Rust curve-ID inference.
   * @returns Returns the resulting `ForwardCurve` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  new (
    id: string,
    tenor: number,
    baseDate: string,
    knots: NumericArray,
    dayCount?: string,
    interp?: string,
    extrapolation?: string,
    projectionGrid?: NumericArray | null,
    resetLag?: number | null
  ): ForwardCurve;
  /**
   * Construct from a named JavaScript options object.
   * @param options - JavaScript options object defining the requested curve construction inputs.
   * @returns Returns the resulting `ForwardCurve` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  fromOptions(options: ForwardCurveOptions): ForwardCurve;
}

/**
 * TypeScript view of the `ForwardCurveOptions` WebAssembly value.
 */
export interface ForwardCurveOptions {
  /**
   * Stable identifier used to name or select the requested object.
   */
  id: string;
  /**
   * Tenor exposed by this `ForwardCurveOptions` value.
   */
  tenor: number;
  /**
   * ISO-8601 base or valuation date that anchors the curve time axis.
   */
  baseDate: string;
  /**
   * Knots exposed by this `ForwardCurveOptions` value.
   */
  knots: NumericArray;
  /**
   * Day-count convention used to convert dates into year fractions.
   */
  dayCount?: string;
  /**
   * Interp exposed by this `ForwardCurveOptions` value.
   */
  interp?: string;
  /**
   * Extrapolation exposed by this `ForwardCurveOptions` value.
   */
  extrapolation?: string;
  /**
   * Projection-grid specification that defines the curve's forward-rate intervals.
   */
  projectionGrid?: NumericArray | null;
  /**
   * Reset lag applied when projecting the index or forward rate.
   */
  resetLag?: number | null;
}

/**
 * SABR volatility cube for swaption pricing.
 *
 * Stores calibrated SABR parameters on an expiry × tenor grid and evaluates
 * implied volatilities via bilinear parameter interpolation followed by the
 * Hagan (2002) approximation.
 */
export interface VolCube extends WasmOwned {
  /**
   * Cube identifier.
   */
  readonly id: string;
  /**
   * Interpolation contract used across the expiry axis.
   */
  readonly interpolationMode: string;
  /**
   * Implied volatility at `(expiry, tenor, strike)`.
   *
   * Returns `Err` if `expiry` or `tenor` falls outside the grid.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param tenor - Underlying swap or index tenor measured in years for the quoted surface point.
   * @param strike - Option strike price in the same price units as the underlying.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  vol(expiry: number, tenor: number, strike: number): number;
  /**
   * Implied volatility with clamped extrapolation.
   *
   * Clamps finite `expiry` and `tenor` values to the grid edges before
   * interpolation. Non-finite inputs return `NaN`.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param tenor - Underlying swap or index tenor measured in years for the quoted surface point.
   * @param strike - Option strike price in the same price units as the underlying.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  volClamped(expiry: number, tenor: number, strike: number): number;
  /**
   * Normal (Bachelier) implied volatility at `(expiry, tenor, strike)`.
   *
   * The returned vol is in absolute rate units (e.g. `0.008` = 80 bp/yr
   * normal vol), the swaption market quoting convention.
   *
   * Returns `Err` if `expiry` or `tenor` falls outside the grid, if the
   * expansion yields a non-finite volatility, or for cross-zero quotes
   * (`(F+s)(K+s) <= 0`) with `beta > 0`, which require an explicit shift.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param tenor - Underlying swap or index tenor measured in years for the quoted surface point.
   * @param strike - Option strike price in the same price units as the underlying.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  volNormal(expiry: number, tenor: number, strike: number): number;
  /**
   * Normal (Bachelier) implied volatility with clamped extrapolation.
   *
   * Clamps finite `expiry` and `tenor` values to the grid edges; a
   * degenerate finite expansion is floored to a small positive normal vol
   * (absolute rate units). Non-finite inputs return `NaN`.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param tenor - Underlying swap or index tenor measured in years for the quoted surface point.
   * @param strike - Option strike price in the same price units as the underlying.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  volNormalClamped(expiry: number, tenor: number, strike: number): number;
}

/**
 * SABR volatility cube for swaption pricing.
 *
 * Stores calibrated SABR parameters on an expiry × tenor grid and evaluates
 * implied volatilities via bilinear parameter interpolation followed by the
 * Hagan (2002) approximation.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const factory: VolCubeConstructor = core.VolCube;
 * void factory;
 * ```
 */
export interface VolCubeConstructor {
  /**
   * Construct a vol cube from a flat SABR parameter array.
   *
   * @param id - Curve identifier.
   * @param expiries - Option expiry axis in years (strictly increasing).
   * @param tenors - Swap tenor axis in years (strictly increasing).
   * @param paramsFlat - Row-major flat array of SABR parameters: `[alpha0, beta0, rho0, nu0, shift0, alpha1, …]`. Length must equal `expiries.len() * tenors.len() * 5`. Pass `NaN` for the shift element of a node to omit the shift.
   * @param forwards - Row-major forward rates, one per grid node. @param interpolationMode - Volatility-surface interpolation mode used between quoted points.
   * @returns Returns the resulting `VolCube` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  new (
    id: string,
    expiries: NumericArray,
    tenors: NumericArray,
    paramsFlat: NumericArray,
    forwards: NumericArray,
    interpolationMode?: string
  ): VolCube;
}

/**
 * Typed FX conversion policy wrapper for WASM callers.
 */
export interface FxConversionPolicy extends WasmOwned {
  /**
   * String form of the conversion policy.
   * @returns Returns the requested string representation or JSON payload.
   */
  toString(): string;
}

/**
 * Typed FX conversion policy wrapper for WASM callers.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const factory: FxConversionPolicyConstructor = core.FxConversionPolicy;
 * void factory;
 * ```
 */
export interface FxConversionPolicyConstructor {
  /**
   * Use spot/forward on the cashflow date.
   * @returns Returns the resulting `FxConversionPolicy` value or WebAssembly handle.
   */
  cashflowDate(): FxConversionPolicy;
  /**
   * Use period end date.
   * @returns Returns the resulting `FxConversionPolicy` value or WebAssembly handle.
   */
  periodEnd(): FxConversionPolicy;
  /**
   * Use an average over the period.
   * @returns Returns the resulting `FxConversionPolicy` value or WebAssembly handle.
   */
  periodAverage(): FxConversionPolicy;
  /**
   * Use a custom provider-defined strategy.
   * @returns Returns the resulting `FxConversionPolicy` value or WebAssembly handle.
   */
  custom(): FxConversionPolicy;
  /**
   * Parse from a string label such as ``\"cashflow_date\"``.
   * @param name - Name supplied to from name; follow the type and convention required by the surrounding API.
   * @returns Returns the resulting `FxConversionPolicy` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  fromName(name: string): FxConversionPolicy;
}

/**
 * Structured FX lookup result for WASM callers.
 */
export interface FxRateResult extends WasmOwned {
  /**
   * The FX conversion rate.
   */
  readonly rate: number;
  /**
   * Whether the rate was obtained via triangulation.
   */
  readonly triangulated: boolean;
}

/**
 * `FxRateResult` has no public constructor; instances come from `FxMatrix.rate`.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const factory: FxRateResultConstructor = core.FxRateResult;
 * void factory;
 * ```
 */
export interface FxRateResultConstructor {
  /**
   * Prototype exposed by this `FxRateResult` value.
   */
  readonly prototype: FxRateResult;
}

/**
 * Foreign-exchange rate matrix for currency conversion.
 */
export interface FxMatrix extends WasmOwned {
  /**
   * Set an explicit FX quote.
   *
   * @param base - Base (from) currency ISO code.
   * @param quote - Quote (to) currency ISO code.
   * @param rate - Conversion rate.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  setQuote(base: string, quote: string, rate: number): void;
  /**
   * Set an authoritative quote scoped to one date and conversion policy.
   * @param base - Base currency code of the FX quote, where the rate is quote per base.
   * @param quote - Quote currency code of the FX rate, expressed per unit of base currency.
   * @param date - ISO-8601 date used by the calculation or market-data lookup.
   * @param policy - FX quote-selection policy for resolving direct, inverse, or triangulated rates.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  setQuoteOn(
    base: string,
    quote: string,
    date: string,
    policy: FxConversionPolicy,
    rate: number
  ): void;
  /**
   * Look up an FX rate.
   *
   * @param base - Base (from) currency ISO code.
   * @param quote - Quote (to) currency ISO code.
   * @param date - ISO date string.
   * @param policy - Reusable conversion policy handle.
   * @returns Returns the resulting `FxRateResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rate(base: string, quote: string, date: string, policy: FxConversionPolicy): FxRateResult;
  /**
   * Look up an FX rate using cashflow-date conversion semantics.
   * @param base - Base currency code of the FX quote, where the rate is quote per base.
   * @param quote - Quote currency code of the FX rate, expressed per unit of base currency.
   * @param date - ISO-8601 date used by the calculation or market-data lookup.
   * @returns Returns the resulting `FxRateResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rateDefault(base: string, quote: string, date: string): FxRateResult;
}

/**
 * Foreign-exchange rate matrix for currency conversion.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const factory: FxMatrixConstructor = core.FxMatrix;
 * void factory;
 * ```
 */
export interface FxMatrixConstructor {
  /**
   * Create an empty FX matrix.
   * @returns Returns the resulting `FxMatrix` value or WebAssembly handle.
   */
  new (): FxMatrix;
}

/**
 * FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
 * 10-delta wings).
 *
 * Stores market-standard FX delta quotes (Wystup 2006, Clark 2011) and
 * converts to a strike-axis volatility surface on demand via Garman-Kohlhagen.
 * The delta convention is **forward delta (premium-unadjusted)**.
 */
export interface FxDeltaVolSurface extends WasmOwned {
  /**
   * Surface identifier.
   */
  readonly id: string;
  /**
   * Expiry axis in years.
   */
  readonly expiries: Float64Array;
  /**
   * Number of expiry pillars.
   */
  readonly numExpiries: number;
  /**
   * Pillar vols at the given expiry index as `[atm, put25d_vol, call25d_vol]`.
   * @param expiryIdx - Zero-based index of the requested expiry pillar in the volatility surface.
   * @returns Returns numeric results as a `Float64Array` in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  pillarVols(expiryIdx: number): Float64Array;
  /**
   * Implied vol at `(expiry, strike)` for the supplied forward.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  impliedVol(expiry: number, strike: number, forward: number): number;
}

/**
 * FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
 * 10-delta wings).
 *
 * Stores market-standard FX delta quotes (Wystup 2006, Clark 2011) and
 * converts to a strike-axis volatility surface on demand via Garman-Kohlhagen.
 * The delta convention is **forward delta (premium-unadjusted)**.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const factory: FxDeltaVolSurfaceConstructor = core.FxDeltaVolSurface;
 * void factory;
 * ```
 */
export interface FxDeltaVolSurfaceConstructor {
  /**
   * Construct an FX delta-quoted vol surface with 25-delta wings.
   *
   * Optional `rr10d` / `bf10d` add 10-delta wings for richer wing
   * interpolation. Pass an empty array for both to omit; if one is
   * provided, the other must be too.
   *
   * @param id - Stable surface identifier.
   * @param expiries - Strictly increasing positive expiry times (years).
   * @param atmVols - ATM delta-neutral straddle vols per expiry.
   * @param rr25d - 25-delta risk reversal per expiry (call vol − put vol).
   * @param bf25d - 25-delta butterfly per expiry (wing avg − ATM).
   * @param rr10d - Optional 10-delta risk reversal per expiry.
   * @param bf10d - Optional 10-delta butterfly per expiry.
   * @returns Returns the resulting `FxDeltaVolSurface` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  new (
    id: string,
    expiries: NumericArray,
    atmVols: NumericArray,
    rr25d: NumericArray,
    bf25d: NumericArray,
    rr10d?: NumericArray,
    bf10d?: NumericArray
  ): FxDeltaVolSurface;
  /**
   * Convert a forward delta to a strike (Garman-Kohlhagen, premium-unadjusted).
   * @param delta - Option delta expressed under the surface's documented delta convention.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  deltaToStrike(delta: number, forward: number, vol: number, expiry: number): number;
  /**
   * Convert a strike to forward delta (Garman-Kohlhagen call delta).
   * @param strike - Option strike price in the same price units as the underlying.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  strikeToDelta(strike: number, forward: number, vol: number, expiry: number): number;
}

/**
 * Monte Carlo pricer result (JSON object from Rust).
 */
export interface MonteCarloEstimateJson {
  /**
   * Mean exposed by this `MonteCarloEstimateJson` value.
   */
  mean: number;
  /**
   * Currency exposed by this `MonteCarloEstimateJson` value.
   */
  currency: string;
  /**
   * Stderr exposed by this `MonteCarloEstimateJson` value.
   */
  stderr: number;
  /**
   * Sample standard deviation (absent when not computed).
   */
  std_dev?: number;
  /**
   * Ci lower exposed by this `MonteCarloEstimateJson` value.
   */
  ci_lower: number;
  /**
   * Ci upper exposed by this `MonteCarloEstimateJson` value.
   */
  ci_upper: number;
  /**
   * Number of independent path estimators; equals `num_simulated_paths` without variance reduction, half of it with antithetic pairing.
   */
  num_paths: number;
  /**
   * Total number of simulated sample paths; `2 * num_paths` with antithetic variates, otherwise equals `num_paths`.
   */
  num_simulated_paths: number;
  /**
   * Legacy skipped-path count; current engines reject non-finite payoffs.
   */
  num_skipped: number;
  /**
   * Median of captured discounted path values (absent when paths are not captured).
   */
  median?: number;
  /**
   * 25th percentile of captured discounted path values (absent when paths are not captured).
   */
  percentile_25?: number;
  /**
   * 75th percentile of captured discounted path values (absent when paths are not captured).
   */
  percentile_75?: number;
  /**
   * Minimum of captured discounted path values (absent when paths are not captured).
   */
  min?: number;
  /**
   * Maximum of captured discounted path values (absent when paths are not captured).
   */
  max?: number;
  /**
   * Relative standard error (`stderr / |mean|`); `Infinity` near zero mean.
   */
  relative_stderr: number;
}

/**
 * Variation margin calculator result (JSON object from Rust).
 */
export interface VariationMarginJson {
  /**
   * Gross exposure exposed by this `VariationMarginJson` value.
   */
  gross_exposure: number;
  /**
   * Net exposure exposed by this `VariationMarginJson` value.
   */
  net_exposure: number;
  /**
   * Delivery amount exposed by this `VariationMarginJson` value.
   */
  delivery_amount: number;
  /**
   * Return amount exposed by this `VariationMarginJson` value.
   */
  return_amount: number;
  /**
   * Net margin exposed by this `VariationMarginJson` value.
   */
  net_margin: number;
  /**
   * Requires call exposed by this `VariationMarginJson` value.
   */
  requires_call: boolean;
}

/**
 * Forecast backtest metrics (JSON object from Rust).
 */
export interface BacktestForecastMetricsJson {
  /**
   * Mae exposed by this `BacktestForecastMetricsJson` value.
   */
  mae: number;
  /**
   * Mape exposed by this `BacktestForecastMetricsJson` value.
   */
  mape: number;
  /**
   * Rmse exposed by this `BacktestForecastMetricsJson` value.
   */
  rmse: number;
  /**
   * N exposed by this `BacktestForecastMetricsJson` value.
   */
  n: number;
}

/**
 * Gross-leverage impact of a liability management exercise.
 * Leverage is gross debt over EBITDA, so `8.0` reads as 8.0x.
 */
export interface LmeLeverageImpact {
  /** Gross debt of the target instrument before the exercise. */
  pre_total_debt: number;
  /** Gross debt of the target instrument after the exercise. */
  post_total_debt: number;
  /** Gross debt over EBITDA before the exercise, as a multiple. */
  pre_leverage: number;
  /** Gross debt over EBITDA after the exercise, as a multiple. */
  post_leverage: number;
  /** Turns of leverage removed: `pre_leverage - post_leverage`. */
  leverage_reduction: number;
}

/** Hold-versus-tender economics of a distressed exchange offer. */
export interface ExchangeOfferAnalysis {
  /** Canonical offer structure, echoed back from the request. */
  exchange_type: 'par_for_par' | 'discount' | 'uptier' | 'downtier';
  /** Present value of the existing claim if it is not tendered. */
  old_npv: number;
  /** Present value of the new instrument received on tendering. */
  new_npv: number;
  /** Cash consent or early-tender fee. */
  consent_fee: number;
  /** Estimated value of attached equity or warrants. */
  equity_sweetener_value: number;
  /** Total tender consideration: `new_npv + consent_fee + equity_sweetener_value`. */
  tender_total: number;
  /** Tender consideration less the hold-out present value. */
  delta_npv: number;
  /** Hold-out recovery fraction that matches the tender; capped at 1.0. */
  breakeven_recovery: number;
  /** True when `tender_total` exceeds `old_npv * 1.02`. */
  tender_recommended: boolean;
}

/** Issuer-side economics of a liability management exercise. */
export interface LmeAnalysis {
  /** Canonical LME structure, echoed back from the request. */
  lme_type: 'open_market_repurchase' | 'tender_offer' | 'amend_and_extend' | 'dropdown';
  /** Cash paid by the issuer, in the caller's monetary unit. */
  cost: number;
  /** Face amount retired; zero for structures that do not extinguish debt. */
  notional_reduction: number;
  /** Par retired less cash paid — the discount captured by the issuer. */
  discount_capture: number;
  /** Discount captured as a fraction of par retired; zero when no par is retired. */
  discount_capture_pct: number;
  /** Value fraction diverted from non-participating holders; nonzero only for a dropdown. */
  remaining_holder_impact_pct: number;
  /** Gross-leverage block, or null when no positive EBITDA was supplied. */
  leverage_impact: LmeLeverageImpact | null;
}

/**
 * Namespaced TypeScript entry points for core calculations and types.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const api: CoreNamespace = core;
 * void api;
 * ```
 */
export interface CoreNamespace {
  /**
   * Currency exposed by this `Core` value.
   */
  Currency: CurrencyConstructor;
  /**
   * Money exposed by this `Core` value.
   */
  Money: MoneyConstructor;
  /**
   * Rate exposed by this `Core` value.
   */
  Rate: RateConstructor;
  /**
   * Bps exposed by this `Core` value.
   */
  Bps: BpsConstructor;
  /**
   * Percentage exposed by this `Core` value.
   */
  Percentage: PercentageConstructor;
  /**
   * Day count exposed by this `Core` value.
   */
  DayCount: DayCountConstructor;
  /**
   * Day count context exposed by this `Core` value.
   */
  DayCountContext: DayCountContextConstructor;
  /**
   * Tenor exposed by this `Core` value.
   */
  Tenor: TenorConstructor;
  /**
   * Create a date and return it as epoch days (days since 1970-01-01).
   * @param year - Four-digit calendar year component of the supplied date.
   * @param month - Calendar month number from 1 through 12.
   * @param day - Calendar day number within the selected month.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  createDate(year: number, month: number, day: number): number;
  /**
   * Convert epoch days back to `[year, month, day]` as a JS array-compatible triple.
   * @param days - Number of days since 1970-01-01 to decompose into year, month, and day.
   * @returns Returns numeric results as a `Int32Array` in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  dateFromEpochDays(days: number): Int32Array;
  /**
   * Adjust a date (epoch days) according to a business-day convention and calendar.
   *
   * Returns the adjusted date as epoch days.
   * @param epochDays - Unadjusted date as days since 1970-01-01.
   * @param convention - Business-day adjustment convention string accepted by the date API.
   * @param calendarCode - Registered holiday-calendar identifier used to find business days.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  adjust(epochDays: number, convention: string, calendarCode: string): number;
  /**
   * Return the list of available calendar codes.
   * @returns Returns the resulting `string[]` collection in the documented order.
   */
  availableCalendars(): string[];
  /**
   * Discount curve exposed by this `Core` value.
   */
  DiscountCurve: DiscountCurveConstructor;
  /**
   * Forward curve exposed by this `Core` value.
   */
  ForwardCurve: ForwardCurveConstructor;
  /**
   * Vol cube exposed by this `Core` value.
   */
  VolCube: VolCubeConstructor;
  /**
   * Fx delta vol surface exposed by this `Core` value.
   */
  FxDeltaVolSurface: FxDeltaVolSurfaceConstructor;
  /**
   * Fx conversion policy exposed by this `Core` value.
   */
  FxConversionPolicy: FxConversionPolicyConstructor;
  /**
   * Fx rate result exposed by this `Core` value.
   */
  FxRateResult: FxRateResultConstructor;
  /**
   * Fx matrix exposed by this `Core` value.
   */
  FxMatrix: FxMatrixConstructor;
  /**
   * Evaluate the static Nelson-Siegel (1987) yield curve for one factor triple.
   *
   * This is the Diebold-Li cross-sectional equation for a single date:
   * `y(tau) = b1 + b2 * s(tau) + b3 * (s(tau) - exp(-lambda * tau))` with
   * `s(tau) = (1 - exp(-lambda * tau)) / (lambda * tau)`. Returns one yield per
   * tenor, in decimal units and in input order.
   * @param lambda - Exponential decay parameter for tenors in years; must be finite and greater than zero (0.7308 is the years-equivalent of Diebold-Li's 0.0609 months value).
   * @param level - Nelson-Siegel beta1, the long-run level factor in decimal yield units such as 0.06 for 6%.
   * @param slope - Nelson-Siegel beta2, the slope factor (negative of the short-minus-long spread) in decimal yield units.
   * @param curvature - Nelson-Siegel beta3, the hump-shaped curvature factor in decimal yield units.
   * @param tenors - Maturities in years, each finite and non-negative; output order matches this array.
   * @returns Returns numeric results as a `Float64Array` in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  nelsonSiegelYields(
    lambda: number,
    level: number,
    slope: number,
    curvature: number,
    tenors: NumericArray,
  ): Float64Array;
  /**
   * Apply a lower-triangular factor L to a vector z, returning `L z`.
   *
   * This is the Cholesky "apply" step that turns independent standard normals
   * into correlated normals: if `A = L L^T` and `z ~ N(0, I)`, then
   * `L z ~ N(0, A)`. Accepts L as `n * n` row-major entries; only the lower
   * triangle is read and the upper triangle is assumed zero.
   * @param l - Lower-triangular Cholesky factor as a flat row-major array of n × n entries.
   * @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
   * @param z - Vector of length n to transform, typically independent standard-normal draws.
   * @returns Returns numeric results as a `Float64Array` in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  applyLowerTriangular(l: NumericArray, n: number, z: NumericArray): Float64Array;
  /**
   * Cholesky decomposition of a symmetric positive-definite matrix.
   *
   * Accepts a square matrix as a nested JS array (`number[][]`, row-major)
   * and returns the lower-triangular factor L such that A = L L^T.
   * @param matrix - Square numeric matrix in the nested or row-major shape required by this callable.
   * @returns Returns the resulting `number[][]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  choleskyDecomposition(matrix: number[][]): number[][];
  /**
   * Solve a symmetric positive-definite linear system A x = b given the
   * Cholesky factor L (where A = L L^T).
   *
   * Accepts L as `number[][]` and b as `number[]`. Returns x as `number[]`.
   * @param chol - Lower-triangular Cholesky factor of the coefficient matrix, in the documented matrix shape.
   * @param b - Right-hand-side vector of a linear system, aligned with the Cholesky factor dimension.
   * @returns Returns the resulting `number[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  choleskySolve(chol: number[][], b: number[]): number[];
  /**
   * Cholesky decomposition for a flat row-major matrix.
   *
   * Accepts a `Float64Array`/`number[]` containing `n * n` row-major entries
   * and returns a flat lower-triangular factor.
   * @param matrix - Square numeric matrix in the nested or row-major shape required by this callable.
   * @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
   * @returns Returns numeric results as a `Float64Array` in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  choleskyDecompositionFlat(matrix: NumericArray, n: number): Float64Array;
  /**
   * Solve a symmetric positive-definite linear system from a flat Cholesky factor.
   * @param chol - Lower-triangular Cholesky factor of the coefficient matrix, in the documented matrix shape.
   * @param b - Right-hand-side vector of a linear system, aligned with the Cholesky factor dimension.
   * @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
   * @returns Returns numeric results as a `Float64Array` in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  choleskySolveFlat(chol: NumericArray, b: NumericArray, n: number): Float64Array;
  /**
   * Validate a flat row-major correlation matrix.
   *
   * This is the only correlation-matrix validator on the `core` namespace.
   * Callers pass `n * n` row-major entries plus the matrix dimension `n`.
   * @param matrix - Square numeric matrix in the nested or row-major shape required by this callable.
   * @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateCorrelationMatrixFlat(matrix: NumericArray, n: number): void;
  /**
   * Arithmetic mean.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  mean(data: number[]): number;
  /**
   * Arithmetic mean over a typed numeric array.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  meanArray(data: NumericArray): number;
  /**
   * Sample variance (unbiased, n-1 denominator).
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  variance(data: number[]): number;
  /**
   * Sample variance over a typed numeric array.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  varianceArray(data: NumericArray): number;
  /**
   * Population variance (n denominator).
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  populationVariance(data: number[]): number;
  /**
   * Population variance over a typed numeric array.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  populationVarianceArray(data: NumericArray): number;
  /**
   * Pearson correlation coefficient.
   * @param x - Numeric observation series aligned one-for-one with the other series.
   * @param y - Numeric observation series aligned one-for-one with the other series.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  correlation(x: number[], y: number[]): number;
  /**
   * Pearson correlation over typed numeric arrays.
   * @param x - Numeric observation series aligned one-for-one with the other series.
   * @param y - Numeric observation series aligned one-for-one with the other series.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  correlationArray(x: NumericArray, y: NumericArray): number;
  /**
   * Sample covariance (unbiased, n-1 denominator).
   * @param x - Numeric observation series aligned one-for-one with the other series.
   * @param y - Numeric observation series aligned one-for-one with the other series.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  covariance(x: number[], y: number[]): number;
  /**
   * Sample covariance over typed numeric arrays.
   * @param x - Numeric observation series aligned one-for-one with the other series.
   * @param y - Numeric observation series aligned one-for-one with the other series.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  covarianceArray(x: NumericArray, y: NumericArray): number;
  /**
   * Empirical quantile (R-7 / NumPy default) with linear interpolation.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @param q - Quantile probability from 0 through 1 used to select the order statistic.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  quantile(data: number[], q: number): number;
  /**
   * Empirical quantile over a typed numeric array.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @param q - Quantile probability from 0 through 1 used to select the order statistic.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  quantileArray(data: NumericArray, q: number): number;
  /**
   * Standard normal CDF Φ(x).
   * @param x - Real-valued input to the requested scalar mathematical function.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  normCdf(x: number): number;
  /**
   * Standard normal PDF φ(x).
   * @param x - Real-valued input to the requested scalar mathematical function.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  normPdf(x: number): number;
  /**
   * Inverse standard normal CDF Φ⁻¹(p).
   * @param p - Probability input strictly between 0 and 1 for the inverse normal distribution.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  standardNormalInvCdf(p: number): number;
  /**
   * Error function erf(x).
   * @param x - Real-valued input to the requested scalar mathematical function.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  erf(x: number): number;
  /**
   * Natural logarithm of the Gamma function ln(Γ(x)).
   * @param x - Real-valued input to the requested scalar mathematical function.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  lnGamma(x: number): number;
  /**
   * Kahan compensated summation.
   * @param values - Numeric values in the order used by the requested numerical operation.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  kahanSum(values: number[]): number;
  /**
   * Kahan compensated summation over a typed numeric array.
   * @param values - Numeric values in the order used by the requested numerical operation.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  kahanSumArray(values: NumericArray): number;
  /**
   * Neumaier compensated summation — handles mixed-sign values.
   * @param values - Numeric values in the order used by the requested numerical operation.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  neumaierSum(values: number[]): number;
  /**
   * Neumaier compensated summation over a typed numeric array.
   * @param values - Numeric values in the order used by the requested numerical operation.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  neumaierSumArray(values: NumericArray): number;
  /**
   * Count the longest consecutive run of strictly positive values.
   * @param values - Numeric values in the order used by the requested numerical operation.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  countConsecutive(values: number[]): number;
  /**
   * Count the longest consecutive run of strictly positive values in a typed array.
   * @param values - Numeric values in the order used by the requested numerical operation.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  countConsecutiveArray(values: NumericArray): number;
  /**
   * Compare hold-versus-tender economics for a distressed exchange offer.
   * Tendering is recommended only when the total consideration exceeds the
   * hold-out present value by more than 2%.
   * @param oldPv - Present value of the existing claim if it is not tendered, in the caller's monetary unit.
   * @param newPv - Present value of the new instrument received on tendering, in the same unit as oldPv.
   * @param consentFee - Cash consent or early-tender fee paid to participating holders, in the same unit as oldPv.
   * @param equitySweetenerValue - Estimated value of equity or warrants attached to the new instrument, in the same unit as oldPv.
   * @param exchangeType - Offer structure: par_for_par (alias par), discount, uptier, or downtier. Case-insensitive; '-' normalises to '_'.
   * @returns Returns the tender total, NPV pickup, breakeven recovery, and tender recommendation.
   * @throws Error - Thrown when an amount is negative or non-finite, or exchangeType is not a recognised structure.
   */
  analyzeExchangeOffer(
    oldPv: number,
    newPv: number,
    consentFee: number,
    equitySweetenerValue: number,
    exchangeType: string,
  ): ExchangeOfferAnalysis;
  /**
   * Compute discount capture and leverage impact for an LME transaction.
   * @param lmeType - Structure of the exercise: open_market (aliases open_market_repurchase, omr), tender_offer (alias tender), amend_and_extend (aliases ae, a&e), or dropdown.
   * @param notional - Outstanding face amount of the target instrument, in the caller's monetary unit; must be finite and positive.
   * @param repurchasePricePct - Price as a fraction of par for repurchases and tenders ((0, 1.5]), the extension fee for amend-and-extend ([0, 0.10]), or the transferred-asset fraction for a dropdown ([0, 1]).
   * @param optAcceptancePct - Fraction of holders participating, in [0, 1].
   * @param ebitda - EBITDA in the same unit as notional; a positive value adds the leverage_impact block, null or non-positive omits it.
   * @returns Returns cash cost, par retired, discount captured, remaining-holder impact, and the optional leverage block.
   * @throws Error - Thrown when notional is not positive, optAcceptancePct is outside [0, 1], repurchasePricePct is outside the range admitted by lmeType, or lmeType is not recognised.
   */
  analyzeLme(
    lmeType: string,
    notional: number,
    repurchasePricePct: number,
    optAcceptancePct: number,
    ebitda?: number | null,
  ): LmeAnalysis;
}

/**
 * Namespaced TypeScript entry point for core APIs.
 */
export declare const core: CoreNamespace;

// --- analytics ------------------------------------------------------------

/**
 * TypeScript type that constrains the accepted numeric array values.
 */
export type NumericArray = number[] | Float64Array;
/**
 * TypeScript type that constrains the accepted numeric matrix values.
 */
export type NumericMatrix = NumericArray[];

/**
 * Descriptive statistics returned by `peerStats`.
 */
export interface PeerStatsJson {
  /**
   * Count exposed by this `PeerStatsJson` value.
   */
  count: number;
  /**
   * Mean exposed by this `PeerStatsJson` value.
   */
  mean: number;
  /**
   * Median exposed by this `PeerStatsJson` value.
   */
  median: number;
  /**
   * Std dev exposed by this `PeerStatsJson` value.
   */
  std_dev: number;
  /**
   * Min exposed by this `PeerStatsJson` value.
   */
  min: number;
  /**
   * Max exposed by this `PeerStatsJson` value.
   */
  max: number;
  /**
   * Q1 exposed by this `PeerStatsJson` value.
   */
  q1: number;
  /**
   * Q3 exposed by this `PeerStatsJson` value.
   */
  q3: number;
  /**
   * Interquartile range (`q3 - q1`).
   */
  iqr: number;
}

/**
 * Single-factor OLS regression result returned by `regressionFairValue`.
 */
export interface RegressionResultJson {
  /**
   * Intercept exposed by this `RegressionResultJson` value.
   */
  intercept: number;
  /**
   * Slope exposed by this `RegressionResultJson` value.
   */
  slope: number;
  /**
   * R squared exposed by this `RegressionResultJson` value.
   */
  r_squared: number;
  /**
   * Fitted value exposed by this `RegressionResultJson` value.
   */
  fitted_value: number;
  /**
   * Residual exposed by this `RegressionResultJson` value.
   */
  residual: number;
  /**
   * N exposed by this `RegressionResultJson` value.
   */
  n: number;
}

/**
 * Per-dimension decomposition in a relative value score.
 */
export interface DimensionScoreJson {
  /**
   * Label exposed by this `DimensionScoreJson` value.
   */
  label: string;
  /**
   * Percentile exposed by this `DimensionScoreJson` value.
   */
  percentile: number;
  /**
   * Z score exposed by this `DimensionScoreJson` value.
   */
  z_score: number;
  /**
   * Regression residual exposed by this `DimensionScoreJson` value.
   */
  regression_residual: number | null;
  /**
   * R squared exposed by this `DimensionScoreJson` value.
   */
  r_squared: number | null;
  /**
   * Weight exposed by this `DimensionScoreJson` value.
   */
  weight: number;
}

/**
 * Composite relative value result returned by `scoreRelativeValue`.
 */
export interface RelativeValueResultJson {
  /**
   * Company id exposed by this `RelativeValueResultJson` value.
   */
  company_id: string;
  /**
   * Composite score exposed by this `RelativeValueResultJson` value.
   */
  composite_score: number;
  /**
   * Dimensions exposed by this `RelativeValueResultJson` value.
   */
  dimensions: DimensionScoreJson[];
  /**
   * Confidence exposed by this `RelativeValueResultJson` value.
   */
  confidence: number;
  /**
   * Peer count exposed by this `RelativeValueResultJson` value.
   */
  peer_count: number;
}

/**
 * Structured formula explanation returned by `explainFormula`.
 */
export interface FormulaExplanationJson {
  /**
   * Node id exposed by this `FormulaExplanationJson` value.
   */
  node_id: string;
  /**
   * Period id exposed by this `FormulaExplanationJson` value.
   */
  period_id: string;
  /**
   * Final value exposed by this `FormulaExplanationJson` value.
   */
  final_value: number;
  /**
   * Node type exposed by this `FormulaExplanationJson` value.
   */
  node_type: string;
  /**
   * Formula text exposed by this `FormulaExplanationJson` value.
   */
  formula_text?: string | null;
  /**
   * Breakdown exposed by this `FormulaExplanationJson` value.
   */
  breakdown: FormulaExplanationStepJson[];
}

/**
 * One component in a structured formula explanation.
 */
export interface FormulaExplanationStepJson {
  /**
   * Component exposed by this `FormulaExplanationStepJson` value.
   */
  component: string;
  /**
   * Value exposed by this `FormulaExplanationStepJson` value.
   */
  value: number;
  /**
   * Operation exposed by this `FormulaExplanationStepJson` value.
   */
  operation?: string | null;
}

/**
 * A single drawdown episode returned by `drawdownDetails`.
 */
export interface DrawdownEpisode {
  /**
   * Start exposed by this `DrawdownEpisode` value.
   */
  start: string;
  /**
   * Valley exposed by this `DrawdownEpisode` value.
   */
  valley: string;
  /**
   * End exposed by this `DrawdownEpisode` value.
   */
  end: string | null;
  /**
   * Duration days exposed by this `DrawdownEpisode` value.
   */
  duration_days: number;
  /**
   * Max drawdown exposed by this `DrawdownEpisode` value.
   */
  max_drawdown: number;
  /**
   * Near recovery threshold exposed by this `DrawdownEpisode` value.
   */
  near_recovery_threshold: number;
  /**
   * Truncated at start exposed by this `DrawdownEpisode` value.
   */
  truncated_at_start: boolean;
}

/**
 * Aggregate statistics for grouped periodic returns.
 */
export interface PeriodStats {
  /**
   * Best exposed by this `PeriodStats` value.
   */
  best: number;
  /**
   * Worst exposed by this `PeriodStats` value.
   */
  worst: number;
  /**
   * Consecutive wins exposed by this `PeriodStats` value.
   */
  consecutive_wins: number;
  /**
   * Consecutive losses exposed by this `PeriodStats` value.
   */
  consecutive_losses: number;
  /**
   * Win rate exposed by this `PeriodStats` value.
   */
  win_rate: number;
  /**
   * Avg return exposed by this `PeriodStats` value.
   */
  avg_return: number;
  /**
   * Avg win exposed by this `PeriodStats` value.
   */
  avg_win: number;
  /**
   * Avg loss exposed by this `PeriodStats` value.
   */
  avg_loss: number;
  /**
   * Payoff ratio exposed by this `PeriodStats` value.
   */
  payoff_ratio: number;
  /**
   * Profit factor exposed by this `PeriodStats` value.
   */
  profit_factor: number;
  /**
   * Cpc ratio exposed by this `PeriodStats` value.
   */
  cpc_ratio: number;
  /**
   * Kelly criterion exposed by this `PeriodStats` value.
   */
  kelly_criterion: number;
}

/**
 * Dated rolling result returned by per-ticker rolling analytics.
 *
 * Exactly one metric-named key (`sharpe`, `sortino`, `volatility`, or
 * `return`) is present, matching the method that produced the series.
 */
export interface DatedSeries {
  /**
   * Dates exposed by this `DatedSeries` value.
   */
  dates: string[];
  /**
   * Sharpe exposed by this `DatedSeries` value.
   */
  sharpe?: Float64Array;
  /**
   * Sortino exposed by this `DatedSeries` value.
   */
  sortino?: Float64Array;
  /**
   * Volatility exposed by this `DatedSeries` value.
   */
  volatility?: Float64Array;
  /**
   * Return exposed by this `DatedSeries` value.
   */
  return?: Float64Array;
}

/**
 * Per-asset skewness/kurtosis pair returned by `skewKurt`.
 */
export interface SkewKurtResult {
  /**
   * Skewness exposed by this `SkewKurtResult` value.
   */
  skewness: Float64Array;
  /**
   * Kurtosis exposed by this `SkewKurtResult` value.
   */
  kurtosis: Float64Array;
}

/**
 * Per-asset VaR/ES pair returned by `valueAtRiskAndEs`.
 */
export interface VarEsResult {
  /**
   * Value at risk exposed by this `VarEsResult` value.
   */
  value_at_risk: Float64Array;
  /**
   * Expected shortfall exposed by this `VarEsResult` value.
   */
  expected_shortfall: Float64Array;
}

/**
 * OLS beta result with standard error and 95% confidence interval.
 *
 * The interval uses Student-t critical values for finite samples and an
 * asymptotic normal approximation once n - 2 >= 240.
 */
export interface BetaResult {
  /**
   * Beta exposed by this `BetaResult` value.
   */
  beta: number;
  /**
   * Std err exposed by this `BetaResult` value.
   */
  std_err: number;
  /**
   * Ci lower exposed by this `BetaResult` value.
   */
  ci_lower: number;
  /**
   * Ci upper exposed by this `BetaResult` value.
   */
  ci_upper: number;
}

/**
 * Single-factor greeks (annualized Jensen alpha, beta, R², adjusted R²).
 */
export interface GreeksResult {
  /**
   * Alpha exposed by this `GreeksResult` value.
   */
  alpha: number;
  /**
   * Beta exposed by this `GreeksResult` value.
   */
  beta: number;
  /**
   * R squared exposed by this `GreeksResult` value.
   */
  r_squared: number;
  /**
   * Adjusted r squared exposed by this `GreeksResult` value.
   */
  adjusted_r_squared: number;
}

/**
 * Rolling greeks output aligned with rolling-window end dates.
 */
export interface RollingGreeksResult {
  /**
   * Dates exposed by this `RollingGreeksResult` value.
   */
  dates: string[];
  /**
   * Alphas exposed by this `RollingGreeksResult` value.
   */
  alphas: Float64Array;
  /**
   * Betas exposed by this `RollingGreeksResult` value.
   */
  betas: Float64Array;
}

/**
 * Multi-factor regression result. Alpha is the raw regression intercept, annualized.
 */
export interface MultiFactorResult {
  /**
   * Alpha exposed by this `MultiFactorResult` value.
   */
  alpha: number;
  /**
   * Betas exposed by this `MultiFactorResult` value.
   */
  betas: number[];
  /**
   * R squared exposed by this `MultiFactorResult` value.
   */
  r_squared: number;
  /**
   * Adjusted r squared exposed by this `MultiFactorResult` value.
   */
  adjusted_r_squared: number;
  /**
   * Residual vol exposed by this `MultiFactorResult` value.
   */
  residual_vol: number;
}

/**
 * Period-to-date lookback returns (per ticker) returned by `lookbackReturns`.
 */
export interface LookbackReturns {
  /**
   * Mtd exposed by this `LookbackReturns` value.
   */
  mtd: number[];
  /**
   * Qtd exposed by this `LookbackReturns` value.
   */
  qtd: number[];
  /**
   * Ytd exposed by this `LookbackReturns` value.
   */
  ytd: number[];
  /**
   * Fytd exposed by this `LookbackReturns` value.
   */
  fytd: number[] | null;
}

/**
 * Stateful performance analytics engine over a panel of ticker series.
 *
 * `Performance` is the single entry point exposed to JS. Construct from
 * a price matrix (`new Performance(...)`) or a return matrix
 * (`Performance.fromReturns(...)`); every metric is then reachable as
 * an instance method.
 *
 * All multi-ticker scalar outputs come back as `number[]` indexed by the
 * panel's ticker order; vector / per-ticker / structured outputs are
 * serialized to plain JS objects (e.g. `DatedSeries`, `BetaResult[]`).
 */
export declare class Performance {
  constructor(
    dates: string[],
    prices: NumericMatrix,
    tickerNames: string[],
    benchmarkTicker?: string | null,
    freq?: string
  );
  /** Construct from a return matrix (one row per `dates` entry per ticker). */
  static fromReturns(
    dates: string[],
    returns: NumericMatrix,
    tickerNames: string[],
    benchmarkTicker?: string | null,
    freq?: string
  ): Performance;
  resetDateRange(start: string, end: string): void;
  resetBenchTicker(ticker: string): void;
  tickerNames(): string[];
  benchmarkIdx(): number;
  freq(): string;
  /** Full return-aligned date grid as ISO date strings (independent of any active window). */
  dates(): string[];
  /** Dates of the currently active analysis window as ISO date strings. */
  activeDates(): string[];
  /** Dates for one ticker's active return series as ISO date strings. */
  activeDatesForTicker(tickerIdx: number): string[];
  cagr(): Float64Array;
  meanReturn(annualize?: boolean): Float64Array;
  volatility(annualize?: boolean): Float64Array;
  sharpe(riskFreeRate?: number): Float64Array;
  /** Sortino ratio; mar is a per-period threshold. */
  sortino(mar?: number): Float64Array;
  calmar(): Float64Array;
  maxDrawdown(): Float64Array;
  meanDrawdown(): Float64Array;
  valueAtRisk(confidence?: number): Float64Array;
  expectedShortfall(confidence?: number): Float64Array;
  trackingError(): Float64Array;
  informationRatio(): Float64Array;
  skewness(): Float64Array;
  kurtosis(): Float64Array;
  geometricMean(): Float64Array;
  /** Skewness and kurtosis from one moments pass per asset. */
  skewKurt(): SkewKurtResult;
  /** Historical VaR and expected shortfall from one tail pass per asset. */
  valueAtRiskAndEs(confidence?: number): VarEsResult;
  /** Downside deviation; mar is a per-period threshold. */
  downsideDeviation(mar?: number): Float64Array;
  maxDrawdownDuration(): number[];
  /** Empyrical-style annualized geometric up-capture. */
  upCapture(): Float64Array;
  /** Empyrical-style annualized geometric down-capture. */
  downCapture(): Float64Array;
  /** Empyrical-style annualized geometric up/down capture ratio. */
  captureRatio(): Float64Array;
  omegaRatio(threshold?: number): Float64Array;
  treynor(riskFreeRate?: number): Float64Array;
  gainToPain(): Float64Array;
  ulcerIndex(): Float64Array;
  martinRatio(): Float64Array;
  recoveryFactor(): Float64Array;
  painIndex(): Float64Array;
  painRatio(riskFreeRate?: number): Float64Array;
  tailRatio(confidence?: number): Float64Array;
  rSquared(): Float64Array;
  battingAverage(): Float64Array;
  parametricVar(confidence?: number): Float64Array;
  cornishFisherVar(confidence?: number): Float64Array;
  cdar(confidence?: number): Float64Array;
  mSquared(riskFreeRate?: number): Float64Array;
  modifiedSharpe(riskFreeRate?: number, confidence?: number): Float64Array;
  sterlingRatio(riskFreeRate?: number, n?: number): Float64Array;
  burkeRatio(riskFreeRate?: number, n?: number): Float64Array;
  /**
   * Per-period simple returns per asset, as decimal fractions (0.01 = +1%).
   *
   * Canonical accessor for the raw return panel over the active window; prefer
   * it over `excessReturns` with an all-zero risk-free series or un-compounding
   * `cumulativeReturns`. Series are span-aware and therefore ragged across
   * assets on edge-ragged panels.
   * @returns One Float64Array per asset, in `tickerNames()` order.
   */
  returns(): Float64Array[];
  /**
   * Per-period simple returns for one asset, as decimal fractions (0.01 = +1%).
   * @param tickerIdx - Zero-based ticker column index in `tickerNames()` order.
   * @returns The asset's simple return series in date order.
   * @throws If `tickerIdx` is outside the loaded ticker columns.
   */
  returnsForTicker(tickerIdx: number): Float64Array;
  cumulativeReturns(): Float64Array[];
  drawdownSeries(): Float64Array[];
  correlationMatrix(): Float64Array[];
  cumulativeReturnsOutperformance(): Float64Array[];
  drawdownDifference(): Float64Array[];
  excessReturns(rf: NumericArray, nperiods?: number): Float64Array[];
  beta(): BetaResult[];
  greeks(riskFreeRate?: number): GreeksResult[];
  rollingGreeks(tickerIdx: number, window?: number, riskFreeRate?: number): RollingGreeksResult;
  rollingVolatility(tickerIdx: number, window?: number): DatedSeries;
  rollingSortino(tickerIdx: number, window?: number, mar?: number): DatedSeries;
  rollingSharpe(tickerIdx: number, window?: number, riskFreeRate?: number): DatedSeries;
  rollingReturns(tickerIdx: number, window: number): DatedSeries;
  drawdownDetails(tickerIdx: number, n?: number): DrawdownEpisode[];
  multiFactorGreeks(tickerIdx: number, factorReturns: NumericMatrix): MultiFactorResult;
  /**
   * Period-to-date lookback returns. The FYTD window starts at the fiscal-year
   * start adjusted to the next business day on `calendar` (default `"nyse"`);
   * pass the calendar id matching your market for non-US panels.
   */
  lookbackReturns(
    refDate: string,
    fiscalYearStartMonth?: number,
    fiscalYearStartDay?: number,
    calendar?: string
  ): LookbackReturns;
  periodStats(
    tickerIdx: number,
    aggFreq?: string,
    fiscalYearStartMonth?: number,
    fiscalYearStartDay?: number
  ): PeriodStats;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Namespaced TypeScript entry points for analytics calculations and types.
 * @example
 * ```typescript
 * import init, { analytics } from "finstack-quant-wasm";
 * await init();
 * const api: AnalyticsNamespace = analytics;
 * void api;
 * ```
 */
export interface AnalyticsNamespace {
  /**
   * `Performance` is the single entry point for analytics on a panel of
   * ticker series. Construct from prices (`new Performance(...)`) or from
   * returns (`Performance.fromReturns(...)`); every metric — return/risk
   * scalars, drawdown statistics, rolling windows, periodic returns
   * (MTD/QTD/YTD/FYTD), benchmark alpha/beta, basic factor models — is a
   * method on the resulting instance.
   */
  Performance: typeof Performance;
}

/**
 * Namespaced TypeScript entry point for analytics APIs.
 */
export declare const analytics: AnalyticsNamespace;

// --- factor_model.credit ------------------------------------------------------

/**
 * Calibrated credit factor hierarchy artifact.
 *
 * Produced by `CreditCalibrator` or deserialized from JSON via `fromJson`.
 * Immutable once constructed.
 */
export declare class CreditFactorModel {
  private constructor();
  /** Deserialize and validate a `CreditFactorModel` from JSON. */
  static fromJson(s: string): CreditFactorModel;
  /** Serialize to pretty-printed JSON. */
  toJson(): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Deterministic calibrator that produces a `CreditFactorModel`.
 *
 * Configuration and inputs are passed as JSON strings.
 */
export declare class CreditCalibrator {
  /** Construct a calibrator from a JSON-serialized `CreditCalibrationConfig`. */
  constructor(configJson: string);
  /** Run the calibration pipeline and return a `CreditFactorModel`. */
  calibrate(inputsJson: string): CreditFactorModel;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Snapshot of all hierarchy-level factor values at a single date.
 *
 * Produced by `decomposeLevels`. Pass to `decomposePeriod` to compute
 * period-over-period changes.
 */
export declare class LevelsAtDate {
  private constructor();
  /** Serialize the snapshot to pretty-printed JSON. */
  toJson(): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Component-wise difference between two `LevelsAtDate` snapshots.
 *
 * Produced by `decomposePeriod`.
 */
export declare class PeriodDecomposition {
  private constructor();
  /** Serialize the decomposition to pretty-printed JSON. */
  toJson(): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Vol-forecast view over a calibrated `CreditFactorModel`.
 *
 * `VolHorizon::Custom` is intentionally **not** exposed.
 *
 * Horizon strings accepted by `covarianceAt`, `idiosyncraticVol`, and
 * `factorModelAt`:
 * - `"one_step"` — calibrated annualized variance unchanged.
 * - `"unconditional"` — long-run.
 * - `'{"n_steps": N}'` — variance scaled by `N`.
 */
export declare class FactorCovarianceForecast {
  constructor(model: CreditFactorModel);
  /**
   * Build the factor covariance matrix at the requested horizon.
   * Returns pretty-printed JSON of a `FactorCovarianceMatrix`.
   */
  covarianceAt(horizonJson: string): string;
  /** Idiosyncratic vol (std dev) for a specific issuer at the requested horizon. */
  idiosyncraticVol(issuerId: string, horizonJson: string): number;
  /**
   * Build a portfolio-level `FactorModelConfig` JSON at the given horizon and
   * risk measure.
   */
  factorModelAt(horizonJson: string, riskMeasureJson: string): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Decompose observed issuer spreads at a point in time into per-level factor
 * values and per-issuer residual adders.
 *
 * @param model                Calibrated `CreditFactorModel`.
 * @param observedSpreadsJson  JSON `{issuer_id: spread}` map.
 * @param observedGeneric      Generic (PC) factor value at `asOf`.
 * @param asOf                 ISO 8601 date string.
 * @param runtimeTagsJson      Optional JSON `{issuer_id: {dim_key: tag}}` for
 *                             issuers not present in the model artifact.
 * @example
 * ```typescript
 * import init, { decomposeLevels } from "finstack-quant-wasm";
 * await init();
 * // Supply the documented arguments to decomposeLevels(...) for your use case.
 * void decomposeLevels;
 * ```
 * @returns Returns the resulting `LevelsAtDate` value or WebAssembly handle.
 * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
 */
export declare function decomposeLevels(
  model: CreditFactorModel,
  observedSpreadsJson: string,
  observedGeneric: number,
  asOf: string,
  runtimeTagsJson?: string
): LevelsAtDate;

/**
 * Difference two `LevelsAtDate` snapshots component-wise.
 *
 * Output is restricted to buckets and issuers present in **both** snapshots.
 * @example
 * ```typescript
 * import init, { decomposePeriod } from "finstack-quant-wasm";
 * await init();
 * // Supply the documented arguments to decomposePeriod(...) for your use case.
 * void decomposePeriod;
 * ```
 * @param fromLevels - Earlier hierarchy-level snapshot used as the start of the period comparison.
 * @param toLevels - Later hierarchy-level snapshot used as the end of the period comparison.
 * @returns Returns the resulting `PeriodDecomposition` value or WebAssembly handle.
 * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
 */
export declare function decomposePeriod(
  fromLevels: LevelsAtDate,
  toLevels: LevelsAtDate
): PeriodDecomposition;

/**
 * Namespaced TypeScript entry points for factor model credit calculations and types.
 * @example
 * ```typescript
 * import init, { factor_model } from "finstack-quant-wasm";
 * await init();
 * const api: FactorModelCreditNamespace = factor_model.credit;
 * void api;
 * ```
 */
export interface FactorModelCreditNamespace {
  /**
   * Credit factor model exposed by this `FactorModelCredit` value.
   */
  CreditFactorModel: typeof CreditFactorModel;
  /**
   * Credit calibrator exposed by this `FactorModelCredit` value.
   */
  CreditCalibrator: typeof CreditCalibrator;
  /**
   * Levels at date exposed by this `FactorModelCredit` value.
   */
  LevelsAtDate: typeof LevelsAtDate;
  /**
   * Period decomposition exposed by this `FactorModelCredit` value.
   */
  PeriodDecomposition: typeof PeriodDecomposition;
  /**
   * Factor covariance forecast exposed by this `FactorModelCredit` value.
   */
  FactorCovarianceForecast: typeof FactorCovarianceForecast;
  /**
   * Decompose observed issuer spreads at a point in time into per-level factor
   * values and per-issuer residual adders.
   *
   * - `model` — calibrated `CreditFactorModel`.
   * - `observed_spreads_json` — JSON `{issuer_id: spread}` map.
   * - `observed_generic` — generic (PC) factor value at `as_of`.
   * - `as_of` — ISO 8601 date string.
   * - `runtime_tags_json` — optional JSON `{issuer_id: {dim_key: tag}}` for
   *   issuers not present in the model artifact.
   *
   * Returns a `LevelsAtDate` handle.
   *
   * # Errors
   * Throws if an issuer has no model row and no `runtime_tags` entry, or if
   * `as_of` cannot be parsed.
   * @param model - Calibrated CreditFactorModel used to produce the covariance forecast.
   * @param observedSpreadsJson - JSON-serialized observed credit spreads used in the level decomposition.
   * @param observedGeneric - Observed generic-market spread component aligned with the model factors.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param runtimeTagsJson - Optional runtime-tag JSON selecting the active factor-model configuration.
   * @returns Returns the resulting `LevelsAtDate` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  decomposeLevels(
    model: CreditFactorModel,
    observedSpreadsJson: string,
    observedGeneric: number,
    asOf: string,
    runtimeTagsJson?: string
  ): LevelsAtDate;
  /**
   * Difference two `LevelsAtDate` snapshots component-wise.
   *
   * Output buckets and issuers are restricted to those present in **both**
   * snapshots so the linear reconciliation invariant on `ΔS_i` holds.
   *
   * # Errors
   * Throws if `from_levels.date > to_levels.date` or the snapshots disagree
   * on hierarchy depth.
   * @param fromLevels - Credit-factor levels at the start of the attribution period.
   * @param toLevels - Credit-factor levels at the end of the attribution period.
   * @returns Returns the resulting `PeriodDecomposition` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  decomposePeriod(fromLevels: LevelsAtDate, toLevels: LevelsAtDate): PeriodDecomposition;
}

/**
 * Namespaced TypeScript entry points for factor model calculations and types.
 * @example
 * ```typescript
 * import init, { factor_model } from "finstack-quant-wasm";
 * await init();
 * const api: FactorModelNamespace = factor_model;
 * void api;
 * ```
 */
export interface FactorModelNamespace {
  /**
   * Credit factor hierarchy artifacts, calibration, and decomposition.
   */
  credit: FactorModelCreditNamespace;
}

/**
 * Namespaced TypeScript entry point for factor model APIs.
 */
export declare const factor_model: FactorModelNamespace;

// --- features ---------------------------------------------------------------

/**
 * TypeScript type that constrains the accepted feature value values.
 */
export type FeatureValue = number | null;
/**
 * TypeScript type that constrains the accepted feature params values.
 */
export type FeatureParams = Record<string, unknown>;

/**
 * Vectorized panel feature transforms.
 *
 * `values` accepts finite numbers or `null`; non-finite values are treated as
 * missing by the Rust crate. Time-series transforms are grouped by `entity` and
 * sorted by `order`; cross-sectional transforms partition by `timeKey`.
 * @example
 * ```typescript
 * import init, { features } from "finstack-quant-wasm";
 * await init();
 * const api: FeaturesNamespace = features;
 * void api;
 * ```
 */
export interface FeaturesNamespace {
  /**
   * Transform a time-series panel column per entity.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param entity - Entity identifier used to group ordered time-series observations.
   * @param order - Observation-order key used to sort each entity time series.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  transformTimeseries(
    values: FeatureValue[],
    entity: string[],
    order: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Transform a cross-section per timestamp.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  transformCrossSectional(
    values: FeatureValue[],
    timeKey: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Transform a cross-section within each time/group sub-partition.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param groups - Group labels aligned with values for within-group cross-sectional operations.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  transformCrossSectionalGrouped(
    values: FeatureValue[],
    timeKey: string[],
    groups: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Remove cross-sectional exposure effects by OLS residualization.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param exposures - Factor-exposure matrix aligned with the supplied observations.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  neutralize(
    values: FeatureValue[],
    timeKey: string[],
    exposures: FeatureValue[][],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Transform two time-series panel columns per entity.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param other - Second value series aligned with the primary series for a pairwise transformation.
   * @param entity - Entity identifier used to group ordered time-series observations.
   * @param order - Observation-order key used to sort each entity time series.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  transformTimeseriesPairwise(
    values: FeatureValue[],
    other: FeatureValue[],
    entity: string[],
    order: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Return rolling OLS residuals per entity.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param exposures - Factor-exposure matrix aligned with the supplied observations.
   * @param entity - Entity identifier used to group ordered time-series observations.
   * @param order - Observation-order key used to sort each entity time series.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rollingRegressionResidual(
    values: FeatureValue[],
    exposures: FeatureValue[][],
    entity: string[],
    order: string[],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Convert a signal to inverse-risk-scaled weights per timestamp.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param volatility - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  riskScaledWeights(
    values: FeatureValue[],
    timeKey: string[],
    volatility: FeatureValue[],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Apply the default signal cleaning pass.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  cleanSignal(
    values: FeatureValue[],
    timeKey: string[],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Normalize a signal cross-sectionally.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  normalizeSignal(
    values: FeatureValue[],
    timeKey: string[],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Convert ranks into long/short weights.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rankToWeights(
    values: FeatureValue[],
    timeKey: string[],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Neutralize a signal and z-score residuals.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param exposures - Factor-exposure matrix aligned with the supplied observations.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @returns Returns the resulting `FeatureValue[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  neutralizeAndZscore(
    values: FeatureValue[],
    timeKey: string[],
    exposures: FeatureValue[][],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Apply a JSON panel transform pipeline.
   * @param specJson - Canonical panel-transformation JSON specifying input columns, operations, and parameters.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  transformPanel(specJson: string): string;
}

/**
 * Namespaced TypeScript entry point for features APIs.
 */
export declare const features: FeaturesNamespace;

// --- valuations.correlation -------------------------------------------------

/**
 * Concrete copula model for portfolio default correlation.
 */
export interface Copula extends WasmOwned {
  /**
   * Number of systematic factors in the model.
   */
  readonly numFactors: number;
  /**
   * Model name for diagnostics.
   */
  readonly modelName: string;
  /**
   * Conditional default probability given factor realization(s).
   * @param defaultThreshold - Latent-variable default threshold corresponding to the marginal default probability.
   * @param factorRealization - Realized systematic-factor value conditioning the default probability.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  conditionalDefaultProb(
    defaultThreshold: number,
    factorRealization: number[],
    correlation: number
  ): number;
  /**
   * Strict lower-tail dependence coefficient `λ_L` at the given
   * correlation.
   *
   * Returns `NaN` when the model has no closed-form `λ_L` (Random Factor
   * Loading); check `Number.isNaN()` before using the result. For the
   * RFL heuristic stress gauge use `stressCorrelationProxy` instead.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  tailDependence(correlation: number): number;
  /**
   * Heuristic stress-correlation proxy for the Random Factor Loading
   * copula.
   *
   * This is **not** the strict copula lower-tail-dependence coefficient
   * `λ_L` (which has no closed form for RFL — `tailDependence` returns
   * `NaN`). It gauges the extra correlation mass in the high-loading
   * tail and vanishes in the Gaussian (`loadingVol = 0`) limit.
   *
   * Throws for non-RFL copulas.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  stressCorrelationProxy(correlation: number): number;
}

/**
 * Copula model specification for configuration and deferred construction.
 */
export interface CopulaSpec extends WasmOwned {
  /**
   * True if this is a Gaussian spec.
   */
  readonly isGaussian: boolean;
  /**
   * True if this is a Student-t spec.
   */
  readonly isStudentT: boolean;
  /**
   * True if this is a Random Factor Loading spec.
   */
  readonly isRfl: boolean;
  /**
   * True if this is a Multi-factor spec.
   */
  readonly isMultiFactor: boolean;
  /**
   * Build a concrete copula from this specification.
   * @returns Returns the resulting `Copula` value or WebAssembly handle.
   */
  build(): Copula;
}

/**
 * Copula model specification for configuration and deferred construction.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const factory: CopulaSpecConstructor = valuations.correlation.CopulaSpec;
 * void factory;
 * ```
 */
export interface CopulaSpecConstructor {
  /**
   * One-factor Gaussian copula (market standard).
   * @returns Returns the resulting `CopulaSpec` value or WebAssembly handle.
   */
  gaussian(): CopulaSpec;
  /**
   * Student-t copula with specified degrees of freedom (must be > 2).
   * @param df - Positive Student-t copula degrees of freedom controlling tail thickness.
   * @returns Returns the resulting `CopulaSpec` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  studentT(df: number): CopulaSpec;
  /**
   * Random Factor Loading copula with stochastic correlation.
   * @param loadingVol - Standard deviation used to randomize the factor loading.
   * @returns Returns the resulting `CopulaSpec` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  randomFactorLoading(loadingVol: number): CopulaSpec;
  /**
   * Multi-factor Gaussian copula with sector structure.
   * @param numFactors - Positive number of systematic factors in the Gaussian factor model.
   * @returns Returns the resulting `CopulaSpec` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  multiFactor(numFactors: number): CopulaSpec;
}

/**
 * Concrete recovery model for credit portfolio pricing.
 */
export interface RecoveryModel extends WasmOwned {
  /**
   * Expected (unconditional, Jensen-corrected) recovery rate.
   */
  readonly expectedRecovery: number;
  /**
   * Loss given default (1 − recovery).
   */
  readonly lgd: number;
  /**
   * Recovery-rate volatility scale (0 for constant models).
   */
  readonly recoveryVolatility: number;
  /**
   * Whether recovery varies with the market factor.
   */
  readonly isStochastic: boolean;
  /**
   * Model name for diagnostics.
   */
  readonly modelName: string;
  /**
   * Recovery conditional on the systematic market factor.
   * @param marketFactor - Realized standardized market factor used to condition recovery or loss given default.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  conditionalRecovery(marketFactor: number): number;
  /**
   * Conditional LGD given market factor.
   * @param marketFactor - Realized standardized market factor used to condition recovery or loss given default.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  conditionalLgd(marketFactor: number): number;
}

/**
 * Recovery model specification for configuration and deferred construction.
 */
export interface RecoverySpec extends WasmOwned {
  /**
   * Location-parameter recovery rate of this spec.
   *
   * For a constant spec this is the constant rate. For a
   * market-correlated spec this returns the `mean` input — the target
   * recovery at factor `Z = 0` — which differs from the Jensen-corrected
   * unconditional mean `E_Z[R(Z)]` whenever the factor sensitivity is
   * non-zero. For the true unconditional mean call
   * `build().expectedRecovery`.
   */
  readonly expectedRecovery: number;
  /**
   * Build a concrete recovery model from this specification.
   * @returns Returns the resulting `RecoveryModel` value or WebAssembly handle.
   */
  build(): RecoveryModel;
}

/**
 * Recovery model specification for configuration and deferred construction.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const factory: RecoverySpecConstructor = valuations.correlation.RecoverySpec;
 * void factory;
 * ```
 */
export interface RecoverySpecConstructor {
  /**
   * Constant recovery rate.
   *
   * Throws if `rate` is not finite or lies outside `[0, 1]`.
   * @param rate - Constant recovery rate expressed as a fraction from 0 through 1.
   * @returns Returns the resulting `RecoverySpec` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  constant(rate: number): RecoverySpec;
  /**
   * Market-correlated (Andersen-Sidenius) stochastic recovery.
   *
   * Throws if `mean` is not finite or lies outside `[0, 1]`, or if `vol` /
   * `correlation` are not finite.
   * @param mean - Mean recovery rate expressed as a fraction from 0 through 1.
   * @param vol - Recovery-rate volatility scale in the correlated recovery model.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @returns Returns the resulting `RecoverySpec` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  marketCorrelated(mean: number, vol: number, correlation: number): RecoverySpec;
  /**
   * Market-standard stochastic recovery (40% mean, 25% vol, +40% corr —
   * recovery falls in stress under the canonical low-factor-stress
   * convention).
   * @returns Returns the resulting `RecoverySpec` value or WebAssembly handle.
   */
  marketStandardStochastic(): RecoverySpec;
}

/**
 * Exported class; construct instances via `CopulaSpec.build()` (no public `new`).
 */
export interface CopulaClass {
  /**
   * Prototype exposed by this `CopulaClass` value.
   */
  readonly prototype: Copula;
}

/**
 * Exported class; construct instances via `RecoverySpec.build()` (no public `new`).
 */
export interface RecoveryModelClass {
  /**
   * Prototype exposed by this `RecoveryModelClass` value.
   */
  readonly prototype: RecoveryModel;
}

/**
 * Tranche loss statistics returned by
 * {@link CorrelationNamespace.trancheLossStatistics}.
 *
 * Fractions are expressed relative to the tranche notional unless the field
 * name says otherwise; amounts are in the same unit as the input losses.
 */
export interface TrancheLossStatisticsJson {
  /** Tranche attachment point as a fraction of pool notional, in `[0, 1)`. */
  attachment: number;
  /** Tranche detachment point as a fraction of pool notional, in `(0, 1]`. */
  detachment: number;
  /** Tranche notional `(detachment - attachment) * poolNotional`. */
  tranche_notional: number;
  /** Mean tranche loss as a fraction of tranche notional, in `[0, 1]`. */
  expected_loss_fraction: number;
  /** Mean tranche loss in pool-notional units. */
  expected_loss_amount: number;
  /** Nearest-rank tranche loss fraction at the distribution's confidence. */
  var_fraction: number;
  /** Nearest-rank tranche loss amount at the distribution's confidence. */
  var_amount: number;
  /** Mean tranche loss fraction from the VaR observation through the worst path. */
  expected_shortfall_fraction: number;
  /** Mean tranche loss amount from the VaR observation through the worst path. */
  expected_shortfall_amount: number;
  /** Share of paths whose pool loss fraction strictly exceeds `attachment`. */
  prob_attachment_breached: number;
  /** Share of paths whose pool loss fraction reaches or exceeds `detachment`. */
  prob_full_writedown: number;
}

/**
 * Namespaced TypeScript entry points for correlation calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const api: CorrelationNamespace = valuations.correlation;
 * void api;
 * ```
 */
export interface CorrelationNamespace {
  /**
   * Copula spec exposed by this `Correlation` value.
   */
  CopulaSpec: CopulaSpecConstructor;
  /**
   * Copula exposed by this `Correlation` value.
   */
  Copula: CopulaClass;
  /**
   * Recovery spec exposed by this `Correlation` value.
   */
  RecoverySpec: RecoverySpecConstructor;
  /**
   * Recovery model exposed by this `Correlation` value.
   */
  RecoveryModel: RecoveryModelClass;
  /**
   * Fréchet-Hoeffding correlation bounds for two Bernoulli marginals.
   *
   * Returns `[rho_min, rho_max]`.
   * @param p1 - First marginal default probability from 0 through 1.
   * @param p2 - Second marginal default probability from 0 through 1.
   * @returns Returns numeric results as a `Float64Array` in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  correlationBounds(p1: number, p2: number): Float64Array;
  /**
   * Joint probabilities for two correlated Bernoulli variables.
   *
   * Returns `[p11, p10, p01, p00]`.
   * @param p1 - First marginal default probability from 0 through 1.
   * @param p2 - Second marginal default probability from 0 through 1.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @returns Returns numeric results as a `Float64Array` in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  jointProbabilities(p1: number, p2: number, correlation: number): Float64Array;
  /**
   * Validate a flat row-major correlation matrix.
   *
   * Accepts a `Float64Array`/`number[]` of `n * n` row-major entries and
   * checks unit diagonal, off-diagonal in `[-1, 1]`, symmetry, and positive
   * semi-definiteness. Returns nothing on success; raises a descriptive error
   * (including the failing dimension or constraint) otherwise.
   * @param matrix - Square numeric matrix in the nested or row-major shape required by this callable.
   * @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateCorrelationMatrix(matrix: NumericArray, n: number): void;
  /**
   * Nearest correlation matrix (Higham 2002) for a near-PSD input.
   *
   * Projects a symmetric, near-unit-diagonal, near-PSD matrix onto the set of
   * valid correlation matrices in Frobenius norm. Gross input violations
   * (asymmetry > 1e-6 or diagonal far from 1) throw rather than being silently
   * reshaped. Returns the flat row-major result as a `Float64Array`.
   */
  /**
   * Nearest correlation matrix (Higham 2002).
   *
   * Given a flat row-major `n*n` matrix that is approximately a correlation
   * matrix but fails Cholesky by a small margin, returns the nearest valid
   * correlation matrix (symmetric, unit diagonal, PSD) in Frobenius norm.
   * Gross input violations raise rather than being silently reshaped.
   * @param matrix - Square numeric matrix in the nested or row-major shape required by this callable.
   * @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
   * @param maxIter - Maximum number of Higham nearest-correlation projection iterations.
   * @param tol - Positive convergence tolerance for the nearest-correlation projection.
   * @returns Returns numeric results as a `Float64Array` in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  nearestCorrelation(matrix: NumericArray, n: number, maxIter?: number, tol?: number): Float64Array;
  /**
   * Tranche loss statistics over a simulated pool loss distribution.
   *
   * `attachment` and `detachment` are fractions of pool notional in `[0, 1]` —
   * a 0-3% equity tranche is `(0.0, 0.03)`, not `(0.0, 3.0)`. Each path's pool
   * loss fraction `L = loss / poolNotional` maps through
   * `clamp(L - attachment, 0, width) / width`, and the resulting distribution
   * is aggregated at `confidence` using loss-positive nearest-rank conventions.
   * @param losses - Loss-positive path losses in one caller-defined unit, one entry per simulated path.
   * @param confidence - Loss-positive VaR and expected-shortfall confidence strictly between 0 and 1.
   * @param attachment - Lower tranche boundary as a fraction of pool notional from 0 through 1.
   * @param detachment - Upper tranche boundary as a fraction of pool notional, strictly above the attachment and at most 1.
   * @param poolNotional - Total pool notional, finite and strictly positive, in the same unit as the losses.
   * @returns Returns the tranche notional, expected loss, VaR, expected shortfall, and breach probabilities.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  trancheLossStatistics(
    losses: NumericArray,
    confidence: number,
    attachment: number,
    detachment: number,
    poolNotional: number,
  ): TrancheLossStatisticsJson;
}

// --- monte_carlo ----------------------------------------------------------
// Convenience subset of finstack-quant-monte-carlo. Advanced Rust process,
// discretization, RNG, payoff, and Greeks types are not standalone WASM types.

/**
 * Namespaced TypeScript entry points for monte carlo calculations and types.
 * @example
 * ```typescript
 * import init, { monte_carlo } from "finstack-quant-wasm";
 * await init();
 * const api: MonteCarloNamespace = monte_carlo;
 * void api;
 * ```
 */
export interface MonteCarloNamespace {
  /**
   * Price a European call option via Monte Carlo under GBM dynamics.
   *
   * Returns a JSON object with `mean`, `currency`, `stderr`, `std_dev`,
   * `ci_lower`, `ci_upper`, `num_paths`, `num_simulated_paths`, `num_skipped`,
   * `median`, `percentile_25`, `percentile_75`, `min`, `max`, and
   * `relative_stderr`.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceEuropeanCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  /**
   * Price a European put option via Monte Carlo under GBM dynamics.
   *
   * Returns a JSON object with `mean`, `currency`, `stderr`, `std_dev`,
   * `ci_lower`, `ci_upper`, `num_paths`, `num_simulated_paths`, `num_skipped`,
   * `median`, `percentile_25`, `percentile_75`, `min`, `max`, and
   * `relative_stderr`.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceEuropeanPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  /**
   * Price a European call under Heston stochastic volatility.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param kappa - Mean-reversion speed of variance in the Heston stochastic-volatility model.
   * @param theta - Long-run variance level in the Heston stochastic-volatility model.
   * @param volOfVol - Annualized volatility of variance in the Heston stochastic-volatility model.
   * @param rho - Instantaneous correlation between the asset and variance shocks.
   * @param v0 - Initial instantaneous variance in the Heston stochastic-volatility model.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceHestonCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    kappa: number,
    theta: number,
    volOfVol: number,
    rho: number,
    v0: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  /**
   * Price a European put under Heston stochastic volatility.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param kappa - Mean-reversion speed of variance in the Heston stochastic-volatility model.
   * @param theta - Long-run variance level in the Heston stochastic-volatility model.
   * @param volOfVol - Annualized volatility of variance in the Heston stochastic-volatility model.
   * @param rho - Instantaneous correlation between the asset and variance shocks.
   * @param v0 - Initial instantaneous variance in the Heston stochastic-volatility model.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceHestonPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    kappa: number,
    theta: number,
    volOfVol: number,
    rho: number,
    v0: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  /**
   * Price an Asian call via Monte Carlo under GBM dynamics.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceAsianCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  /**
   * Price an Asian put via Monte Carlo under GBM dynamics.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceAsianPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string
  ): MonteCarloEstimateJson;
  /**
   * Price an American put via LSMC under GBM dynamics.
   *
   * Optional knobs:
   * - `use_parallel` (default `false`): run path generation on the rayon pool.
   * - `basis` (default `"laguerre"`): regression basis — `"laguerre"`,
   *   `"polynomial"`, or `"normalized_polynomial"`.
   * - `basis_degree` (default `3`): polynomial/Laguerre degree. Must be
   *   positive; `"laguerre"` additionally requires degree in `[1, 4]`.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @param useParallel - Whether simulation paths are evaluated in parallel when supported.
   * @param basis - Regression basis family used by the American-option exercise estimator.
   * @param basisDegree - Maximum polynomial degree used by the American-option exercise basis.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceAmericanPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string,
    useParallel?: boolean,
    basis?: string,
    basisDegree?: number
  ): MonteCarloEstimateJson;
  /**
   * Price an American call via LSMC under GBM dynamics.
   *
   * Optional knobs match [`price_american_put`].
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @param useParallel - Whether simulation paths are evaluated in parallel when supported.
   * @param basis - Regression basis family used by the American-option exercise estimator.
   * @param basisDegree - Maximum polynomial degree used by the American-option exercise basis.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceAmericanCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    numSteps?: number,
    currency?: string,
    useParallel?: boolean,
    basis?: string,
    basisDegree?: number
  ): MonteCarloEstimateJson;
  /**
   * Two-pass unbiased American put price (training fit + out-of-sample pricing).
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param pricingSeed - Independent deterministic seed used for unbiased-pricing sampling.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @param useParallel - Whether simulation paths are evaluated in parallel when supported.
   * @param basis - Regression basis family used by the American-option exercise estimator.
   * @param basisDegree - Maximum polynomial degree used by the American-option exercise basis.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceAmericanPutUnbiased(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    pricingSeed: bigint,
    numSteps?: number,
    currency?: string,
    useParallel?: boolean,
    basis?: string,
    basisDegree?: number
  ): MonteCarloEstimateJson;
  /**
   * Two-pass unbiased American call price (training fit + out-of-sample pricing).
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @param numPaths - Number of simulated stochastic paths; larger values improve sampling precision.
   * @param seed - Deterministic random-number seed used to reproduce simulation output.
   * @param pricingSeed - Independent deterministic seed used for unbiased-pricing sampling.
   * @param numSteps - Number of time steps per simulated path.
   * @param currency - ISO-4217 currency code for the monetary amount or market convention.
   * @param useParallel - Whether simulation paths are evaluated in parallel when supported.
   * @param basis - Regression basis family used by the American-option exercise estimator.
   * @param basisDegree - Maximum polynomial degree used by the American-option exercise basis.
   * @returns Returns the resulting `MonteCarloEstimateJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceAmericanCallUnbiased(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numPaths: number,
    seed: bigint,
    pricingSeed: bigint,
    numSteps?: number,
    currency?: string,
    useParallel?: boolean,
    basis?: string,
    basisDegree?: number
  ): MonteCarloEstimateJson;
  /**
   * Black-Scholes call price.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  blackScholesCall(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number
  ): number;
  /**
   * Black-Scholes put price.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to option expiry in years on the model's annual time basis.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  blackScholesPut(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number
  ): number;
}

/**
 * Namespaced TypeScript entry point for monte carlo APIs.
 */
export declare const monte_carlo: MonteCarloNamespace;

// --- margin ----------------------------------------------------------------

/**
 * Namespaced TypeScript entry points for margin calculations and types.
 * @example
 * ```typescript
 * import init, { margin } from "finstack-quant-wasm";
 * await init();
 * const api: MarginNamespace = margin;
 * void api;
 * ```
 */
export interface MarginNamespace {
  /**
   * Create a standard USD regulatory CSA specification as JSON.
   *
   * Returns the canonical ISDA-compliant CSA for USD OTC derivatives.
   * @returns Returns the requested string representation or JSON payload.
   */
  csaUsdRegulatory(): string;
  /**
   * Create a standard EUR regulatory CSA specification as JSON.
   * @returns Returns the requested string representation or JSON payload.
   */
  csaEurRegulatory(): string;
  /**
   * Validate a CSA specification JSON string.
   *
   * Deserializes and re-serializes the input to verify it conforms
   * to the `CsaSpec` schema. Returns the canonical JSON on success.
   * @param json - CSA specification JSON to validate and normalize into canonical form.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateCsaJson(json: string): string;
  /**
   * Calculate variation margin given exposure, posted collateral, and CSA JSON.
   *
   * Returns a JSON object with delivery_amount, return_amount, net_exposure,
   * and requires_call fields.
   *
   * @param csaJson - CSA specification as JSON string
   * @param exposure - Current mark-to-market exposure amount
   * @param postedCollateral - Currently posted collateral amount
   * @param currency - ISO currency code (e.g. "USD")
   * @param year - Calculation year
   * @param month - Calculation month (1-12)
   * @param day - Calendar day number within the selected month of the VM calculation date. @param csaJson - CSA specification JSON governing thresholds, minimum transfer, and timing. @param exposure - Current mark-to-market exposure in the supplied currency units. @param postedCollateral - Collateral already posted in the supplied currency units. @param currency - ISO-4217 currency code shared by exposure and collateral amounts. @param year - Calendar year of the VM calculation date. @param month - Calendar month from 1 through 12 of the VM calculation date. @param day - Calendar day number within the selected month of the VM calculation date.
   * @returns Returns the resulting `VariationMarginJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  calculateVm(
    csaJson: string,
    exposure: number,
    postedCollateral: number,
    currency: string,
    year: number,
    month: number,
    day: number
  ): VariationMarginJson;
}

/**
 * Namespaced TypeScript entry point for margin APIs.
 */
export declare const margin: MarginNamespace;

// --- cashflows -------------------------------------------------------------

/**
 * JSON bridge to the Rust `finstack-quant-cashflows` crate.
 *
 * All methods accept and return JSON strings that mirror the canonical Rust
 * serde model. Cashflow JSON types are exported from `./types`.
 * @example
 * ```typescript
 * import init, { cashflows } from "finstack-quant-wasm";
 * await init();
 * const api: CashflowsNamespace = cashflows;
 * void api;
 * ```
 */
export interface CashflowsNamespace {
  /**
   * Build a cashflow schedule from a `CashflowScheduleBuildSpec` JSON string.
   *
   * @param specJson    JSON-encoded `CashflowScheduleBuildSpec`.
   * @param marketJson  Optional JSON-encoded market context for floating-rate lookups.
   * @returns           JSON-encoded `CashFlowSchedule`.
   * @throws            If the spec or market JSON is malformed, or schedule construction fails.
   * @returns JSON-encoded `CashFlowSchedule`.
   * @throws If the spec or market JSON is malformed, or schedule construction fails.
   */
  buildCashflowScheduleJson(specJson: string, marketJson?: string | null): string;

  /**
   * Validate a cashflow schedule JSON string and return it canonicalized.
   *
   * @param scheduleJson JSON-encoded `CashFlowSchedule`.
   * @returns            Canonicalized JSON-encoded `CashFlowSchedule`.
   * @throws             If the schedule JSON is malformed or fails validation.
   * @returns Canonicalized JSON-encoded `CashFlowSchedule`.
   * @throws If the schedule JSON is malformed or fails validation.
   */
  validateCashflowScheduleJson(scheduleJson: string): string;

  /**
   * Extract dated flows from a cashflow schedule JSON string.
   *
   * @param scheduleJson - JSON-encoded `CashFlowSchedule`.
   * @returns JSON array of settlement cash entries. PIK and
   *   `DefaultedNotional` state rows are omitted; parse the full schedule JSON
   *   when flow classification is required.
   * @throws If the schedule JSON is malformed.
   */
  datedFlowsJson(scheduleJson: string): string;

  /**
   * Compute accrued interest from a cashflow schedule JSON string as of a given date.
   *
   * @param scheduleJson - JSON-encoded `CashFlowSchedule`.
   * @param asOf - ISO-8601 date (YYYY-MM-DD) for the accrual snapshot.
   * @param configJson - Optional JSON-encoded `AccrualConfig` overriding defaults.
   * @returns Accrued interest in the schedule's settlement currency as a JS
   *   number. The Rust engine computes from the canonical schedule and then
   *   crosses the WASM boundary as `f64`; for large notionals, compare with an
   *   absolute tolerance scaled to the schedule notional rather than expecting
   *   decimal-string equality.
   * @throws If any JSON input is malformed or the accrual computation fails.
   */
  accruedInterestJson(scheduleJson: string, asOf: string, configJson?: string | null): number;

}

/**
 * Namespaced TypeScript entry point for cashflows APIs.
 */
export declare const cashflows: CashflowsNamespace;

// --- covenants -------------------------------------------------------------

/**
 * JSON bridge to the Rust `finstack-quant-covenants` crate.
 * @example
 * ```typescript
 * import init, { covenants } from "finstack-quant-wasm";
 * await init();
 * const api: CovenantsNamespace = covenants;
 * void api;
 * ```
 */
export interface CovenantsNamespace {
  /**
   * Validate and canonicalize a covenant spec JSON string.
   * @param specJson - JSON-serialized covenant specification to validate.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateCovenantSpec(specJson: string): string;
  /**
   * Validate and canonicalize a covenant report JSON string.
   * @param reportJson - JSON-serialized covenant evaluation report to validate.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateCovenantReport(reportJson: string): string;
  /**
   * Validate and canonicalize a covenant engine JSON string.
   * @param engineJson - JSON-serialized covenant engine and its covenant definitions.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateCovenantEngine(engineJson: string): string;
  /**
   * Evaluate a covenant engine JSON string against a JSON metric map.
   * @param engineJson - JSON-serialized covenant engine and its covenant definitions.
   * @param metricsJson - JSON object of financial metrics referenced by the covenant engine.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  evaluateEngine(engineJson: string, metricsJson: string, asOf: string): string;
  /**
   * Standard leveraged-buyout covenant package as JSON.
   * @param initialLeverage - Maximum leverage ratio permitted at the initial test date.
   * @param interestCoverage - Minimum EBITDA-to-cash-interest coverage ratio.
   * @param fixedChargeCoverage - Minimum EBITDA-to-fixed-charges coverage ratio.
   * @param maxCapex - Maximum capital expenditure amount or ratio in the covenant convention.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  lboStandard(
    initialLeverage: number,
    interestCoverage: number,
    fixedChargeCoverage: number,
    maxCapex: number
  ): string;
  /**
   * Covenant-lite package as JSON.
   * @param maxLeverage - Maximum total debt-to-EBITDA leverage ratio.
   * @param maxSeniorLeverage - Maximum senior-debt-to-EBITDA leverage ratio.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  covLite(maxLeverage: number, maxSeniorLeverage: number): string;
  /**
   * Real-estate covenant package as JSON.
   * @param minDscr - Minimum debt-service coverage ratio.
   * @param minDebtYield - Minimum net-operating-income debt yield expressed as a decimal.
   * @param maxLtv - Maximum loan-to-value ratio expressed as a decimal.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  realEstate(minDscr: number, minDebtYield: number, maxLtv: number): string;
  /**
   * Project-finance covenant package as JSON.
   * @param minDscr - Minimum debt-service coverage ratio.
   * @param distributionLockupDscr - DSCR threshold below which borrower distributions are locked up.
   * @param minLiquidity - Minimum required liquidity reserve in the model's monetary units.
   * @param maxNetLeverage - Maximum net-debt-to-EBITDA leverage ratio.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  projectFinance(
    minDscr: number,
    distributionLockupDscr: number,
    minLiquidity: number,
    maxNetLeverage: number
  ): string;
}

/**
 * Namespaced TypeScript entry point for covenants APIs.
 */
export declare const covenants: CovenantsNamespace;

// --- valuations ------------------------------------------------------------

export declare class Market {
  constructor(json: string);
  toJson(): string;
}

/**
 * TypeScript view of the typed `Bond` WebAssembly instrument.
 *
 * Thin wrapper over the canonical Rust `Bond`. Serialize with `toJson()` and
 * pass the result to `valuations.instruments.priceInstrument` (or the other
 * generic pricing entry points) to price it.
 */
export interface Bond extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Returns the requested string representation or JSON payload.
   */
  readonly id: string;
  /**
   * Serialize to tagged instrument JSON (`{"type": "bond", "spec": ...}`).
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  toJson(): string;
}

/**
 * Constructor surface for the typed `Bond` WebAssembly instrument.
 * @example
 * ```typescript
 * import init, { core, valuations } from "finstack-quant-wasm";
 * await init();
 * const usd = new core.Currency("USD");
 * const bond = valuations.instruments.Bond.fixed(
 *   "BOND-1",
 *   new core.Money(1_000_000, usd),
 *   new core.Rate(0.05),
 *   "2024-01-01",
 *   "2034-01-01",
 *   "USD-OIS"
 * );
 * const result = valuations.instruments.priceInstrument(bond.toJson(), marketJson, "2024-06-30", "default");
 * ```
 */
export interface BondConstructor {
  /**
   * Create a standard fixed-rate bond (semi-annual, 30/360, T+2). Mirrors Rust `Bond::fixed`.
   * @param id - Unique instrument identifier.
   * @param notional - Principal amount of the bond.
   * @param couponRate - Annual coupon rate.
   * @param issue - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param discountCurveId - Discount curve identifier used for pricing.
   * @returns Returns the resulting typed instrument or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  fixed(
    id: string,
    notional: Money,
    couponRate: Rate,
    issue: string,
    maturity: string,
    discountCurveId: string
  ): Bond;
  /**
   * Create a floating-rate bond (FRN) linked to a forward index. Mirrors Rust `Bond::floating`.
   * @param id - Unique instrument identifier.
   * @param notional - Principal amount of the bond.
   * @param indexId - Forward curve identifier (e.g. `"USD-SOFR-3M"`).
   * @param marginBp - Spread over the index in basis points.
   * @param issue - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param freq - Payment frequency (e.g. `Tenor.quarterly()`).
   * @param dc - Day count convention (e.g. `DayCount.act360()`).
   * @param discountCurveId - Discount curve identifier used for pricing.
   * @returns Returns the resulting typed instrument or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  floating(
    id: string,
    notional: Money,
    indexId: string,
    marginBp: Bps,
    issue: string,
    maturity: string,
    freq: Tenor,
    dc: DayCount,
    discountCurveId: string
  ): Bond;
  /**
   * Deserialize a bond from tagged instrument JSON (`{"type": "bond", "spec": ...}`).
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the resulting typed instrument or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  fromJson(json: string): Bond;
}

/**
 * TypeScript view of the typed `TermLoan` WebAssembly instrument.
 *
 * Thin wrapper over the canonical Rust `TermLoan`. Serialize with `toJson()`
 * and pass the result to `valuations.instruments.priceInstrument` (or the
 * other generic pricing entry points) to price it.
 */
export interface TermLoan extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Returns the requested string representation or JSON payload.
   */
  readonly id: string;
  /**
   * Serialize to tagged instrument JSON (`{"type": "term_loan", "spec": ...}`).
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  toJson(): string;
}

/**
 * Constructor surface for the typed `TermLoan` WebAssembly instrument.
 *
 * Rust has no `fixed`/`floating` convenience constructors for term loans;
 * construct via `fromJson` with tagged JSON or start from `example()`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const loan = valuations.instruments.TermLoan.example();
 * const result = valuations.instruments.priceInstrument(loan.toJson(), marketJson, "2024-06-30", "default");
 * ```
 */
export interface TermLoanConstructor {
  /**
   * Deserialize a term loan from tagged instrument JSON (`{"type": "term_loan", "spec": ...}`).
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the resulting typed instrument or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  fromJson(json: string): TermLoan;
  /**
   * Canonical example term loan (mirrors Rust `TermLoan::example`).
   * @returns Returns the resulting typed instrument or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  example(): TermLoan;
}

/**
 * Namespaced TypeScript entry points for valuation instruments calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const api: ValuationInstrumentsNamespace = valuations.instruments;
 * void api;
 * ```
 */
export interface ValuationInstrumentsNamespace {
  /**
   * Typed `Bond` instrument class (see `BondConstructor`).
   */
  Bond: BondConstructor;
  /**
   * Typed `TermLoan` instrument class (see `TermLoanConstructor`).
   */
  TermLoan: TermLoanConstructor;
  /**
   * Construct tagged bond instrument JSON from a cashflow schedule.
   * @param instrumentId - Stable instrument identifier used for pricing and metric keys.
   * @param scheduleJson - Canonical cashflow-schedule JSON used to construct the fixed-income instrument.
   * @param discountCurveId - Market-context discount-curve identifier for the instrument currency.
   * @param quotedClean - Optional observed clean bond price in the schedule's documented price quotation convention.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  bondFromCashflowsJson(
    instrumentId: string,
    scheduleJson: string,
    discountCurveId: string,
    quotedClean?: number | null
  ): string;
  /**
   * Validate a tagged instrument JSON string.
   *
   * Deserializes the input against the known instrument schema and
   * returns the canonical (re-serialized) JSON.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateInstrumentJson(json: string): string;
  /**
   * Price an instrument from its tagged JSON and return a ValuationResult JSON.
   *
   * Pass `model = "default"` to use the instrument-native default model.
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit for the instrument-native model (matches the Python binding's `model="default"`).
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceInstrument(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    model?: string | null,
  ): string;
  /**
   * Price an instrument with explicit metric requests.
   *
   * Omit `model` (or pass `"default"`) for the instrument-native default
   * model, and omit `metrics` for none — matching the Python binding's
   * `model="default"`, `metrics=[]` defaults.
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit for the instrument-native model.
   * @param metrics - Optional array of canonical metric identifiers to calculate with the instrument price.
   * @param pricingOptions - Optional JSON pricing overrides accepted by the canonical instrument validator.
   * @param marketHistory - Optional serialized historical market snapshots required by historical pricing models.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceInstrumentWithMetrics(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    model?: string | null,
    metrics?: string[] | null,
    pricingOptions?: string | null,
    marketHistory?: string | null
  ): string;
  /**
   * Price an instrument using a pre-parsed [`Market`].
   *
   * Avoids the per-call market-parse overhead of `priceInstrument`.
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param market - Market context or JSON payload supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceInstrumentWithMarket(
    instrumentJson: string,
    market: Market,
    asOf: string,
    model: string
  ): string;
  /**
   * Price an instrument with explicit metric requests using a pre-parsed [`Market`].
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param market - Market context or JSON payload supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
   * @param metrics - Array of canonical metric identifiers to calculate with the instrument price.
   * @param pricingOptions - Optional JSON pricing overrides accepted by the canonical instrument validator.
   * @param marketHistory - Optional serialized historical market snapshots required by historical pricing models.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceInstrumentWithMetricsAndMarket(
    instrumentJson: string,
    market: Market,
    asOf: string,
    model: string,
    metrics: string[],
    pricingOptions?: string | null,
    marketHistory?: string | null
  ): string;
  /**
   * Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.
   *
   * `model` must be `"discounting"` or `"hazard_rate"`. Unsupported models or
   * incompatible instrument types throw. For supported pairs, the envelope's
   * `total_pv` matches the instrument's `base_value` within rounding.
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  instrumentCashflowsJson(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    model: string
  ): string;
  /**
   * Per-flow cashflow envelope using a pre-parsed [`Market`].
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param market - Market context or JSON payload supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  instrumentCashflowsWithMarket(
    instrumentJson: string,
    market: Market,
    asOf: string,
    model: string
  ): string;
  /**
   * List every pricing model key registered in the standard pricer registry.
   *
   * The list is registry-derived rather than enum-derived, so it reflects real
   * dispatch coverage: a model with no registered pricer is omitted. Returns a
   * sorted array of canonical keys (`"discounting"`, `"black76"`, …) accepted
   * by the `model` argument of `priceInstrument`.
   * @returns Returns the resulting `string[]` collection in ascending model-key order.
   * @throws Error - Thrown when the registry cannot be serialized for the JavaScript boundary.
   */
  listModels(): string[];
  /**
   * List the standard registry's pricing models grouped by instrument type.
   *
   * Returns a JSON object `{ instrument_type: [model_key, ...], ... }`. Only
   * instrument types with at least one registered pricer appear, and each
   * entry lists only the models that can actually price that instrument.
   * @returns Returns the resulting `Record<string, string[]>` value keyed by instrument type.
   * @throws Error - Thrown when the registry cannot be serialized for the JavaScript boundary.
   */
  listModelsGrouped(): Record<string, string[]>;
  /**
   * List all metric IDs in the standard metric registry.
   * @returns Returns the resulting `string[]` collection in the documented order.
   */
  listStandardMetrics(): string[];
  /**
   * List all standard metrics organized by group.
   *
   * Returns a JSON object `{ group_name: [metric_id, ...], ... }` where
   * each key is a human-readable group name (e.g. "Pricing", "Greeks",
   * "Sensitivity") and the value is a sorted array of metric ID strings.
   * @returns Returns the resulting `Record<string, string[]>` value or WebAssembly handle.
   */
  listStandardMetricsGrouped(): Record<string, string[]>;
  /**
   * Z-spread-equivalent discount margin for a floating-rate tranche, returned in
   * decimal units (`0.015` = 150 bp).
   *
   * Contractual cashflows are projected without changing coupon projection,
   * then a constant additive spread is applied to the discount curve. The result
   * is zero at model PV, negative for a richer (higher) `targetPv`, and positive
   * for a cheaper (lower) `targetPv`; it is not the contractual quoted margin.
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param trancheId - Identifier of the floating-rate tranche whose contractual cashflows are spread-discounted.
   * @param marketJson - Canonical market-context JSON supplying the discount curve and any forward curves or historical fixings required for cashflow projection.
   * @param asOf - ISO-8601 valuation date used for projection and discounting.
   * @param targetPv - Target present value in the tranche's currency; values above model PV produce a negative result and values below model PV produce a positive result.
   * @returns The z-spread-equivalent discount margin in decimal units.
   * @throws Error - Thrown if JSON or the date is malformed, the deal is invalid, the tranche is missing or fixed-rate, targetPv is non-finite, required market data is unavailable, or the spread solve fails or exceeds ±5000 bp.
   */
  structuredCreditTrancheDiscountMargin(
    instrumentJson: string,
    trancheId: string,
    marketJson: string,
    asOf: string,
    targetPv: number
  ): number;
  /**
   * Break-even constant default rate (CDR, decimal) for a tranche — the highest
   * CDR at which the tranche takes no principal writedown.
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  structuredCreditTrancheBreakevenCdr(
    instrumentJson: string,
    trancheId: string,
    marketJson: string,
    asOf: string
  ): number;
  /**
   * Option-adjusted spread for a tranche; returns a JSON `OasResult`.
   *
   * `marketPricePct` is the quoted price as a percentage of original balance.
   * `config`, when present, is a JSON `OasConfig`; the default is used otherwise.
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketPricePct - Tranche market price as a percentage of original balance.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param config - Optional OasConfig JSON; omit to use the default OAS solver configuration.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  structuredCreditTrancheOas(
    instrumentJson: string,
    trancheId: string,
    marketPricePct: number,
    marketJson: string,
    asOf: string,
    config?: string | null
  ): string;
  /**
   * Scenario (CPR x CDR x severity) table for a tranche; returns a JSON
   * `ScenarioTable`. `grid` is a JSON `ScenarioGrid` (`cprs`, `cdrs`,
   * `severities`).
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param grid - ScenarioGrid JSON containing the CPR, CDR, and severity axes for the table.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  structuredCreditTrancheScenarioTable(
    instrumentJson: string,
    trancheId: string,
    marketJson: string,
    asOf: string,
    grid: string
  ): string;
  /**
   * Per-tranche risk/spread metrics (PV, price, WAL, z-spread, CS01, spread/
   * modified duration, convexity) computed from one tranche's own cashflows.
   *
   * `marketPricePct`, when provided, is the quoted price (% of original balance)
   * the z-spread and CS01 are solved against; otherwise the tranche's own model
   * price is used (zero z-spread). Returns a JSON-serialized `TrancheMetrics`.
   * @param instrumentJson - Canonical JSON payload representing the instrument consumed by this API.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param marketPricePct - Optional tranche market price as a percentage of original balance; omit for model price.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  structuredCreditTrancheMetrics(
    instrumentJson: string,
    trancheId: string,
    marketJson: string,
    asOf: string,
    marketPricePct?: number | null
  ): string;
}

/**
 * TypeScript type that constrains the accepted fx instrument spec values.
 */
export type FxInstrumentSpec = Record<string, unknown> | string;

/**
 * TypeScript view of the `FxInstrument` WebAssembly value.
 */
export interface FxInstrument extends WasmOwned {
  /**
   * Serialize this `FxInstrument` value to canonical JSON.
   * @returns Returns the requested string representation or JSON payload.
   */
  toJson(): string;
  /**
   * Perform price for this `FxInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  price(marketJson: string, asOf: string, model?: string | null): string;
  /**
   * WASM order keeps optional arguments trailing: metrics precedes model.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param metrics - Metric keys or values included in the requested calculation.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @param pricingOptions - Pricing options that select calculation behavior and output detail.
   * @param marketHistory - Chronological market snapshots used to project or backtest the result.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  priceWithMetrics(
    marketJson: string,
    asOf: string,
    metrics: string[],
    model?: string | null,
    pricingOptions?: string | null,
    marketHistory?: string | null
  ): string;
}

/**
 * TypeScript view of the `FxOptionInstrument` WebAssembly value.
 */
export interface FxOptionInstrument extends FxInstrument {
  /**
   * Perform delta for this `FxOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  delta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform gamma for this `FxOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  gamma(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform vega for this `FxOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  vega(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform theta for this `FxOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  theta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform rho for this `FxOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rho(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform foreign rho for this `FxOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  foreignRho(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform vanna for this `FxOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  vanna(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform volga for this `FxOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  volga(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform greeks for this `FxOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the resulting `Record<string, number>` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  greeks(marketJson: string, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * TypeScript view of the `FxDigitalOptionInstrument` WebAssembly value.
 */
export interface FxDigitalOptionInstrument extends FxInstrument {
  /**
   * Perform delta for this `FxDigitalOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  delta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform gamma for this `FxDigitalOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  gamma(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform vega for this `FxDigitalOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  vega(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform theta for this `FxDigitalOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  theta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform rho for this `FxDigitalOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rho(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform greeks for this `FxDigitalOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the resulting `Record<string, number>` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  greeks(marketJson: string, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * TypeScript view of the `FxTouchOptionInstrument` WebAssembly value.
 */
export interface FxTouchOptionInstrument extends FxInstrument {
  /**
   * Perform delta for this `FxTouchOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  delta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform gamma for this `FxTouchOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  gamma(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform vega for this `FxTouchOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  vega(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform rho for this `FxTouchOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rho(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform greeks for this `FxTouchOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the resulting `Record<string, number>` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  greeks(marketJson: string, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * TypeScript view of the `FxBarrierOptionInstrument` WebAssembly value.
 */
export interface FxBarrierOptionInstrument extends FxTouchOptionInstrument {
  /**
   * Perform vanna for this `FxBarrierOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  vanna(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Perform volga for this `FxBarrierOptionInstrument` value.
   * @param marketJson - JSON-serialized MarketContext whose quotes, curves, and other objects satisfy this calculation's documented requirements.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Pricing-model key selecting the valuation model implemented by the underlying Rust API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  volga(marketJson: string, asOf: string, model?: string | null): number;
}

/**
 * Construction and factory entry points for `FxInstrument` WebAssembly values.
 * @example
 * ```typescript
 * import init from "finstack-quant-wasm";
 * await init();
 * const factory: FxInstrumentConstructor = FxInstrumentConstructor;
 * void factory;
 * ```
 */
export interface FxInstrumentConstructor<T extends FxInstrument> {
  /**
   * Create a new `FxInstrument` WebAssembly value.
   * @param spec - Structured specification that defines the requested object or calculation.
   * @returns Returns the resulting `T` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  new (spec: FxInstrumentSpec): T;
  /**
   * Parse a `FxInstrument` value from canonical JSON.
   * @param json - JSON-serialized representation accepted by this API.
   * @returns Returns the resulting `T` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  fromJson(json: string): T;
}

/**
 * Namespaced TypeScript entry points for fx calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const api: FxNamespace = valuations.fx;
 * void api;
 * ```
 */
export interface FxNamespace {
  /**
   * Fx spot exposed by this `Fx` value.
   */
  FxSpot: FxInstrumentConstructor<FxInstrument>;
  /**
   * Fx forward exposed by this `Fx` value.
   */
  FxForward: FxInstrumentConstructor<FxInstrument>;
  /**
   * Fx swap exposed by this `Fx` value.
   */
  FxSwap: FxInstrumentConstructor<FxInstrument>;
  /**
   * Ndf exposed by this `Fx` value.
   */
  Ndf: FxInstrumentConstructor<FxInstrument>;
  /**
   * Fx option exposed by this `Fx` value.
   */
  FxOption: FxInstrumentConstructor<FxOptionInstrument>;
  /**
   * Fx digital option exposed by this `Fx` value.
   */
  FxDigitalOption: FxInstrumentConstructor<FxDigitalOptionInstrument>;
  /**
   * Fx touch option exposed by this `Fx` value.
   */
  FxTouchOption: FxInstrumentConstructor<FxTouchOptionInstrument>;
  /**
   * Fx barrier option exposed by this `Fx` value.
   */
  FxBarrierOption: FxInstrumentConstructor<FxBarrierOptionInstrument>;
  /**
   * Fx variance swap exposed by this `Fx` value.
   */
  FxVarianceSwap: FxInstrumentConstructor<FxInstrument>;
  /**
   * Quanto option exposed by this `Fx` value.
   */
  QuantoOption: FxInstrumentConstructor<FxOptionInstrument>;
}

// --- SABR (Stochastic Alpha Beta Rho) volatility -------------------------

/**
 * SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
 */
export interface SabrParameters extends WasmOwned {
  /**
   * SABR `alpha` (ATM volatility level).
   */
  readonly alpha: number;
  /**
   * SABR `beta` (backbone exponent).
   */
  readonly beta: number;
  /**
   * SABR `nu` (vol-of-vol).
   */
  readonly nu: number;
  /**
   * SABR `rho` (spot/vol correlation).
   */
  readonly rho: number;
  /**
   * Displacement applied for shifted SABR, if any.
   */
  readonly shift: number | undefined;
  /**
   * Whether a displacement (shift) is configured.
   * @returns Returns `true` when the documented condition is satisfied.
   */
  isShifted(): boolean;
}

/**
 * SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const factory: SabrParametersConstructor = valuations.SabrParameters;
 * void factory;
 * ```
 */
export interface SabrParametersConstructor {
  /**
   * Create the object from its inputs.
   * @param alpha - Positive SABR initial volatility scale parameter.
   * @param beta - SABR CEV elasticity parameter from 0 through 1.
   * @param nu - Positive SABR volatility-of-volatility parameter.
   * @param rho - Instantaneous correlation between the asset and variance shocks.
   * @param shift - Additive SABR rate shift applied to forward and strike before modelling.
   * @returns Returns the resulting `SabrParameters` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  new (alpha: number, beta: number, nu: number, rho: number, shift?: number): SabrParameters;
  /**
   * Equity-standard defaults `(alpha=0.20, beta=1.0, nu=0.30, rho=-0.20)`.
   * @returns Returns the resulting `SabrParameters` value or WebAssembly handle.
   */
  equityDefault(): SabrParameters;
  /**
   * Rates-standard defaults `(alpha=0.02, beta=0.5, nu=0.30, rho=0.0)`.
   * @returns Returns the resulting `SabrParameters` value or WebAssembly handle.
   */
  ratesDefault(): SabrParameters;
}

/**
 * Hagan-2002 SABR volatility model.
 */
export interface SabrModel extends WasmOwned {
  /**
   * Black implied volatility for the given strike.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  impliedVol(forward: number, strike: number, t: number): number;
  /**
   * Parameters used by this model.
   */
  readonly params: SabrParameters;
  /**
   * Whether the parameterization admits negative forwards.
   * @returns Returns `true` when the documented condition is satisfied.
   */
  supportsNegativeRates(): boolean;
}

/**
 * Hagan-2002 SABR volatility model.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const factory: SabrModelConstructor = valuations.SabrModel;
 * void factory;
 * ```
 */
export interface SabrModelConstructor {
  /**
   * Create the object from its inputs.
   * @param params - SABR parameter object containing alpha, beta, nu, rho, and optional shift.
   * @returns Returns the resulting `SabrModel` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  new (params: SabrParameters): SabrModel;
}

/**
 * TypeScript view of the `SabrSmileArbitrageResult` WebAssembly value.
 */
export interface SabrSmileArbitrageResult {
  /**
   * Arbitrage free exposed by this `SabrSmileArbitrageResult` value.
   */
  arbitrage_free: boolean;
  /**
   * Butterfly violations exposed by this `SabrSmileArbitrageResult` value.
   */
  butterfly_violations: Array<{
    strike: number;
    butterfly_value: number;
    severity_pct: number;
  }>;
  /**
   * Monotonicity violations exposed by this `SabrSmileArbitrageResult` value.
   */
  monotonicity_violations: Array<{
    strike_low: number;
    strike_high: number;
    price_low: number;
    price_high: number;
  }>;
}

/**
 * Volatility smile generator for a fixed `(forward, t)` pair.
 */
export interface SabrSmile extends WasmOwned {
  /**
   * At-the-money implied volatility.
   * @returns Returns the computed numeric result in the units described above.
   */
  atmVol(): number;
  /**
   * Black implied volatility for the given strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  impliedVol(strike: number): number;
  /**
   * Implied volatilities for a strike grid.
   * @param strikes - Option strikes at which to evaluate the SABR volatility smile.
   * @returns Returns the resulting `number[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  generateSmile(strikes: number[]): number[];
  /**
   * Butterfly + monotonicity arbitrage diagnostics.
   *
   * Returns a JSON object with `arbitrage_free`, `butterfly_violations`,
   * and `monotonicity_violations` arrays (snake_case keys matching the Rust
   * canonical fields and the Python binding).
   * @param strikes - Ordered option strikes used to test the calibrated smile for static arbitrage.
   * @param r - Continuously compounded risk-free rate, expressed as a decimal.
   * @param q - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @returns Returns the resulting `SabrSmileArbitrageResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  arbitrageDiagnostics(strikes: number[], r?: number, q?: number): SabrSmileArbitrageResult;
}

/**
 * Volatility smile generator for a fixed `(forward, t)` pair.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const factory: SabrSmileConstructor = valuations.SabrSmile;
 * void factory;
 * ```
 */
export interface SabrSmileConstructor {
  /**
   * Create the object from its inputs.
   * @param params - SABR parameter object containing alpha, beta, nu, rho, and optional shift.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @returns Returns the resulting `SabrSmile` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  new (params: SabrParameters, forward: number, t: number): SabrSmile;
}

/**
 * SABR calibrator (Levenberg-Marquardt with beta fixed).
 */
export interface SabrCalibrator extends WasmOwned {
  /**
   * Return a copy of this calibrator with an overridden convergence
   * tolerance, preserving all other settings (e.g. the iteration cap from
   * `highPrecision`).
   * @param tolerance - Non-negative numerical convergence tolerance for the calibration optimizer.
   * @returns Returns the resulting `SabrCalibrator` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  withTolerance(tolerance: number): SabrCalibrator;
  /**
   * Calibrate `(alpha, nu, rho)` to market vols with `beta` fixed.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strikes - Option strikes aligned one-for-one with market_vols.
   * @param marketVols - Market-implied annualized volatilities aligned one-for-one with strikes.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @param beta - SABR CEV elasticity parameter held fixed during calibration.
   * @returns Returns the resulting `SabrParameters` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  calibrate(
    forward: number,
    strikes: number[],
    marketVols: number[],
    t: number,
    beta: number
  ): SabrParameters;
  /**
   * Calibrate with automatic shift selection for negative-rate smiles.
   *
   * When the forward or any strike is negative, a shifted-SABR fit is
   * performed with an automatically chosen shift; otherwise this behaves
   * like `calibrate`.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strikes - Option strikes aligned one-for-one with market_vols.
   * @param marketVols - Market-implied annualized volatilities aligned one-for-one with strikes.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @param beta - SABR CEV elasticity parameter held fixed during calibration.
   * @returns Returns the resulting `SabrParameters` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  calibrateAutoShift(
    forward: number,
    strikes: number[],
    marketVols: number[],
    t: number,
    beta: number
  ): SabrParameters;
}

/**
 * SABR calibrator (Levenberg-Marquardt with beta fixed).
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const factory: SabrCalibratorConstructor = valuations.SabrCalibrator;
 * void factory;
 * ```
 */
export interface SabrCalibratorConstructor {
  /**
   * Create the object from its inputs.
   * @returns Returns the resulting `SabrCalibrator` value or WebAssembly handle.
   */
  new (): SabrCalibrator;
  /**
   * Calibrator preset with tighter convergence tolerances.
   * @returns Returns the resulting `SabrCalibrator` value or WebAssembly handle.
   */
  highPrecision(): SabrCalibrator;
}

/**
 * Namespaced TypeScript entry points for valuation credit calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const api: ValuationCreditNamespace = valuations.credit;
 * void api;
 * ```
 */
export interface ValuationCreditNamespace {
  /**
   * Build a structural Merton model JSON payload.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal.
   * @param debtBarrier - Positive debt face value defining the structural-model default barrier.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  mertonModelJson(
    assetValue: number,
    assetVol: number,
    debtBarrier: number,
    riskFreeRate: number
  ): string;
  /**
   * Build a CreditGrades structural model JSON payload.
   * @param equityValue - Current market value of equity in the firm's monetary units.
   * @param equityVol - Annualized equity-return volatility expressed as a decimal.
   * @param totalDebt - Total debt face value in the firm's monetary units.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param barrierUncertainty - Lognormal dispersion of the CreditGrades default barrier, not a generic uncertainty score.
   * @param meanRecovery - Mean recovery rate at default expressed as a fraction from 0 through 1.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  creditGradesModelJson(
    equityValue: number,
    equityVol: number,
    totalDebt: number,
    riskFreeRate: number,
    barrierUncertainty: number,
    meanRecovery: number
  ): string;
  /**
   * Compute structural default probability from model JSON.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  mertonDefaultProbability(modelJson: string, horizon: number): number;
  /**
   * Compute distance-to-default from a Merton model JSON payload.
   *
   * Distance-to-default is `ln(V/B)/(sigma*sqrt(T))` plus drift adjustments.
   * Lower values indicate higher default risk.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  mertonDistanceToDefault(modelJson: string, horizon: number): number;
  /**
   * Compute the implied credit spread (per year) from a Merton model JSON
   * payload, given a recovery rate. Matches the structural-model-implied
   * spread used to back into a hazard curve.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  mertonImpliedSpread(modelJson: string, horizon: number, recovery: number): number;
  /**
   * Evaluate a `DynamicRecoverySpec` JSON payload at a given accreted
   * notional, returning the implied recovery rate. Result is clamped to
   * `[0, base_recovery]`.
   * @param specJson - Serialized DynamicRecoverySpec JSON defining the notional-to-recovery mapping.
   * @param notional - Signed trade notional in the instrument's native currency units.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  dynamicRecoveryAtNotional(specJson: string, notional: number): number;
  /**
   * Evaluate an `EndogenousHazardSpec` JSON payload at a given leverage
   * level, returning the implied hazard rate. Floored at 0.
   * @param specJson - Serialized EndogenousHazardSpec JSON defining the leverage-to-hazard mapping.
   * @param leverage - Debt-to-assets leverage ratio used by the structural credit model.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  endogenousHazardAtLeverage(specJson: string, leverage: number): number;
  /**
   * Convenience evaluator: hazard rate after a PIK accrual updates the
   * outstanding notional. Computes leverage = `accreted_notional / asset_value`
   * then evaluates the hazard mapping.
   * @param specJson - Serialized EndogenousHazardSpec JSON defining the leverage-to-hazard mapping.
   * @param accretedNotional - Outstanding notional after PIK accrual, in the debt's monetary units.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  endogenousHazardAfterPikAccrual(
    specJson: string,
    accretedNotional: number,
    assetValue: number
  ): number;
  /**
   * Build a constant dynamic-recovery spec JSON payload.
   * @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  dynamicRecoveryConstantJson(recovery: number): string;
  /**
   * Build an endogenous hazard power-law spec JSON payload.
   * @param baseHazard - Reference annual default intensity used by the leverage-to-hazard mapping.
   * @param baseLeverage - Positive reference debt-to-assets leverage ratio for the hazard mapping.
   * @param exponent - Positive exponent controlling sensitivity in the documented power-law mapping.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  endogenousHazardPowerLawJson(baseHazard: number, baseLeverage: number, exponent: number): string;
  /**
   * Build a credit-state JSON payload for toggle-exercise decisions.
   *
   * Parameter order follows the canonical Rust `CreditState` field order
   * (and the Python binding): `hazardRate`, `distanceToDefault`, `leverage`,
   * `accretedNotional`, `couponDue`, `assetValue`.
   * @param hazardRate - Annualized instantaneous default intensity, expressed as a decimal.
   * @param distanceToDefault - Optional distance to default, measured as standard deviations from the default point.
   * @param leverage - Debt-to-assets leverage ratio used by the structural credit model.
   * @param accretedNotional - Outstanding notional after PIK accrual, in the debt's monetary units.
   * @param couponDue - Cash coupon amount due at the toggle decision date, in debt monetary units.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  creditStateJson(
    hazardRate: number,
    distanceToDefault: number | null | undefined,
    leverage: number,
    accretedNotional: number,
    couponDue: number,
    assetValue?: number | null
  ): string;
  /**
   * Build a threshold toggle-exercise model JSON payload.
   * @param variable - Credit-state variable: `"hazard_rate"`, `"distance_to_default"`, or `"leverage"`.
   * @param threshold - Threshold value in the units of the selected credit-state variable.
   * @param direction - Threshold comparison: `"above"` selects PIK above the level and `"below"` below it.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  toggleExerciseThresholdJson(
    variable: 'hazard_rate' | 'distance_to_default' | 'leverage',
    threshold: number,
    direction: 'above' | 'below'
  ): string;
  /**
   * Build an optimal toggle-exercise model JSON payload.
   *
   * `nested_paths` is the Monte-Carlo path count for the nested optimal-exercise
   * simulation. It is rejected if it exceeds `Number.MAX_SAFE_INTEGER` (`2^53-1`):
   * `usize` counts marshal across the wasm boundary as IEEE-754 doubles, so a
   * larger value would round silently rather than fail loudly.
   * @param nestedPaths - Number of nested Monte Carlo paths for continuation-value estimation; must fit JavaScript's safe integer range.
   * @param equityDiscountRate - Annual equity-holder discount rate used in the nested toggle decision.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param horizon - Forward-looking model horizon measured in years.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  toggleExerciseOptimalJson(
    nestedPaths: number,
    equityDiscountRate: number,
    assetVol: number,
    riskFreeRate: number,
    horizon: number
  ): string;
}

/**
 * Namespaced TypeScript entry points for credit derivatives calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const api: CreditDerivativesNamespace = valuations.creditDerivatives;
 * void api;
 * ```
 */
export interface CreditDerivativesNamespace {
  /**
   * Example tagged `CreditDefaultSwap` instrument JSON.
   * @returns Returns the requested string representation or JSON payload.
   */
  creditDefaultSwapExampleJson(): string;
  /**
   * Example tagged `CDSIndex` instrument JSON.
   * @returns Returns the requested string representation or JSON payload.
   */
  cdsIndexExampleJson(): string;
  /**
   * Example tagged `CDSTranche` instrument JSON.
   * @returns Returns the requested string representation or JSON payload.
   */
  cdsTrancheExampleJson(): string;
  /**
   * Example tagged `CDSOption` instrument JSON.
   * @returns Returns the requested string representation or JSON payload.
   */
  cdsOptionExampleJson(): string;
}

/**
 * Namespaced TypeScript entry points for valuations calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const api: ValuationsNamespace = valuations;
 * void api;
 * ```
 */
export interface ValuationsNamespace {
  /**
   * Pearson correlation coefficient.
   * @param x - Numeric observation series aligned one-for-one with the other series.
   * @param y - Numeric observation series aligned one-for-one with the other series.
   */
  correlation: CorrelationNamespace;
  /**
   * Structural credit models and toggle-exercise helpers.
   */
  credit: ValuationCreditNamespace;
  /**
   * CDS-family JSON wrappers and pricing helpers.
   */
  creditDerivatives: CreditDerivativesNamespace;
  /**
   * Direct FX instrument wrappers.
   */
  fx: FxNamespace;
  /**
   * Instrument JSON validation and pricing helpers.
   */
  instruments: ValuationInstrumentsNamespace;
  /**
   * Deserialize a `ValuationResult` from JSON and return the canonical JSON.
   *
   * Validates the input conforms to the `ValuationResult` schema.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateValuationResultJson(json: string): string;
  /**
   * Validate a calibration plan JSON and return the canonical (pretty-printed) form.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @param envelope - Calibration envelope containing the plan, market data, and optional prior market objects.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateCalibrationJson(envelope: CalibrationEnvelope | string): string;
  /**
   * Execute a `CalibrationEnvelope` and return the full `CalibrationResultEnvelope`.
   * Accepts either a typed object or a pre-serialized JSON string.
   * The canonical path for building a `MarketContext` from quotes — the resulting
   * `result.final_market` is a materialized state ready for `MarketContext::try_from`
   * (Rust) or `result.market` (Python).
   *
   * @throws Error with `name = "CalibrationEnvelopeError"` and structured `cause`
   *   (e.g. `e.cause.kind === "solver_not_converged"`) on calibration failure.
   *
   * ⚠️ BLOCKING: calibration may be CPU-heavy. Wrap calls in an application
   * timeout until `timeout_ms` is carried by the calibration envelope schema.
   * @param envelope_json - CalibrationEnvelope JSON containing targets, parameters, bounds, and dependencies.
   * @param envelope - Calibration envelope containing the plan, market data, and optional prior market objects.
   * @returns Returns the resulting `CalibrationResultEnvelope` value or WebAssembly handle.
   */
  calibrate(envelope: CalibrationEnvelope | string): CalibrationResultEnvelope;
  /**
   * Pre-flight envelope validation without invoking the solver.
   *
   * Returns a JSON-serialized `ValidationReport` listing every error found
   * plus the dependency graph. Microseconds.
   * @param envelope_json - CalibrationEnvelope JSON containing targets, parameters, bounds, and dependencies.
   * @throws Error with `name = "CalibrationEnvelopeError"` if the envelope JSON is malformed.
   * @param envelope - Calibration envelope containing the plan, market data, and optional prior market objects.
   * @returns Returns the requested string representation or JSON payload.
   */
  dryRun(envelope: CalibrationEnvelope | string): string;
  /**
   * Returns the static dependency graph of a calibration plan as JSON.
   * @param envelope_json - CalibrationEnvelope JSON containing targets, parameters, bounds, and dependencies.
   * @throws Error with `name = "CalibrationEnvelopeError"` if the envelope JSON is malformed.
   * @param envelope - Calibration envelope containing the plan, market data, and optional prior market objects.
   * @returns Returns the requested string representation or JSON payload.
   */
  dependencyGraphJson(envelope: CalibrationEnvelope | string): string;
  /**
   * Market exposed by this `Valuations` value.
   */
  Market: typeof Market;
  /**
   * Per-unit Black-Scholes / Garman-Kohlhagen price of a European option.
   *
   * @param spot - Spot price of the underlying.
   * @param strike - Strike of the option.
   * @param r - Risk-free rate, **decimal** continuously compounded
   * (e.g. `0.05` for 5%).
   * @param q - Continuous dividend yield (or foreign rate for FX),
   * **decimal** continuously compounded.
   * @param sigma - Annualized volatility, **decimal**
   * (e.g. `0.20` for 20%).
   * @param t - Time to expiry in **years**.
   * @param isCall - `true` for a call, `false` for a put.
   * @returns Per-unit option price.
   *
   * @example
   * ```javascript
   * import init, { valuations } from "finstack-quant-wasm";
   * await init();
   * const price = valuations.bsPrice(
   *   100,    // spot
   *   100,    // strike (ATM)
   *   0.05,   // r = 5%
   *   0.0,    // q = 0
   *   0.20,   // sigma = 20%
   *   1.0,    // 1 year
   *   true,   // call
   * );
   * // price ≈ 10.45
   * ```
   *
   * @throws If the inputs produce a non-finite price (e.g. negative volatility).
   */
  bsPrice(
    spot: number,
    strike: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    isCall: boolean
  ): number;
  /**
   * Black-Scholes / Garman-Kohlhagen Greeks as a `{delta, gamma, vega, theta, rho, rho_q}` object.
   *
   * @param spot - Spot price of the underlying.
   * @param strike - Strike of the option.
   * @param r - Risk-free rate, **decimal** continuously compounded.
   * @param q - Dividend yield (or foreign rate for FX), **decimal**
   * continuously compounded.
   * @param sigma - Annualized volatility, **decimal**.
   * @param t - Time to expiry in **years**.
   * @param isCall - `true` for a call, `false` for a put.
   * @param thetaDays - Day-count denominator for theta. Default `365`.
   * Pass `252` for trading-day theta.
   * @returns Object `{ delta, gamma, vega, theta, rho, rho_q }` (snake_case keys
   * matching the Rust/Python canonical names). `vega` and
   * both rho values are **per 1% move**; `theta` is **per day** under
   * `thetaDays`.
   * @throws If serialization to JS fails (should not happen on valid inputs).
   *
   * @example
   * ```javascript
   * const g = valuations.bsGreeks(100, 100, 0.05, 0.0, 0.20, 1.0, true);
   * // g.delta ≈ 0.64, g.gamma ≈ 0.019, g.vega ≈ 0.38 (per 1% vol)
   * ```
   */
  bsGreeks(
    spot: number,
    strike: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    isCall: boolean,
    thetaDays?: number
  ): {
    delta: number;
    gamma: number;
    vega: number;
    theta: number;
    rho: number;
    rho_q: number;
  };
  /**
   * Solve for Black-Scholes / Garman-Kohlhagen implied volatility.
   *
   * @param spot - Spot price of the underlying.
   * @param strike - Strike of the option.
   * @param r - Risk-free rate, **decimal** continuously compounded.
   * @param q - Dividend yield, **decimal** continuously compounded.
   * @param t - Time to expiry in **years**.
   * @param price - Observed option price (per unit).
   * @param isCall - `true` for a call, `false` for a put.
   * @returns Annualized implied volatility, **decimal** (e.g. `0.20`).
   * @throws If `price` is below intrinsic value, above the no-arbitrage
   * upper bound, or the solver fails to converge.
   *
   * @example
   * ```javascript
   * const iv = valuations.bsImpliedVol(100, 100, 0.05, 0.0, 1.0, 10.45, true);
   * // iv ≈ 0.20
   * ```
   */
  bsImpliedVol(
    spot: number,
    strike: number,
    r: number,
    q: number,
    t: number,
    price: number,
    isCall: boolean
  ): number;
  /**
   * Solve for Black-76 (forward-based) implied volatility.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param df - Discount factor from valuation to expiry, expressed as a positive decimal.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @param price - Price in the documented quote convention for this instrument.
   * @param isCall - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  black76ImpliedVol(
    forward: number,
    strike: number,
    df: number,
    t: number,
    price: number,
    isCall: boolean
  ): number;
  /**
   * Reiner-Rubinstein continuous-monitoring barrier call price.
   *
   * `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param barrier - Continuously monitored barrier level in the same price units as spot.
   * @param r - Continuously compounded risk-free rate, expressed as a decimal.
   * @param q - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @param direction - Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.
   * @param knock - Barrier activation: `"in"` for knock-in or `"out"` for knock-out.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  barrierCall(
    spot: number,
    strike: number,
    barrier: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    direction: 'up' | 'down',
    knock: 'in' | 'out'
  ): number;
  /**
   * Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param r - Continuously compounded risk-free rate, expressed as a decimal.
   * @param q - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @param numFixings - Positive number of equally spaced averaging observations before expiry.
   * @param averaging - Asian averaging convention: `"arithmetic"` (default) or `"geometric"`.
   * @param isCall - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  asianOptionPrice(
    spot: number,
    strike: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    numFixings: number,
    averaging?: 'arithmetic' | 'geometric',
    isCall?: boolean
  ): number;
  /**
   * Conze-Viswanathan lookback option.
   *
   * `strike_type` is `"fixed"` (default) or `"floating"`. For `"floating"`,
   * `strike` is ignored and `extremum` is the observed min/max to date.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param r - Continuously compounded risk-free rate, expressed as a decimal.
   * @param q - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @param extremum - Observed running minimum for a call or maximum for a put, in spot-price units.
   * @param strikeType - Lookback payoff convention: `"fixed"` (default) or `"floating"`.
   * @param isCall - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  lookbackOptionPrice(
    spot: number,
    strike: number,
    r: number,
    q: number,
    sigma: number,
    t: number,
    extremum: number,
    strikeType?: 'fixed' | 'floating',
    isCall?: boolean
  ): number;
  /**
   * Quanto option (FX-adjusted cross-currency) price in domestic currency.
   *
   * @throws If the inputs produce a non-finite price.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param t - Time from the curve base date in years on the documented day-count basis.
   * @param rateDomestic - Domestic continuously compounded risk-free rate, expressed as a decimal.
   * @param rateForeign - Foreign continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param volAsset - Annualized asset-price volatility expressed as a decimal.
   * @param volFx - Annualized FX-rate volatility expressed as a decimal.
   * @param correlation - Instantaneous correlation between the documented asset and FX-rate shocks, from -1 to 1.
   * @param isCall - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
   * @returns Returns the computed numeric result in the units described above.
   */
  quantoOptionPrice(
    spot: number,
    strike: number,
    t: number,
    rateDomestic: number,
    rateForeign: number,
    divYield: number,
    volAsset: number,
    volFx: number,
    correlation: number,
    isCall?: boolean
  ): number;
  /**
   * SABR parameters `(alpha, beta, nu, rho)` with optional `shift`.
   */
  SabrParameters: SabrParametersConstructor;
  /**
   * Hagan-2002 SABR volatility model.
   */
  SabrModel: SabrModelConstructor;
  /**
   * SABR smile generator for a fixed `(forward, t)` pair.
   */
  SabrSmile: SabrSmileConstructor;
  /**
   * Levenberg-Marquardt SABR calibrator (beta fixed).
   */
  SabrCalibrator: SabrCalibratorConstructor;
  /**
   * Price a European option under the Black-Scholes model using the COS method.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param dividend - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param maturity - Time to option expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
   * @param nTerms - Optional positive number of COS expansion terms; omit to use the pricer default.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  bsCosPrice(
    spot: number,
    strike: number,
    rate: number,
    dividend: number,
    vol: number,
    maturity: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /**
   * Price a European option under the Variance Gamma model using the COS method.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param dividend - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param theta - Variance-Gamma drift parameter controlling skew in log returns.
   * @param nu - Variance-Gamma variance-rate parameter; larger values increase tail thickness.
   * @param maturity - Time to option expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
   * @param nTerms - Optional positive number of COS expansion terms; omit to use the pricer default.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  vgCosPrice(
    spot: number,
    strike: number,
    rate: number,
    dividend: number,
    sigma: number,
    theta: number,
    nu: number,
    maturity: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /**
   * Price a European option under Merton (1976) jump-diffusion using the COS method.
   * @param spot - Current spot price or exchange rate in the documented quote convention.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param dividend - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param muJump - Mean log jump size in the Merton jump-diffusion model.
   * @param sigmaJump - Standard deviation of log jump sizes in the Merton jump-diffusion model.
   * @param lambda - Annual jump-arrival intensity in the Merton jump-diffusion model.
   * @param maturity - Time to option expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
   * @param nTerms - Optional positive number of COS expansion terms; omit to use the pricer default.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  mertonJumpCosPrice(
    spot: number,
    strike: number,
    rate: number,
    dividend: number,
    sigma: number,
    muJump: number,
    sigmaJump: number,
    lambda: number,
    maturity: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /**
   * Simulated TARN coupon profile along a deterministic floating-rate path.
   *
   * Returns a JSON object:
   * ```text
   * {
   *   "coupons_paid": number[],
   *   "cumulative":   number[],
   *   "redemption_index": number | null,
   *   "redeemed_early":   boolean
   * }
   * ```
   *
   * Each period's coupon is `max(fixed_rate - L_i, coupon_floor) * day_count_fraction`.
   * Payments accumulate in a
   * [`CumulativeCouponTracker`](finstack_quant_valuations::instruments::rates::exotics_shared::cumulative_coupon::CumulativeCouponTracker) configured with
   * `target_coupon`; once cumulative hits the target, the final coupon is
   * capped and the instrument is considered redeemed.
   * @param fixedRate - Fixed coupon rate in decimal form before subtracting each floating fixing.
   * @param couponFloor - Minimum period coupon rate in decimal form after the TARN rate calculation.
   * @param floatingFixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
   * @param targetCoupon - Cumulative coupon target, as a fraction of notional, that redeems the TARN.
   * @param dayCountFraction - Accrual year fraction applied to each coupon period.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  tarnCouponProfile(
    fixedRate: number,
    couponFloor: number,
    floatingFixings: number[],
    targetCoupon: number,
    dayCountFraction: number
  ): {
    coupons_paid: number[];
    cumulative: number[];
    redemption_index: number | null;
    redeemed_early: boolean;
  };
  /**
   * Snowball coupon schedule.
   *
   *   `c_i = clip(c_{i-1} + fixed_rate - L_i, floor, cap)` with `c_0 = initial_coupon`.
   * @param initialCoupon - Starting coupon rate before the first snowball update, in decimal form.
   * @param fixedRate - Fixed coupon rate in decimal form added at each snowball step.
   * @param floatingFixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
   * @param floor - Minimum permitted coupon rate in decimal form.
   * @param cap - Maximum permitted coupon rate in decimal form.
   * @returns Returns the resulting `number[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  snowballCouponProfile(
    initialCoupon: number,
    fixedRate: number,
    floatingFixings: number[],
    floor: number,
    cap: number
  ): number[];
  /**
   * Path-independent inverse-floater coupon schedule.
   * @param fixedRate - Fixed coupon rate in decimal form before the leveraged floating deduction.
   * @param floatingFixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
   * @param floor - Minimum permitted coupon rate in decimal form.
   * @param cap - Maximum permitted coupon rate in decimal form.
   * @param leverage - Positive multiplier applied to each floating fixing in the inverse-floater coupon.
   * @returns Returns the resulting `number[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  inverseFloaterCouponProfile(
    fixedRate: number,
    floatingFixings: number[],
    floor: number,
    cap: number,
    leverage: number
  ): number[];
  /**
   * Intrinsic (undiscounted, unhedged) payoff of a CMS spread option.
   *
   * `call:  notional * max(long_cms - short_cms - strike, 0)`
   * `put:   notional * max(strike - (long_cms - short_cms), 0)`
   * @param longCms - Long-tenor CMS rate in decimal form.
   * @param shortCms - Short-tenor CMS rate in decimal form.
   * @param strike - CMS rate-spread strike in decimal form.
   * @param isCall - Whether to value a call (`true`) or put (`false`); defaults follow the callable's contract.
   * @param notional - Signed trade notional in the instrument's native currency units.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  cmsSpreadOptionIntrinsic(
    longCms: number,
    shortCms: number,
    strike: number,
    isCall: boolean,
    notional: number
  ): number;
  /**
   * Accrued coupon on a range-accrual leg over a set of observations.
   *
   * Counts the fraction of observations with a rate in the inclusive interval
   * `[lower, upper]` and scales by the period day-count fraction:
   *
   * `accrued = coupon_rate * day_count_fraction * (#in-range / #observations)`.
   *
   * The call provision is not applied here.
   * @param lower - Inclusive lower bound of the observed-rate range, in decimal form.
   * @param upper - Inclusive upper bound of the observed-rate range, in decimal form.
   * @param observations - Observed floating rates in decimal form for the accrual period.
   * @param couponRate - Contractual coupon rate in decimal form before range weighting.
   * @param dayCountFraction - Accrual year fraction for the coupon period.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  callableRangeAccrualAccrued(
    lower: number,
    upper: number,
    observations: number[],
    couponRate: number,
    dayCountFraction: number
  ): number;
}

/**
 * Namespaced TypeScript entry point for valuations APIs.
 */
export declare const valuations: ValuationsNamespace;

// --- attribution -----------------------------------------------------------

/**
 * Parameters for P&L attribution via [`attribute_pnl`].
 */
export interface AttributionParams extends WasmOwned {}

/**
 * Namespaced TypeScript entry points for attribution calculations and types.
 * @example
 * ```typescript
 * import init, { attribution } from "finstack-quant-wasm";
 * await init();
 * const api: AttributionNamespace = attribution;
 * void api;
 * ```
 */
export interface AttributionNamespace {
  /**
   * Parameters constructor emitted by wasm-bindgen for attribution calls.
   *
   * `configJson` may include `{ "execution_policy": "serial" }` when the host
   * already parallelizes attribution at the portfolio or batch level.
   */
  AttributionParams: new (
    instrumentJson: string,
    marketT0Json: string,
    marketT1Json: string,
    asOfT0: string,
    asOfT1: string,
    methodJson: string,
    configJson?: string,
    fullCrossAttribution?: boolean
  ) => AttributionParams;
  /**
   * Run P&L attribution for a single instrument.
   *
   * Accepts an [`AttributionParams`] struct with the instrument JSON, two market
   * snapshots, dates, and a method descriptor. Returns the `PnlAttribution`
   * result as JSON. `config_json` may include `"execution_policy": "serial"`
   * for hosts that already parallelize attribution at a higher level.
   * @param params - Fully specified AttributionParams object containing instrument, markets, dates, and method.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  attributePnl(params: AttributionParams): string;
  /**
   * Run attribution from a full JSON `AttributionEnvelope` and return JSON.
   *
   * Power-user variant for full envelope round-trip workflows.
   * @param specJson - JSON-serialized AttributionParams specification to validate and execute.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  attributePnlFromSpec(specJson: string): string;
  /**
   * Validate an attribution specification JSON.
   *
   * Deserializes against the `AttributionEnvelope` schema, checks the
   * `schema` version tag (the same gate `execute` applies, so a payload that
   * validates here cannot later be rejected at execution), and returns the
   * canonical JSON.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateAttributionJson(json: string): string;
  /**
   * Return the default waterfall factor ordering as a JSON array.
   * @returns Returns the resulting `string[]` collection in the documented order.
   */
  defaultWaterfallOrder(): string[];
  /**
   * Return the default metric IDs used by metrics-based attribution.
   * @returns Returns the resulting `string[]` collection in the documented order.
   */
  defaultAttributionMetrics(): string[];
}

/**
 * Namespaced TypeScript entry point for attribution APIs.
 */
export declare const attribution: AttributionNamespace;

// --- statements ------------------------------------------------------------

/**
 * Namespaced TypeScript entry points for statements calculations and types.
 * @example
 * ```typescript
 * import init, { statements } from "finstack-quant-wasm";
 * await init();
 * const api: StatementsNamespace = statements;
 * void api;
 * ```
 */
export interface StatementsNamespace {
  /**
   * Validate a `FinancialModelSpec` JSON string.
   *
   * Deserializes the input against the model schema, runs semantic validation,
   * and returns the canonical (re-serialized) JSON.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateFinancialModelJson(json: string): string;
  /**
   * Get the node identifiers from a model specification JSON.
   *
   * Returns a JS array of node ID strings in declaration order.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the resulting `string[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  modelNodeIds(json: string): string[];
  /**
   * Validate a `CheckSuiteSpec` JSON string.
   *
   * Deserializes the spec, re-serializes to canonical form, and
   * returns the JSON string. Useful for client-side validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateCheckSuiteSpec(json: string): string;
  /**
   * Validate a `CapitalStructureSpec` JSON string.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateCapitalStructureSpec(json: string): string;
  /**
   * Validate a `WaterfallSpec` JSON string.
   *
   * Performs both serde deserialization and the waterfall's internal
   * consistency check (for example rejecting `Sweep` ordered after `Equity`
   * when an ECF sweep is configured).
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateWaterfallSpec(json: string): string;
  /**
   * Validate an `EcfSweepSpec` JSON string.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateEcfSweepSpec(json: string): string;
  /**
   * Validate a `PikToggleSpec` JSON string.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validatePikToggleSpec(json: string): string;
  /**
   * Evaluate a `FinancialModelSpec` and return the `StatementResult` JSON.
   * @param modelJson - JSON-serialized FinancialModelSpec to evaluate across its statement periods.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  evaluateModel(modelJson: string): string;
  /**
   * Evaluate a `FinancialModelSpec` against a `MarketContext` as of a given date.
   *
   * Required for capital-structure-aware models. The `as_of` argument is an
   * ISO 8601 date string (e.g. `"2025-01-15"`).
   * @param modelJson - JSON-serialized FinancialModelSpec to evaluate across its statement periods.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  evaluateModelWithMarket(modelJson: string, marketJson: string, asOf: string): string;
  /**
   * Parse a DSL formula and return a debug string for its AST.
   *
   * Useful for previewing expression structure in UI tooling before
   * committing a formula to a model.
   * @param formula - Financial-model formula string to parse into its canonical expression representation.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  parseFormula(formula: string): string;
  /**
   * Validate that a DSL formula parses and compiles successfully.
   *
   * Returns `true` when the formula is valid; throws a `FinstackError`
   * otherwise. This mirrors the Python `validate_formula` API.
   * @param formula - Financial-model formula string to parse and validate without evaluation.
   * @returns Returns `true` when the documented condition is satisfied.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateFormula(formula: string): boolean;
}

/**
 * Namespaced TypeScript entry point for statements APIs.
 */
export declare const statements: StatementsNamespace;

// --- statements_analytics -------------------------------------------------

/**
 * TypeScript view of the `GoalSeekResult` WebAssembly value.
 */
export interface GoalSeekResult {
  /**
   * Solved value exposed by this `GoalSeekResult` value.
   */
  solved_value: number;
  /**
   * Updated model json exposed by this `GoalSeekResult` value.
   */
  updated_model_json?: string;
}

/**
 * Namespaced TypeScript entry points for statements analytics calculations and types.
 * @example
 * ```typescript
 * import init, { statements_analytics } from "finstack-quant-wasm";
 * await init();
 * const api: StatementsAnalyticsNamespace = statements_analytics;
 * void api;
 * ```
 */
export interface StatementsAnalyticsNamespace {
  /**
   * Run a sensitivity analysis on a financial model.
   *
   * Accepts JSON strings for the model spec and sensitivity configuration,
   * evaluates all perturbation scenarios, and returns JSON results.
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param configJson - Canonical JSON payload representing the config consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  runSensitivity(modelJson: string, configJson: string): string;
  /**
   * Run a variance analysis comparing two evaluated statement results.
   *
   * Returns JSON-serialized variance report.
   * @param baseJson - Canonical JSON payload representing the base consumed by this API.
   * @param comparisonJson - Canonical JSON payload representing the comparison consumed by this API.
   * @param configJson - Canonical JSON payload representing the config consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  runVariance(baseJson: string, comparisonJson: string, configJson: string): string;
  /**
   * Evaluate all scenarios in a scenario set against a base model.
   *
   * Returns a JSON object mapping scenario names to their statement results.
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param scenarioSetJson - Canonical JSON payload representing the scenario set consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  evaluateScenarioSet(modelJson: string, scenarioSetJson: string): string;
  /**
   * Compute forecast accuracy metrics (MAE, MAPE, RMSE).
   *
   * Takes two float arrays (actual, forecast) and returns a JSON object
   * with keys `mae`, `mape`, `rmse`, `n`.
   * @param actual - Actual realized values aligned one-for-one with the forecast series.
   * @param forecast - Forecast values aligned one-for-one with the actual realized series.
   * @returns Returns the resulting `BacktestForecastMetricsJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  backtestForecast(actual: number[], forecast: number[]): BacktestForecastMetricsJson;
  /**
   * Generate tornado chart entries for a sensitivity result.
   * @param resultJson - Canonical JSON payload representing the result consumed by this API.
   * @param metricNode - Statement metric node identifier selected for the requested analysis.
   * @param period - Model period label for the requested statement value or calculation.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  generateTornadoEntries(resultJson: string, metricNode: string, period?: string): string;
  /**
   * Run Monte Carlo simulation on a financial model (JSON in/out).
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param configJson - Canonical JSON payload representing the config consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  runMonteCarlo(modelJson: string, configJson: string): string;
  /**
   * Find the driver value that makes a target node reach a target value.
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param targetNode - Statement node identifier whose value is driven toward the target.
   * @param targetPeriod - Model period label in which the goal-seek target is evaluated.
   * @param targetValue - Numeric target value the goal-seek routine attempts to reach.
   * @param driverNode - Statement node identifier adjusted by the goal-seek routine.
   * @param driverPeriod - Model period label of the adjustable goal-seek driver.
   * @param updateModel - Whether to return the model with the solved driver value applied.
   * @param boundsLo - Lower numeric bound allowed for the goal-seek driver.
   * @param boundsHi - Upper numeric bound allowed for the goal-seek driver.
   * @returns Returns the resulting `GoalSeekResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  goalSeek(
    modelJson: string,
    targetNode: string,
    targetPeriod: string,
    targetValue: number,
    driverNode: string,
    driverPeriod: string,
    updateModel: boolean,
    boundsLo?: number | null,
    boundsHi?: number | null
  ): GoalSeekResult;
  /**
   * Rank the headline DCF assumptions by enterprise-value impact.
   *
   * The statement model is evaluated once; each shocked point re-runs only the
   * DCF. Returns JSON with the baseline enterprise value, tornado entries as
   * deltas versus that baseline sorted by descending absolute swing, and the
   * effective (possibly clamped) shock levels.
   * @param modelJson - Canonical JSON payload representing the financial model spec consumed by this API.
   * @param wacc - Baseline weighted average cost of capital in decimal form (0.10 = 10%).
   * @param terminalValueJson - Canonical JSON payload representing the terminal value spec, selecting whether growth or the exit multiple is shocked.
   * @param ufcfNode - Node identifier holding unlevered free cash flow for the forecast periods.
   * @param netDebtOverride - Optional flat net-debt amount used instead of the model-derived bridge.
   * @param waccSensitivityBump - Absolute shock applied to WACC and to the terminal growth rate, in decimal (0.01 = +/-100 bp).
   * @param waccDenominatorEpsilon - Minimum spread preserved between WACC and the terminal growth rate so 1/(wacc - g) stays defined, in decimal.
   * @param exitMultipleBump - Absolute shock applied to an exit multiple, in turns of the multiple (1.0 = +/-1.0x).
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  dcfSensitivity(
    modelJson: string,
    wacc: number,
    terminalValueJson: string,
    ufcfNode: string,
    netDebtOverride?: number | null,
    waccSensitivityBump?: number | null,
    waccDenominatorEpsilon?: number | null,
    exitMultipleBump?: number | null
  ): string;
  /**
   * Evaluate a leveraged-buyout transaction against a statement model.
   *
   * Entry enterprise value is priced at the model's first period, the sponsor
   * equity check is solved as the sources-and-uses residual, and exit proceeds
   * are the exit enterprise value less the modelled net debt at the exit
   * period. IRR is out of scope: pair the returned `exit_equity_proceeds` with
   * the equity outflow at close and call `portfolio.mwrXirr`.
   * @param modelJson - Canonical JSON payload representing the financial model spec consumed by this API.
   * @param entryMultiple - Entry valuation multiple applied to the entry metric (8.5 = 8.5x).
   * @param entryMetricNode - Node identifier supplying the entry valuation metric, read at the model's first period.
   * @param exitMultiple - Exit valuation multiple applied to the exit metric (9.5 = 9.5x).
   * @param exitMetricNode - Node identifier supplying the exit valuation metric, read at the exit period.
   * @param exitNetDebtNode - Node identifier supplying net debt outstanding at the exit period, where a modelled amortisation schedule lands.
   * @param exitPeriod - Model period label at which the sponsor exits, e.g. "2029".
   * @param sourcesJson - Canonical JSON array of funded debt tranches at close, each {"name", "amount"} in the model currency.
   * @param transactionFees - Transaction fees and expenses funded at close, in the model currency.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  evaluateLbo(
    modelJson: string,
    entryMultiple: number,
    entryMetricNode: string,
    exitMultiple: number,
    exitMetricNode: string,
    exitNetDebtNode: string,
    exitPeriod: string,
    sourcesJson: string,
    transactionFees: number
  ): string;
  /**
   * Weighted-average cost of capital (WACC).
   *
   * Blends the required return on equity with the after-tax cost of debt:
   * `WACC = w_E * r_E + w_D * r_D * (1 - T)`.
   * @param equityWeight - Equity share of total capital as a decimal fraction (0.6 = 60% equity-funded).
   * @param costOfEquity - Required return on equity in decimal form, typically from CAPM (0.115 = 11.5%).
   * @param debtWeight - Debt share of total capital as a decimal fraction; must sum with the equity weight to 1.0.
   * @param costOfDebt - Pre-tax marginal borrowing yield in decimal form, before the interest tax shield (0.06 = 6%).
   * @param taxRate - Marginal corporate tax rate as a decimal fraction in [0, 1] (0.25 = 25%).
   * @returns Returns the blended discount rate as a decimal fraction.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  wacc(
    equityWeight: number,
    costOfEquity: number,
    debtWeight: number,
    costOfDebt: number,
    taxRate: number
  ): number;
  /**
   * Trace dependencies for a node and return ASCII tree.
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param nodeId - Stable node identifier used to select the required domain object.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  traceDependencies(modelJson: string, nodeId: string): string;
  /**
   * Explain a formula for a specific node and period (JSON in/out).
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param resultsJson - Canonical JSON payload representing the results consumed by this API.
   * @param nodeId - Stable node identifier used to select the required domain object.
   * @param period - Model period label for the requested statement value or calculation.
   * @returns Returns the resulting `FormulaExplanationJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  explainFormula(
    modelJson: string,
    resultsJson: string,
    nodeId: string,
    period: string
  ): FormulaExplanationJson;
  /**
   * Explain a formula for a specific node and period as formatted text.
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param resultsJson - Canonical JSON payload representing the results consumed by this API.
   * @param nodeId - Stable node identifier used to select the required domain object.
   * @param period - Model period label for the requested statement value or calculation.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  explainFormulaText(
    modelJson: string,
    resultsJson: string,
    nodeId: string,
    period: string
  ): string;
  /**
   * Generate a P&L summary report as formatted text.
   * @param resultsJson - Canonical JSON payload representing the results consumed by this API.
   * @param lineItems - Ordered statement line-item definitions included in the summary report.
   * @param periods - Ordered period labels or observations aligned with the supplied data.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  plSummaryReport(resultsJson: string, lineItems: string[], periods: string[]): string;
  /**
   * Generate a credit assessment report as formatted text.
   * @param resultsJson - Canonical JSON payload representing the results consumed by this API.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  creditAssessmentReport(resultsJson: string, asOf: string): string;
  /**
   * Run checks from a suite spec against a model (JSON in/out).
   *
   * Evaluates the model, resolves the suite spec into runnable checks
   * (built-in **and** user-defined formula checks), and returns a JSON
   * check report.
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param suiteSpecJson - Canonical JSON payload representing the suite spec consumed by this API.
   * @param resultsJson - Canonical JSON payload representing the results consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  runChecks(modelJson: string, suiteSpecJson: string, resultsJson?: string | null): string;
  /**
   * Run three-statement checks using node mappings.
   *
   * Accepts a model and a mapping JSON, builds the appropriate check
   * suite, evaluates the model, runs the checks, and returns the report.
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param mappingJson - Canonical JSON payload representing the mapping consumed by this API.
   * @param resultsJson - Canonical JSON payload representing the results consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  runThreeStatementChecks(
    modelJson: string,
    mappingJson: string,
    resultsJson?: string | null
  ): string;
  /**
   * Run credit underwriting checks using credit-specific mappings.
   * @param modelJson - Canonical JSON payload representing the model consumed by this API.
   * @param mappingJson - Canonical JSON payload representing the mapping consumed by this API.
   * @param resultsJson - Canonical JSON payload representing the results consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  runCreditUnderwritingChecks(
    modelJson: string,
    mappingJson: string,
    resultsJson?: string | null
  ): string;
  /**
   * Render a check report as plain text.
   * @param reportJson - Canonical JSON payload representing the report consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  renderCheckReportText(reportJson: string): string;
  /**
   * Render a check report as HTML.
   * @param reportJson - Canonical JSON payload representing the report consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  renderCheckReportHtml(reportJson: string): string;
  // Comps — comparable company analysis
  /**
   * Percentile rank of `value` within `data` on a 0-1 scale.
   *
   * Returns `null` when `data` is empty rather than a synthetic 0.5.
   * @param value - Subject-company metric value to rank against the peer sample.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  percentileRank(value: number, data: number[]): number | null;
  /**
   * Z-score of `value` within `data`.
   *
   * Returns `null` when fewer than two observations are provided or the
   * peer variance is zero, instead of a synthetic zero.
   * @param value - Subject-company metric value to standardize against the peer sample.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  zScore(value: number, data: number[]): number | null;
  /**
   * Descriptive statistics over a peer distribution.
   *
   * Returns `null` (matching the other comps helpers) when `data` is empty.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  peerStats(data: number[]): PeerStatsJson | null;
  /**
   * Single-factor OLS fit of `y` on `x` evaluated at the subject observation.
   * @param xValues - Comparable-company independent-variable values aligned with y_values.
   * @param yValues - Comparable-company dependent-variable values aligned with x_values.
   * @param subjectX - Subject company's independent-variable value for the fitted regression.
   * @param subjectY - Subject company's observed dependent-variable value for relative-value comparison.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  regressionFairValue(
    xValues: number[],
    yValues: number[],
    subjectX: number,
    subjectY: number
  ): RegressionResultJson | null;
  /**
   * Compute a canonical valuation multiple for a company-metric bag.
   * @param companyMetrics - Company financial-metric object supplying numerator and denominator inputs.
   * @param multiple - Supported valuation multiple identifier, such as EV/EBITDA or P/E.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  computeMultiple(companyMetrics: unknown, multiple: string): number | null;
  /**
   * Composite rich/cheap scoring across multiple dimensions.
   * @param peerSet - Comparable-company metric records used to score relative value.
   * @param dimensions - Metric dimensions and weights included in the relative-value score.
   * @returns Returns the resulting `RelativeValueResultJson` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  scoreRelativeValue(peerSet: unknown, dimensions: unknown[]): RelativeValueResultJson;
}

/**
 * Namespaced TypeScript entry point for statements analytics APIs.
 */
export declare const statements_analytics: StatementsAnalyticsNamespace;

// --- portfolio -------------------------------------------------------------

/**
 * TypeScript view of the `ScenarioRevalueResult` WebAssembly value.
 */
export interface ScenarioRevalueResult {
  /**
   * Valuation exposed by this `ScenarioRevalueResult` value.
   */
  valuation: Record<string, unknown>;
  /**
   * Report exposed by this `ScenarioRevalueResult` value.
   */
  report: Record<string, unknown>;
}

/**
 * Scenario-attributable profit and loss together with the scenario
 * application report.
 */
export interface ScenarioPnlResult {
  /**
   * Profit-and-loss ladder: base-currency `total` plus a `by_position`
   * map of per-position base-currency amounts. Positions added or removed
   * by the scenario are zero-filled against the missing side, so
   * `by_position` always sums to `total`.
   */
  pnl: Record<string, unknown>;
  /**
   * Report exposed by this `ScenarioPnlResult` value.
   */
  report: Record<string, unknown>;
}

/**
 * Typed handle to a built portfolio. Construct once via
 * `Portfolio.fromSpec` and reuse it across cashflow / valuation calls to
 * skip the per-call `PortfolioSpec` parse + rebuild cost.
 */
export declare class Portfolio {
  private constructor();
  static fromSpec(specJson: string): Portfolio;
  readonly id: string;
  readonly asOf: string;
  readonly baseCcy: string;
  numPositions(): number;
  toSpecJson(): string;
  /** Release the underlying wasm heap allocation. Do not use this handle after calling `free()`. */
  free(): void;
}

/**
 * Namespaced TypeScript entry points for portfolio calculations and types.
 * @example
 * ```typescript
 * import init, { portfolio } from "finstack-quant-wasm";
 * await init();
 * const api: PortfolioNamespace = portfolio;
 * void api;
 * ```
 */
export interface PortfolioNamespace {
  /**
   * Typed handle for cached portfolio builds.
   */
  Portfolio: typeof Portfolio;
  /**
   * Parse and validate a portfolio specification from JSON.
   *
   * Returns the re-serialized canonical JSON form.
   * @param jsonStr - Canonical JSON string to validate, parse, or normalize for this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  parsePortfolioSpec(jsonStr: string): string;
  /**
   * Compute a single-period Brinson-Fachler attribution from sector JSON.
   *
   * Accepts a JSON array of `SectorPeriod` objects and returns a JSON
   * `BrinsonPeriodResult`.
   * @param sectorsJson - Canonical JSON payload representing the sectors consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  brinsonFachler(sectorsJson: string): string;
  /**
   * Compute Carino-linked multi-period Brinson attribution from period JSON.
   *
   * Accepts a JSON array of periods, where each period is an array of
   * `SectorPeriod` objects, and returns a JSON `CarinoLinkedAttribution`.
   * @param periodsJson - Canonical JSON payload representing the periods consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  carinoLink(periodsJson: string): string;
  /**
   * Compute a Modified-Dietz TWRR sub-period return from period JSON.
   * @param periodJson - Canonical JSON payload representing the period consumed by this API.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  twrrModifiedDietz(periodJson: string): number | undefined;
  /**
   * Geometrically link TWRR sub-period returns from returns JSON.
   * @param returnsJson - Canonical JSON payload representing the returns consumed by this API.
   * @param horizonYears - Return-linking horizon measured in years for annualization.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  twrrLinked(returnsJson: string, horizonYears: number): string | undefined;
  /**
   * Compute money-weighted return via XIRR from dated cashflow JSON.
   * @param cashflowsJson - Canonical JSON payload representing the cashflows consumed by this API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  mwrXirr(cashflowsJson: string): number;
  /**
   * Build a runtime portfolio from a JSON spec, validate, and round-trip.
   *
   * Deserializes the spec, constructs the portfolio with live instruments,
   * validates structural invariants, then re-serializes for confirmation.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  buildPortfolioFromSpec(specJson: string): string;
  /**
   * Extract the total portfolio value from a JSON result.
   * @param resultJson - Canonical JSON payload representing the result consumed by this API.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  portfolioResultTotalValue(resultJson: string): number;
  /**
   * Extract a specific metric from a portfolio result JSON.
   *
   * Returns `undefined` (via `Option`) if the metric was not produced.
   * @param resultJson - Canonical JSON payload representing the result consumed by this API.
   * @param metricId - Stable metric identifier used to select the required domain object.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  portfolioResultGetMetric(resultJson: string, metricId: string): number | undefined;
  /**
   * Aggregate portfolio metrics from a valuation JSON.
   * @param valuationJson - Canonical JSON payload representing the valuation consumed by this API.
   * @param baseCcy - ISO-4217 base currency in which aggregate portfolio values are reported.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  aggregateMetrics(
    valuationJson: string,
    baseCcy: string,
    marketJson: string,
    asOf: string
  ): string;
  /**
   * Value a portfolio from its spec and market context.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param strictRisk - Whether unavailable risk metrics are treated as calculation errors.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  valuePortfolio(specJson: string, marketJson: string, strictRisk: boolean): string;
  /**
   * Value an already-built [`Portfolio`] handle. Skips the per-call
   * `PortfolioSpec` parse + `Portfolio::from_spec` rebuild that
   * [`value_portfolio`] performs; use this when sweeping market scenarios
   * against a fixed portfolio.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param strictRisk - Whether unavailable risk metrics are treated as calculation errors.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  valuePortfolioBuilt(portfolio: Portfolio, marketJson: string, strictRisk: boolean): string;
  /**
   * Aggregate the full classified cashflow ladder for a portfolio.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  aggregateFullCashflows(specJson: string, marketJson: string): string;
  /**
   * Aggregate the full classified cashflow ladder for an already-built
   * [`Portfolio`] handle.
   *
   * Skips the per-call `PortfolioSpec` parse + `Portfolio::from_spec` rebuild.
   * For batched or chained workflows (repeated cashflow builds across market
   * scenarios on the same portfolio), this is the cheap path.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  aggregateFullCashflowsBuilt(portfolio: Portfolio, marketJson: string): string;
  /**
   * Apply a scenario to a portfolio and revalue.
   *
   * Returns a JS object with structured `valuation` and `report` values.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param scenarioJson - Canonical JSON payload representing the scenario consumed by this API.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @returns Returns the resulting `ScenarioRevalueResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  applyScenarioAndRevalue(
    specJson: string,
    scenarioJson: string,
    marketJson: string
  ): ScenarioRevalueResult;
  /**
   * Apply a scenario to an already-built [`Portfolio`] handle and revalue.
   * Returns a JS object with structured `valuation` and `report` values.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param scenarioJson - Canonical JSON payload representing the scenario consumed by this API.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @returns Returns the resulting `ScenarioRevalueResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  applyScenarioAndRevalueBuilt(
    portfolio: Portfolio,
    scenarioJson: string,
    marketJson: string
  ): ScenarioRevalueResult;
  /**
   * Compute the profit and loss attributable to a scenario.
   *
   * Values the portfolio against the unshocked market and against the
   * scenario-shocked market, and returns a JS object with structured `pnl`
   * (base-currency `total` plus `by_position`) and `report` values.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param scenarioJson - Canonical JSON payload representing the scenario whose profit-and-loss impact is measured.
   * @param marketJson - Canonical market-context JSON supplying the unshocked curves, quotes, and FX data used for the base leg.
   * @returns Returns the resulting `ScenarioPnlResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  scenarioPnl(
    specJson: string,
    scenarioJson: string,
    marketJson: string
  ): ScenarioPnlResult;
  /**
   * Compute the profit and loss attributable to a scenario for an
   * already-built [`Portfolio`] handle.
   *
   * Values the portfolio against the unshocked market and against the
   * scenario-shocked market, and returns a JS object with structured `pnl`
   * (base-currency `total` plus `by_position`) and `report` values. Positions
   * added or removed by the scenario are zero-filled against the missing side,
   * so the drill-down always sums to the total.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param scenarioJson - Canonical JSON payload representing the scenario whose profit-and-loss impact is measured.
   * @param marketJson - Canonical market-context JSON supplying the unshocked curves, quotes, and FX data used for the base leg.
   * @returns Returns the resulting `ScenarioPnlResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  scenarioPnlBuilt(
    portfolio: Portfolio,
    scenarioJson: string,
    marketJson: string
  ): ScenarioPnlResult;
  /**
   * Optimize portfolio weights using the LP-based optimizer.
   *
   * Accepts a `PortfolioOptimizationSpec` JSON (portfolio + objective +
   * constraints + options) and a `MarketContext` JSON.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  optimizePortfolio(specJson: string, marketJson: string): string;
  /**
   * Replay a portfolio through dated market snapshots.
   *
   * Accepts a portfolio spec, an array of dated market snapshots, and a
   * replay configuration. Returns a JSON-serialized `ReplayResult`.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param snapshotsJson - Canonical JSON payload representing the snapshots consumed by this API.
   * @param configJson - Canonical JSON payload representing the config consumed by this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  replayPortfolio(specJson: string, snapshotsJson: string, configJson: string): string;
  /**
   * Decompose portfolio VaR into position contributions via parametric Euler
   * allocation. Inputs mirror the Python binding's signature.
   *
   * `covariance_json` must deserialize to an `n x n` row-major nested array.
   * @param positionIdsJson - Canonical JSON payload representing the position ids consumed by this API.
   * @param weightsJson - Canonical JSON payload representing the weights consumed by this API.
   * @param covarianceJson - Canonical JSON payload representing the covariance consumed by this API.
   * @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  parametricVarDecomposition(
    positionIdsJson: string,
    weightsJson: string,
    covarianceJson: string,
    confidence: number
  ): string;
  /**
   * Decompose portfolio Expected Shortfall into position contributions via
   * parametric Euler allocation.
   *
   * Returns an ES-shaped JSON payload mirroring the Python
   * ``parametric_es_decomposition`` return value: a top-level
   * ``{portfolio_var, portfolio_es, confidence, n_positions, contributions}``
   * object whose ``contributions`` entries are
   * ``{position_id, component_es, marginal_es, pct_contribution}``.
   * @param positionIdsJson - Canonical JSON payload representing the position ids consumed by this API.
   * @param weightsJson - Canonical JSON payload representing the weights consumed by this API.
   * @param covarianceJson - Canonical JSON payload representing the covariance consumed by this API.
   * @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  parametricEsDecomposition(
    positionIdsJson: string,
    weightsJson: string,
    covarianceJson: string,
    confidence: number
  ): string;
  /**
   * Decompose portfolio VaR/ES from per-position scenario P&Ls via historical
   * simulation.
   *
   * `position_pnls_json` is a nested array shaped `[n_positions][n_scenarios]`.
   * @param positionIdsJson - Canonical JSON payload representing the position ids consumed by this API.
   * @param positionPnlsJson - Canonical JSON payload representing the position pnls consumed by this API.
   * @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  historicalVarDecomposition(
    positionIdsJson: string,
    positionPnlsJson: string,
    confidence: number
  ): string;
  /**
   * Evaluate a per-position risk budget against actual component VaRs.
   * @param positionIdsJson - Canonical JSON payload representing the position ids consumed by this API.
   * @param actualVarJson - Canonical JSON payload representing the actual var consumed by this API.
   * @param targetVarPctJson - Canonical JSON payload representing the target var pct consumed by this API.
   * @param portfolioVar - Total portfolio VaR used to convert risk-budget shares into absolute amounts.
   * @param utilizationThreshold - Actual-to-target risk ratio that flags a budget breach.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  evaluateRiskBudget(
    positionIdsJson: string,
    actualVarJson: string,
    targetVarPctJson: string,
    portfolioVar: number,
    utilizationThreshold: number
  ): string;
  /**
   * Effective bid-ask spread via Roll (1984). Returns `undefined` when the
   * serial covariance is non-negative (Roll assumption violated) or inputs too short.
   * @param returnsJson - Canonical JSON payload representing the returns consumed by this API.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  rollEffectiveSpread(returnsJson: string): number | undefined;
  /**
   * Amihud (2002) illiquidity ratio from returns and volumes.
   * @param returnsJson - Canonical JSON payload representing the returns consumed by this API.
   * @param volumesJson - Canonical JSON payload representing the volumes consumed by this API.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  amihudIlliquidity(returnsJson: string, volumesJson: string): number | undefined;
  /**
   * Trading days required to liquidate at the given participation rate.
   * @param positionValue - Current position market value in the relevant currency units.
   * @param avgDailyVolume - Average daily trading volume in the same units as the position size.
   * @param participationRate - Maximum fraction of average daily volume used for execution.
   * @returns Returns the computed numeric result in the units described above.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  daysToLiquidate(positionValue: number, avgDailyVolume: number, participationRate: number): number;
  /**
   * Classify a position into a liquidity tier from its days-to-liquidate.
   *
   * Uses the default `[1, 5, 20, 60]` trading-day thresholds. Returns one of
   * `"tier1" .. "tier5"`.
   * @param daysToLiquidate - Days to liquidate supplied to liquidity tier; follow the type and convention required by the surrounding API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  liquidityTier(daysToLiquidate: number): string;
  /**
   * Liquidity-adjusted VaR following Bangia, Diebold, Schuermann & Stroughair (1999).
   * Loss sign convention: `var` and `lvar` are non-positive.
   * @param var - Base market value-at-risk before adding the liquidity adjustment.
   * @param spreadMean - Mean bid-ask spread in the quote units required by the liquidity model.
   * @param spreadVol - Volatility of the bid-ask spread in the liquidity model's units.
   * @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
   * @param positionValue - Current position market value in the relevant currency units.
   * @param varValue - Value-at-Risk level or estimate consumed by this calculation.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  lvarBangia(
    varValue: number,
    spreadMean: number,
    spreadVol: number,
    confidence: number,
    positionValue: number
  ): string;
  /**
   * Almgren-Chriss (2001) market impact decomposition for a uniform execution.
   * @param positionSize - Trade size in shares or notional units for the execution calculation.
   * @param avgDailyVolume - Average daily trading volume in the same units as the position size.
   * @param volatility - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param executionHorizonDays - Planned execution horizon measured in trading days.
   * @param permanentImpactCoef - Permanent market-impact coefficient in the execution-cost model.
   * @param temporaryImpactCoef - Temporary market-impact coefficient in the execution-cost model.
   * @param referencePrice - Optional reference price used to express execution impact in monetary units.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  almgrenChrissImpact(
    positionSize: number,
    avgDailyVolume: number,
    volatility: number,
    executionHorizonDays: number,
    permanentImpactCoef: number,
    temporaryImpactCoef: number,
    referencePrice?: number | null
  ): string;
  /**
   * Kyle (1985) linear price impact lambda estimated from observed volumes
   * and returns via the Amihud-ratio proxy. Returns `undefined` on invalid inputs.
   * @param volumesJson - Canonical JSON payload representing the volumes consumed by this API.
   * @param returnsJson - Canonical JSON payload representing the returns consumed by this API.
   * @returns Returns the result using the declared TypeScript shape.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  kyleLambda(volumesJson: string, returnsJson: string): number | undefined;
  /**
   * Compute first-order factor sensitivities and return the matrix as JSON.
   *
   * Accepts a JSON array of positions, a JSON array of `FactorDefinition`,
   * a `MarketContext` JSON, an ISO 8601 date, and an optional `BumpSizeConfig`
   * JSON.  Returns a JSON object with `position_ids`, `factor_ids`, and a
   * row-major `data` matrix.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  computeFactorSensitivities(
    positionsJson: string,
    factorsJson: string,
    marketJson: string,
    asOf: string,
    bumpConfigJson?: string
  ): string;
  /**
   * Compute first-order factor sensitivities using a pre-parsed [`Market`].
   *
   * Avoids reparsing market JSON for repeated factor analytics calls.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param market - Market context or JSON payload supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  computeFactorSensitivitiesWithMarket(
    positionsJson: string,
    factorsJson: string,
    market: Market,
    asOf: string,
    bumpConfigJson?: string
  ): string;
  /**
   * Compute scenario P&L profiles via full repricing and return as JSON.
   *
   * Same position/factor/market inputs as `computeFactorSensitivities`, plus
   * an optional `n_scenario_points` integer.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @param nScenarioPoints - Positive number of evenly spaced bump levels in each P-and-L profile.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  computePnlProfiles(
    positionsJson: string,
    factorsJson: string,
    marketJson: string,
    asOf: string,
    bumpConfigJson?: string,
    nScenarioPoints?: number
  ): string;
  /**
   * Compute scenario P&L profiles using a pre-parsed [`Market`].
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param market - Market context or JSON payload supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @param nScenarioPoints - Positive number of evenly spaced bump levels in each P-and-L profile.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  computePnlProfilesWithMarket(
    positionsJson: string,
    factorsJson: string,
    market: Market,
    asOf: string,
    bumpConfigJson?: string,
    nScenarioPoints?: number
  ): string;
  /**
   * Decompose portfolio risk into factor and position contributions.
   *
   * Uses the parametric (covariance-based) Euler decomposition.  Accepts
   * a JSON sensitivity matrix (same schema as the output of
   * `computeFactorSensitivities`), a `FactorCovarianceMatrix` JSON, and an
   * optional `RiskMeasure` JSON.
   *
   * Returns a JSON object with `total_risk`, `measure`, `residual_risk`,
   * `factor_contributions` (array), and `position_factor_contributions` (array).
   * @param sensitivitiesJson - Canonical factor-sensitivity result JSON to decompose.
   * @param covarianceJson - Factor covariance-matrix JSON aligned with the supplied sensitivities.
   * @param riskMeasureJson - Risk-measure configuration JSON selecting the decomposition metric.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  decomposeFactorRisk(
    sensitivitiesJson: string,
    covarianceJson: string,
    riskMeasureJson?: string
  ): string;
}

/**
 * Namespaced TypeScript entry point for portfolio APIs.
 */
export declare const portfolio: PortfolioNamespace;

// --- scenarios -------------------------------------------------------------

/**
 * TypeScript view of the `ScenarioWarning` WebAssembly value.
 */
export interface ScenarioWarning {
  /**
   * Kind exposed by this `ScenarioWarning` value.
   */
  kind: string;
  [key: string]: unknown;
}

/**
 * TypeScript view of the `ScenarioApplyResult` WebAssembly value.
 */
export interface ScenarioApplyResult {
  /**
   * Market json exposed by this `ScenarioApplyResult` value.
   */
  market_json: string;
  /**
   * Model json exposed by this `ScenarioApplyResult` value.
   */
  model_json: string;
  /**
   * Operations applied exposed by this `ScenarioApplyResult` value.
   */
  operations_applied: number;
  /**
   * User operations exposed by this `ScenarioApplyResult` value.
   */
  user_operations: number;
  /**
   * Expanded operations exposed by this `ScenarioApplyResult` value.
   */
  expanded_operations: number;
  /**
   * Warnings exposed by this `ScenarioApplyResult` value.
   */
  warnings: ScenarioWarning[];
}

/**
 * TypeScript view of the `ScenarioApplyMarketResult` WebAssembly value.
 */
export interface ScenarioApplyMarketResult {
  /**
   * Market json exposed by this `ScenarioApplyMarketResult` value.
   */
  market_json: string;
  /**
   * Operations applied exposed by this `ScenarioApplyMarketResult` value.
   */
  operations_applied: number;
  /**
   * User operations exposed by this `ScenarioApplyMarketResult` value.
   */
  user_operations: number;
  /**
   * Expanded operations exposed by this `ScenarioApplyMarketResult` value.
   */
  expanded_operations: number;
  /**
   * Warnings exposed by this `ScenarioApplyMarketResult` value.
   */
  warnings: ScenarioWarning[];
}

/**
 * Namespaced TypeScript entry points for scenarios calculations and types.
 * @example
 * ```typescript
 * import init, { scenarios } from "finstack-quant-wasm";
 * await init();
 * const api: ScenariosNamespace = scenarios;
 * void api;
 * ```
 */
export interface ScenariosNamespace {
  /**
   * Parse and validate a scenario specification from JSON.
   *
   * Returns the validated, re-serialized JSON.
   * @param jsonStr - Canonical JSON string to validate, parse, or normalize for this API.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  parseScenarioSpec(jsonStr: string): string;
  /**
   * Compose multiple scenario specs (JSON array) into a single scenario.
   *
   * Specs are merged in priority order (lower number runs first).
   * @param specsJson - JSON array of validated ScenarioSpec objects to compose in priority order.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  composeScenarios(specsJson: string): string;
  /**
   * Validate a scenario specification JSON without executing it.
   *
   * Returns `true` if valid, throws on error.
   * @param jsonStr - Canonical JSON string to validate, parse, or normalize for this API.
   * @returns Returns `true` when the documented condition is satisfied.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  validateScenarioSpec(jsonStr: string): boolean;
  /**
   * List all built-in template identifiers.
   *
   * Returns a JSON array of template ID strings.
   * @returns Returns the resulting `string[]` collection in the documented order.
   */
  listBuiltinTemplates(): string[];
  /**
   * Get metadata for all built-in templates as a JSON string.
   * @returns Returns the requested string representation or JSON payload.
   */
  listBuiltinTemplateMetadata(): string;
  /**
   * Build a scenario spec from a built-in template.
   *
   * Returns JSON-serialized `ScenarioSpec`.
   * @param templateId - Identifier of a built-in scenario template in the embedded registry.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  buildFromTemplate(templateId: string): string;
  /**
   * List component IDs for a built-in composite template.
   *
   * Returns a JS array of component ID strings.
   * @param templateId - Identifier of a built-in scenario template in the embedded registry.
   * @returns Returns the resulting `string[]` collection in the documented order.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  listTemplateComponents(templateId: string): string[];
  /**
   * Build a specific component from a built-in composite template.
   * @param templateId - Identifier of a built-in scenario template in the embedded registry.
   * @param componentId - Identifier of a component within the selected composite template.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  buildTemplateComponent(templateId: string, componentId: string): string;
  /**
   * Build a scenario spec from fields.
   * @param id - Stable identifier used to name and retrieve the supplied domain object.
   * @param operationsJson - JSON array of scenario operation specifications in execution order.
   * @param name - Optional human-readable scenario name.
   * @param description - Optional human-readable description of the scenario purpose.
   * @param priority - Execution priority; lower values run earlier during composition.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  buildScenarioSpec(
    id: string,
    operationsJson: string,
    name?: string,
    description?: string,
    priority?: number
  ): string;
  /**
   * Apply a scenario to a market context and financial model.
   *
   * Returns a JSON object with `market_json`, `model_json`,
   * `operations_applied`, `user_operations`, `expanded_operations`,
   * `rounding_context` (active rounding-mode stamp), `time_roll` (a
   * `RollForwardReport`, only present when the scenario contained a
   * `time_roll_forward` operation), and `warnings`.
   *
   * This entry point supplies no instrument portfolio and no holiday calendar
   * to the engine: instrument-scoped operations (`instrument_price_pct_by_*`,
   * `instrument_spread_bp_by_*`, correlation shocks) are inert and produce a
   * warning, and `time_roll_forward` in `business_days` mode adjusts without
   * holiday information.
   * @param scenarioJson - JSON-serialized ScenarioSpec to validate and apply.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param modelJson - JSON-serialized FinancialModelSpec that scenario operations may mutate.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @returns Returns the resulting `ScenarioApplyResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  applyScenario(
    scenarioJson: string,
    marketJson: string,
    modelJson: string,
    asOf: string
  ): ScenarioApplyResult;
  /**
   * Apply a scenario to a market context only (no model mutations).
   *
   * Returns the same envelope shape as [`apply_scenario`] minus `model_json`;
   * the same caveats apply (no instrument portfolio, no holiday calendar).
   * @param scenarioJson - JSON-serialized ScenarioSpec to validate and apply.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @returns Returns the resulting `ScenarioApplyMarketResult` value or WebAssembly handle.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  applyScenarioToMarket(
    scenarioJson: string,
    marketJson: string,
    asOf: string
  ): ScenarioApplyMarketResult;
  /**
   * Compute horizon total return under a scenario.
   *
   * Applies a scenario specification to project an instrument forward, then
   * decomposes the resulting P&L using factor-based attribution.
   *
   * @param instrumentJson - JSON-serialized instrument (tagged).
   * @param marketJson - JSON-serialized `MarketContext`.
   * @param asOf - Valuation date (ISO 8601).
   * @param scenarioJson - JSON-serialized `ScenarioSpec`.
   * @param method - Attribution method: "parallel", "waterfall", "metrics_based", "taylor".
   * # Returns
   *
   * JSON-serialized `HorizonResult`.
   * @param configJson - Optional FinstackConfig JSON for horizon analysis; omit to use defaults.
   * @param calendarId - Optional holiday calendar (e.g. "nyse", "target") used to
   *   business-day adjust `time_roll_forward` targets under `business_days` mode.
   *   Omit for a weekends-only calendar; unknown identifiers throw.
   * @returns Returns the requested string representation or JSON payload.
   * @throws Error - Thrown when supplied values are malformed, violate the documented constraints, or the underlying calculation cannot complete.
   */
  computeHorizonReturn(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    scenarioJson: string,
    method?: string,
    configJson?: string,
    calendarId?: string
  ): string;
}

/**
 * Namespaced TypeScript entry point for scenarios APIs.
 */
export declare const scenarios: ScenariosNamespace;
