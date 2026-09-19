// Type declarations for the finstack-quant-wasm namespaced facade.
// Shapes follow `wasm-bindgen` JS names in `src/api/**` (see Rust `js_name`).
// The raw `pkg/finstack_quant_wasm.d.ts` emitted by wasm-bindgen is intentionally
// not the package root contract: it exposes a flat module, while `index.js`
// publishes a namespaced facade. Keep this file as the facade declaration and
// use generated `types/generated/*` files only for JSON envelope shapes.
//
// Building a MarketContext from quotes (canonical path):
//
//   import { calibration } from 'finstack-quant-wasm/exports/calibration.js';
//   import type { CalibrationEnvelope } from 'finstack-quant-wasm';
//   const envelope: CalibrationEnvelope = {
//     schema: 'finstack_quant.calibration/1',
//     plan: { id: 'usd_curves', quote_sets: {...}, steps: [...], settings: {} },
//     market_data: [],   // flat id-addressable quotes/snapshots
//     prior_market: [],  // optional pre-built curves/surfaces
//   };
//   const result = calibration.calibrate(envelope);  // CalibrationResultEnvelope
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
// Structured diagnostics: errors thrown by `calibrate`,
// `validateCalibrationJson`, and `dryRun` have:
//   - name: 'CalibrationEnvelopeError'
//   - kind: Rust-owned execution category such as 'strict_load' or
//     'solver_not_converged'
//   - stage: 'ingestion', 'configuration', 'context', 'preflight', 'target',
//     or 'solver'
//   - step_id: offending step ID, or undefined for plan-wide failures
//   - solver_diagnostics: structured fit diagnostics, or undefined when unavailable
//   - details: JSON-serialized stable execution-error payload
//   - cause: the same stable execution-error payload as a structured object
// `kind` and `step_id` are independent: never use a step ID as the category.

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
 */
export default function init(
  moduleOrPath?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>
): Promise<InitOutput>;

// --- Calibration envelope types (generated from Rust via ts-rs) ---
import type { CalibrationEnvelope } from './types/generated/CalibrationEnvelope';
import type { CalibrationResultEnvelope } from './types/generated/CalibrationResultEnvelope';
import type { MaterializationReport } from './types/generated/MaterializationReport';
import type { ValidationReport } from './types/generated/ValidationReport';

export type { CalibrationEnvelope, CalibrationResultEnvelope };
export type { Diagnostic } from './types/generated/Diagnostic';
export type { MaterializationPhases } from './types/generated/MaterializationPhases';
export type { MaterializationReport } from './types/generated/MaterializationReport';
export type { ValidationReport } from './types/generated/ValidationReport';
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
 * Dates are ISO-8601 values in ascending order. The `prices` and `returns`
 * supplied to the panel constructors are ticker-major and column-oriented;
 * matrix inputs to other methods follow their parameter documentation. Scalar
 * rates and returns use decimal fractions; numeric outputs are Float64Array
 * values in ticker order unless the method documents an object or matrix shape.
 *
 * Invalid dates, shapes, frequencies, tickers, and confidence levels are
 * returned as rejected JsValue errors.
 */
export interface Performance extends WasmOwned {}
/**
 * Calibrated credit factor hierarchy artifact.
 *
 * Produced by [`JsCreditCalibrator`] or loaded from JSON via
 * [`JsCreditFactorModel::from_json`]. Immutable once constructed.
 */
export interface CreditFactorModel extends WasmOwned {}
/**
 * Deterministic calibrator that produces a [`JsCreditFactorModel`].
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
 * Component-wise difference between two [`JsLevelsAtDate`] snapshots.
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
 * Construct once from JSON, then pass to `priceInstrumentWithMarket` and
 * other `*WithMarket` pricing entry points. Eliminates the per-call
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
   * @example
   * ```javascript
   * const eur = new core.Currency("eur"); // case-insensitive
   * eur.code; // "EUR"
   * ```
   * @param code - Three-letter ISO-4217 code (e.g. `"USD"`, `"eur"`, `"GBP"`). Leading and trailing whitespace is trimmed.
   * @returns Constructed `Currency`.
   * @throws If `code` is not a recognized ISO-4217 alphabetic code.
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
 * Money values pin a numeric amount to a [`JsCurrency`]. Arithmetic
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
   * @returns The [`JsCurrency`] this amount is tagged with.
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
   * @returns Converted `Money` amount in the target currency.
   * @param target - Target Currency for the converted monetary amount.
   * @param rate - FX conversion rate expressed as target-currency units per source-currency unit.
   * @throws Error - For a different target currency, throws a JavaScript exception if `rate` is non-finite or not strictly positive, or if the converted amount cannot be represented as a decimal.
   */
  convertAtRate(target: Currency, rate: number): Money;
  /**
   * Add two amounts.
   *
   * @example
   * ```javascript
   * const usd = new core.Currency("USD");
   * const a = new core.Money(10, usd);
   * const b = new core.Money(5, usd);
   * a.add(b).amount;  // 15
   * ```
   * @param other - Another `Money` value.
   * @returns Sum, in the same currency.
   * @throws If `other.currency` differs from `this.currency`, or the operation is not representable as a `Decimal`.
   */
  add(other: Money): Money;
  /**
   * Subtract two amounts.
   *
   * @param other - Another `Money` value.
   * @returns Difference, in the same currency.
   * @throws If `other.currency` differs from `this.currency`, or the operation is not representable as a `Decimal`.
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
  /**
   * Serialize to a JSON string using the canonical Rust serde schema.
   *
   * @returns A JSON string carrying the exact decimal amount and the ISO-4217 currency code.
   * @throws If serialization fails (should not happen for valid `Money`).
   */
  toJson(): string;
}

/**
 * Currency-tagged monetary amount.
 *
 * Money values pin a numeric amount to a [`JsCurrency`]. Arithmetic
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
   * @example
   * ```javascript
   * const usd = new core.Currency("USD");
   * const m = new core.Money(1234.56, usd);
   * m.amount;          // 1234.56
   * m.currency.code;   // "USD"
   * ```
   * @param amount - Numeric amount in major units (must be finite).
   * @param currency - ISO-4217 Currency object that tags the amount and controls arithmetic compatibility.
   * @returns The constructed `Money`.
   * @throws If `amount` is non-finite (NaN, ±∞) or cannot be represented as a `Decimal`.
   */
  new (amount: number, currency: Currency): Money;
  /**
   * Deserialize from a JSON string produced by `Money.toJson`.
   *
   * @param json - A JSON string in the canonical Rust `Money` schema.
   * @returns The parsed `Money`.
   * @throws If `json` is malformed or fails strict schema validation.
   */
  fromJson(json: string): Money;
}

/**
 * Interest or discount rate stored as a decimal (e.g. `0.05` is 5%).
 *
 * Conventions:
 * - **Decimal**: `0.05` represents 5%.
 * - **Percent**: `5.0` represents 5%.
 * - **Basis points**: `500` represents 5% (1 bp = 0.01%).
 *
 * Use the `fromPercent` or `fromBp` factories to avoid scaling errors
 * when working with quoted rates.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const r = core.Rate.fromBp(250);     // 2.5% as 250 bp
 * r.asDecimal;  // 0.025
 * r.asPercent;  // 2.5
 * r.asBp;      // 250
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
   * @returns Rate in bp.
   */
  readonly asBp: number;
}

/**
 * Interest or discount rate stored as a decimal (e.g. `0.05` is 5%).
 *
 * Conventions:
 * - **Decimal**: `0.05` represents 5%.
 * - **Percent**: `5.0` represents 5%.
 * - **Basis points**: `500` represents 5% (1 bp = 0.01%).
 *
 * Use the `fromPercent` or `fromBp` factories to avoid scaling errors
 * when working with quoted rates.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const r = core.Rate.fromBp(250);     // 2.5% as 250 bp
 * r.asDecimal;  // 0.025
 * r.asPercent;  // 2.5
 * r.asBp;      // 250
 * ```
 */
export interface RateConstructor {
  /**
   * Create a rate from a decimal value.
   *
   * @example
   * ```javascript
   * const r = new core.Rate(0.05);  // 5%
   * r.asPercent;  // 5
   * ```
   * @param decimal - Rate as a decimal (e.g. `0.05` for 5%).
   * @returns The constructed `Rate`.
   * @throws If `decimal` is non-finite (NaN, ±∞).
   */
  new (decimal: number): Rate;
  /**
   * Create a rate from a percent figure.
   *
   * @example
   * ```javascript
   * const r = core.Rate.fromPercent(5.0);
   * r.asDecimal;  // 0.05
   * ```
   * @param pct - Percent value (e.g. `5.0` for 5%).
   * @returns The constructed `Rate`.
   * @throws If `pct` is non-finite.
   */
  fromPercent(pct: number): Rate;
  /**
   * Create a rate from a whole number of basis points.
   *
   * The canonical Rust `Rate::from_bp` takes an integer (`i32`) number
   * of basis points. Because JavaScript numbers are `f64`, this binding
   * accepts a float but **rejects fractional input** rather than
   * silently rounding it: a sub-bp rate quietly rounded to whole bp is a
   * pricing bug, not a convenience. Use `new Rate(decimal)` or
   * `Rate.fromPercent` for sub-bp rates.
   *
   * @example
   * ```javascript
   * const r = core.Rate.fromBp(250);  // 2.5%
   * r.asDecimal;  // 0.025
   * ```
   * @param bp - Rate in whole basis points (e.g. `500` for 5%).
   * @returns The constructed `Rate`.
   * @throws If `bp` is non-finite or not a whole number of basis points.
   */
  fromBp(bp: number): Rate;
}

/**
 * Basis points (1 bp = 0.01%, 10_000 bp = 100%).
 *
 * Stored as integer bp internally; constructors reject fractional input.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const spread = new core.Bps(125);
 * spread.asDecimal();  // 0.0125
 * spread.asBp();      // 125
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
   * @returns Integer bp.
   */
  asBp(): number;
}

/**
 * Basis points (1 bp = 0.01%, 10_000 bp = 100%).
 *
 * Stored as integer bp internally; constructors reject fractional input.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const spread = new core.Bps(125);
 * spread.asDecimal();  // 0.0125
 * spread.asBp();      // 125
 * ```
 */
export interface BpsConstructor {
  /**
   * Create basis points from a whole-number value.
   *
   * @param value - Value in whole basis points (e.g. `25` for 25 bp).
   * @returns The constructed `Bps`.
   * @throws If `value` is non-finite or not a whole number of basis points. Sub-bp spreads must use the JSON instrument path (which preserves fractional values) or a decimal `Rate`.
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
 * - `act_act_afb` (AFB / Actual/Actual Euro) → `DayCount.actActAfb`
 * - `30_360_it` (Italian) → `DayCount.thirty360It`
 * - `bus_252` → `DayCount.bus252`
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const day_count = core.DayCount.act365f();
 * const start = core.createDate(2025, 1, 15);
 * const end   = core.createDate(2025, 7, 15);
 * const yf    = day_count.yearFraction(start, end);
 * // yf ≈ 0.4959 (181 / 365)
 * ```
 */
export interface DayCount extends WasmOwned {
  /**
   * Compute the year fraction between two dates given as epoch days.
   *
   * Act/Act ISMA and Bus/252 require explicit frequency/calendar context.
   * This method throws for those conventions; call
   * `DayCount.yearFractionWithContext` with a configured `DayCountContext`.
   *
   * @example
   * ```javascript
   * const dayCount = core.DayCount.act360();
   * const start = core.createDate(2025, 1, 15);
   * const end   = core.createDate(2025, 4, 15);
   * dayCount.yearFraction(start, end); // 90 / 360 = 0.25
   * ```
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01.
   * @returns Year fraction (`>= 0` if `end >= start`).
   * @throws If either date is out of representable range. Act/Act ISMA and Bus/252 require explicit frequency/calendar context. This method throws for those conventions; call `DayCount.yearFractionWithContext` with a configured `DayCountContext`.
   */
  yearFraction(startEpochDays: number, endEpochDays: number): number;
  /**
   * Compute a signed year fraction, preserving the start/end orientation.
   * @returns Signed year fraction in years; negative when `end` is before `start`.
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01.
   * @throws Error - Throws a JavaScript exception if either epoch-day value is outside the representable date range or the selected convention requires calendar or coupon-frequency context. Use `yearFractionWithContext` for Bus/252 and Act/Act ISMA.
   */
  signedYearFraction(startEpochDays: number, endEpochDays: number): number;
  /**
   * Compute the year fraction with explicit convention context.
   * @returns Non-negative year fraction in years under the selected convention and context.
   * @param startEpochDays - Start date as days since 1970-01-01.
   * @param endEpochDays - End date as days since 1970-01-01.
   * @param ctx - DayCountContext supplying calendar, frequency, coupon-period, and termination metadata.
   * @throws Error - Throws a JavaScript exception if either epoch-day value is outside the representable date range, the start is after the end, the context names an unknown calendar, or the selected convention's required context is missing or invalid.
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
   * @returns Signed calendar-day count from start to end.
   * @throws Error - Throws a JavaScript exception if either epoch-day value is outside the representable date range.
   */
  calendarDays(startEpochDays: number, endEpochDays: number): bigint;
  /**
   * Convention name.
   * @returns Human-readable string form of this value.
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
 * - `act_act_afb` (AFB / Actual/Actual Euro) → `DayCount.actActAfb`
 * - `30_360_it` (Italian) → `DayCount.thirty360It`
 * - `bus_252` → `DayCount.bus252`
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const day_count = core.DayCount.act365f();
 * const start = core.createDate(2025, 1, 15);
 * const end   = core.createDate(2025, 7, 15);
 * const yf    = day_count.yearFraction(start, end);
 * // yf ≈ 0.4959 (181 / 365)
 * ```
 */
export interface DayCountConstructor {
  /**
   * Parse a day-count convention from its string name.
   *
   * @param name - Convention name (e.g. `"act_360"`, `"30_360"`, `"act_act"`). Underscored snake_case is canonical.
   * @returns The parsed `DayCount`.
   * @throws If `name` is not a recognized day-count convention.
   */
  new (name: string): DayCount;
  /**
   * Act/360 day-count convention.
   * @returns A `DayCount` handle for this convention.
   */
  act360(): DayCount;
  /**
   * One unit per nonempty contractual accrual period; empty periods return zero.
   * @returns The 1/1 convention used for annual inflation accrual periods.
   * @throws This constructor does not throw.
   */
  oneOne(): DayCount;
  /**
   * Actual/365 Fixed.
   * @returns A `DayCount` handle for this convention.
   */
  act365f(): DayCount;
  /**
   * Actual/365L (ICMA Rule 251). Annual periods (or periods without
   * frequency context) use denominator 366 exactly when February 29 falls
   * in `(start, end]`; non-annual periods use 366 exactly when the end
   * date's year is a leap year. Otherwise the denominator is 365. This is
   * not ACT/ACT AFB.
   * @returns A `DayCount` handle for this convention.
   */
  act365l(): DayCount;
  /**
   * 30/360 US (Bond Basis).
   * @returns A `DayCount` handle for this convention.
   */
  thirty360(): DayCount;
  /**
   * 30E/360 (Eurobond Basis).
   * @returns A `DayCount` handle for this convention.
   */
  thirtyE360(): DayCount;
  /**
   * 30E/360 ISDA day-count convention.
   * @returns A `DayCount` handle for this convention.
   */
  thirtyE360Isda(): DayCount;
  /**
   * Actual/Actual (ISDA).
   * @returns A `DayCount` handle for this convention.
   */
  actAct(): DayCount;
  /**
   * Actual/Actual (ICMA/ISMA).
   * @returns A `DayCount` handle for this convention.
   */
  actActIsma(): DayCount;
  /**
   * Actual/Actual AFB (Actual/Actual Euro).
   *
   * Walks whole years backwards from the end date (QuantLib
   * `ActualActual::AFB`). A year-step landing on 28 February of a leap
   * year is bumped to 29 February. The residual uses denominator 366 if
   * 29 February lies in `[start, residual_end)`, else 365.
   * @returns A `DayCount` handle for this convention.
   */
  actActAfb(): DayCount;
  /**
   * Return a `DayCount` handle configured for thirty360 it.
   *
   * Day 31 becomes 30, and any February day after the 27th becomes 30
   * (QuantLib `Thirty360::Italian`). Distinct from US SIA and 30E/360.
   * @returns A `DayCount` handle for this convention.
   */
  thirty360It(): DayCount;
  /**
   * Business/252 day-count convention.
   * @returns A `DayCount` handle for this convention.
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
   * @returns A new `DayCountContext` handle.
   */
  withCalendar(calendarCode: string): DayCountContext;
  /**
   * Return a copy with the coupon frequency used by Act/Act ISMA.
   * @param frequency - Coupon-frequency Tenor required by Actual/Actual ICMA calculations.
   * @returns A new `DayCountContext` handle.
   */
  withFrequency(frequency: Tenor): DayCountContext;
  /**
   * Return a copy with the business-day basis used by Bus/252.
   * @param busBasis - Business-day denominator for Bus/252, normally 252.
   * @returns A new `DayCountContext` handle.
   */
  withBusBasis(busBasis: number): DayCountContext;
  /**
   * Return a copy with the reference coupon period (epoch days) used by
   * Act/Act ICMA. Errors when either date is out of range or
   * `start >= end`.
   * @param startEpochDays - Reference coupon-period start as days since 1970-01-01.
   * @param endEpochDays - Reference coupon-period end as days since 1970-01-01.
   * @returns A new `DayCountContext` handle.
   * @throws Error - Throws a JavaScript exception if either epoch-day value is outside the representable date range or the start is not strictly before the end.
   */
  withCouponPeriod(startEpochDays: number, endEpochDays: number): DayCountContext;
  /**
   * Return a copy indicating whether the accrual end is the instrument's
   * termination date (required by 30E/360 ISDA February-end handling).
   * @param value - Whether the accrual end is the contractual termination date for 30E/360 ISDA.
   * @returns A new `DayCountContext` handle.
   */
  withEndIsTerminationDate(value: boolean): DayCountContext;
}

/**
 * Optional context for day-count conventions that need market metadata.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const context = new core.DayCountContext()
 *   .withBusBasis(252)
 *   .withEndIsTerminationDate(true);
 * console.log(context);
 * ```
 */
export interface DayCountContextConstructor {
  /**
   * Create an empty day-count context.
   * @returns An empty `DayCountContext` handle.
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
   * Unit count of this tenor, such as `3` for `"3M"`.
   */
  readonly count: number;
  /**
   * Approximate length in years (simple estimate, no calendar).
   * @returns Approximate tenor length in years, such as `0.25` for `"3M"`.
   */
  toYearsSimple(): number;
  /**
   * Tenor string representation.
   * @returns Human-readable string form of this value.
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
   * @param s - Tenor string. Accepted forms include `"3M"`, `"1Y"`, `"2W"`, `"7D"`, `"6M"`, `"10Y"`. Whitespace is permitted.
   * @returns The parsed `Tenor`.
   * @throws If `s` cannot be parsed (unknown unit, missing count).
   */
  new (s: string): Tenor;
  /**
   * One-day tenor (`"1D"`).
   * @returns A `Tenor` handle for this named period.
   */
  daily(): Tenor;
  /**
   * One-week tenor (`"1W"`).
   * @returns A `Tenor` handle for this named period.
   */
  weekly(): Tenor;
  /**
   * One-month tenor (`"1M"`).
   * @returns A `Tenor` handle for this named period.
   */
  monthly(): Tenor;
  /**
   * 3-month (quarterly) tenor.
   * @returns A `Tenor` handle for this named period.
   */
  quarterly(): Tenor;
  /**
   * 6-month (semi-annual) tenor.
   * @returns A `Tenor` handle for this named period.
   */
  semiAnnual(): Tenor;
  /**
   * 12-month (annual) tenor.
   * @returns A `Tenor` handle for this named period.
   */
  annual(): Tenor;
}

/**
 * Discount-curve validation policy: market-standard or negative-rate-friendly.
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
   * @returns Discount factor for 1 unit paid at time `t`.
   * @param t - Time from the curve base date in years.
   */
  df(t: number): number;
  /**
   * Continuously-compounded zero rate at year fraction `t`.
   * @returns Continuously compounded zero rate as a decimal, such as `0.04` for 4%.
   * @param t - Time from the curve base date in years.
   */
  zero(t: number): number;
  /**
   * Continuously-compounded forward rate between `t1` and `t2`.
   * @returns Continuously compounded forward rate as a decimal over `(t1, t2)`.
   * @param t1 - Earlier curve time in years used as the start of the forward interval.
   * @param t2 - Later curve time in years used as the end of the forward interval.
   * @throws Error - Throws a JavaScript exception if either time is non-finite, `t2` is not later than `t1`, the interval is shorter than the curve's minimum forward tenor, or either endpoint discount factor is non-finite or non-positive.
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
   * @param id - Curve identifier (e.g. `"USD-OIS"`). Used as the lookup key inside a `MarketContext`.
   * @param baseDate - ISO-8601 date string (`"YYYY-MM-DD"`). All `time` values are interpreted as year fractions from this date under `dayCount`.
   * @param knots - Flat `[t0, df0, t1, df1, …]` array. `t` in years, `df` strictly positive. Length must be even.
   * @param interp - Interpolation style. When omitted, the Rust builder default (`"monotone_convex"`) applies. One of `"linear"`, `"log_linear"`, `"monotone_convex"`, `"cubic_hermite"`, `"piecewise_quadratic_forward"`.
   * @param extrapolation - Extrapolation policy. When omitted, the Rust builder default (`"flat_forward"`) applies. One of `"flat_zero"`, `"flat_forward"`, `"nan"`.
   * @param dayCount - Day-count convention (defaults to curve-ID inference).
   * @param validationMode - Rust validation preset: `"market_standard"` (default) or `"negative_rate_friendly"`.
   * @param forwardFloor - Required minimum implied forward when using `"negative_rate_friendly"`.
   * @returns The constructed `DiscountCurve`.
   * @throws If `knots` length is odd, the date is malformed, the interpolation style is unknown, or any `df` is non-positive.
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
   * @returns A `DiscountCurve` handle.
   * @param id - Curve identifier stored on the constructed discount curve.
   * @param baseDate - ISO-8601 curve base date from which time coordinates are measured.
   * @param continuousRate - Flat continuously compounded zero rate expressed as a decimal.
   * @throws Error - Throws a JavaScript exception if `baseDate` is not a valid ISO date, `continuousRate` is non-finite, or the implied discount factors are not finite and strictly positive.
   */
  flat(id: string, baseDate: string, continuousRate: number): DiscountCurve;
}

/**
 * Credit hazard-rate curve for default-probability modelling.
 *
 * Built from `(time, hazard_rate)` pillars where `time` is a year fraction
 * from `baseDate` and `hazard_rate` is the instantaneous default intensity
 * `λ(t)`. Survival is `S(t) = exp(-∫₀ᵗ λ(u) du)`.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * // Flat 200bp hazard rate, 40% recovery.
 * const hz = new core.HazardCurve(
 *   "ACME-HZD",
 *   "2025-01-02",
 *   [0.0, 0.02, 30.0, 0.02],
 *   0.4,
 * );
 * hz.sp(5.0);          // survival probability at 5y
 * hz.hazardRate(5.0);  // instantaneous hazard rate at 5y
 * ```
 */
export interface HazardCurve extends WasmOwned {
  /**
   * Curve identifier.
   */
  readonly id: string;
  /**
   * Base date as ISO string.
   */
  readonly baseDate: string;
  /**
   * Recovery rate assumed on default.
   */
  readonly recoveryRate: number;
  /**
   * Survival probability `S(t)` at year fraction `t`.
   * @param t - Time from the curve base date in years.
   * @returns The probability of surviving from the base date through `t`, in `[0, 1]`. This operation does not throw.
   */
  sp(t: number): number;
  /**
   * Instantaneous hazard rate `lambda(t)` at year fraction `t`.
   * @param t - Time from the curve base date in years.
   * @returns The annualized default intensity at `t`, expressed as a decimal rate. This operation does not throw.
   */
  hazardRate(t: number): number;
}

/**
 * Credit hazard-rate curve for default-probability modelling.
 *
 * Built from `(time, hazard_rate)` pillars where `time` is a year fraction
 * from `baseDate` and `hazard_rate` is the instantaneous default intensity
 * `λ(t)`. Survival is `S(t) = exp(-∫₀ᵗ λ(u) du)`.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * // Flat 200bp hazard rate, 40% recovery.
 * const hz = new core.HazardCurve(
 *   "ACME-HZD",
 *   "2025-01-02",
 *   [0.0, 0.02, 30.0, 0.02],
 *   0.4,
 * );
 * hz.sp(5.0);          // survival probability at 5y
 * hz.hazardRate(5.0);  // instantaneous hazard rate at 5y
 * ```
 */
export interface HazardCurveConstructor {
  /**
   * Construct from an array of `[time, hazardRate]` pairs.
   *
   * @param id - Curve identifier (e.g. `"ACME-HZD"`).
   * @param baseDate - ISO-8601 date string (`"YYYY-MM-DD"`). All `time` values are year fractions from this date under `dayCount`.
   * @param knots - Flat `[t0, lambda0, t1, lambda1, …]` array. `t` in years, `lambda` a non-negative intensity. Length must be even.
   * @param recoveryRate - Required recovery on default as a decimal fraction in `[0, 1]`.
   * @param dayCount - Day-count convention (default `"act_365f"`).
   * @returns The constructed `HazardCurve`.
   * @throws If `recoveryRate` is missing, non-finite, or outside `[0, 1]`, `knots` length is odd, the date is malformed, the day-count is unknown, or the curve otherwise fails validation.
   */
  new (
    id: string,
    baseDate: string,
    knots: NumericArray,
    recoveryRate: number,
    dayCount?: string
  ): HazardCurve;
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
  readonly projectionGrid: Float64Array | undefined;
  /**
   * Business days from fixing to spot.
   */
  readonly resetLag: number;
  /**
   * Forward rate at year fraction `t`.
   * @returns Simply compounded forward rate as a decimal at time `t`.
   * @param t - Time from the curve base date in years.
   */
  rate(t: number): number;
  /**
   * Discount-factor-implied simple forward over `(t1, t2)`.
   * @returns Simple forward rate as a decimal implied by discount factors over `(t1, t2)`.
   * @param t1 - Earlier curve time in years used as the start of the forward interval.
   * @param t2 - Later curve time in years used as the end of the forward interval.
   * @throws Error - Throws a JavaScript exception if either time is non-finite, `t2` is not later than `t1`, a projection discount factor cannot be computed, or the implied rate is non-finite.
   */
  rateBetween(t1: number, t2: number): number;
}

/**
 * Forward rate curve for a floating-rate index with a fixed tenor.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const curve = new core.ForwardCurve({
 *   id: "USD-SOFR-3M", tenor: 0.25, baseDate: "2026-01-02",
 *   knots: [0, 0.03, 1, 0.035]
 * });
 * console.log(curve.rate(0.5));
 * ```
 */
export interface ForwardCurveConstructor {
  /**
   * Construct a forward curve using named inputs and canonical Rust defaults.
   * @returns The validated forward curve.
   * @param options - ForwardCurveOptions object: curve id, tenor in years, ISO baseDate, flat time/decimal-rate knots, and optional dayCount, interp, extrapolation, projectionGrid and resetLag. Omitted policies use the Rust builder defaults; arrays and typed arrays are accepted.
   * @throws Error - Throws Error when options cannot be decoded or canonical curve validation rejects dates, conventions, knots, tenor, reset lag, or projection grid.
   */
  new (options: ForwardCurveOptions): ForwardCurve;
}

/**
 * Named options for constructing a `ForwardCurve`.
 */
export interface ForwardCurveOptions {
  /**
   * Curve identifier stored on the constructed forward curve.
   */
  id: string;
  /**
   * Index tenor in years, such as 0.25 for a 3-month forward.
   */
  tenor: number;
  /**
   * ISO-8601 base or valuation date that anchors the curve time axis.
   */
  baseDate: string;
  /**
   * Flat `[time, rate]` pairs in year-fraction / decimal-rate units.
   */
  knots: NumericArray;
  /**
   * Day-count convention used to convert dates into year fractions.
   */
  dayCount?: string;
  /**
   * Interpolation style between knots, such as `"monotone_convex"`.
   */
  interp?: string;
  /**
   * Extrapolation policy beyond the last knot, such as `"flat_forward"`.
   */
  extrapolation?: string;
  /**
   * Projection-grid specification that defines the curve's forward-rate intervals.
   */
  projectionGrid?: NumericArray;
  /**
   * Reset lag applied when projecting the index or forward rate.
   */
  resetLag?: number | null;
}

/**
 * Data-only SABR volatility cube for swaption pricing.
 *
 * Stores parameter nodes, forward nodes, axes, and interpolation metadata.
 * Use `models.volatility` for evaluation.
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
}

/**
 * Data-only SABR volatility cube for swaption pricing.
 *
 * Stores calibrated parameter nodes and forwards on an expiry × tenor grid.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const cube = new core.VolCube(
 *   "USD-SWAPTION",
 *   [1],
 *   [5],
 *   [0.02, 0.5, -0.2, 0.4, Number.NaN],
 *   [0.03]
 * );
 * console.log(cube.id, cube.interpolationMode);
 * cube.free();
 * ```
 */
export interface VolCubeConstructor {
  /**
   * Construct a vol cube from a flat SABR parameter array.
   *
   * @returns A `VolCube` handle.
   * @param id - Curve identifier.
   * @param expiries - Option expiry axis in years (strictly increasing).
   * @param tenors - Swap tenor axis in years (strictly increasing).
   * @param paramsFlat - Row-major flat array of SABR parameters: `[alpha0, beta0, rho0, nu0, shift0, alpha1, …]`. Length must equal `expiries.len() * tenors.len() * 5`. Pass `NaN` for the shift element of a node to omit the shift.
   * @param forwards - Row-major forward rates, one per grid node.
   * @param interpolationMode - Volatility-surface interpolation mode used between quoted points.
   * @throws Error - Throws a JavaScript exception if an axis is empty, non-finite, non-positive, or not strictly increasing; the parameter or forward array has the wrong length; a forward is non-finite; any SABR node has invalid alpha, beta, rho, nu, or shift; or `interpolationMode` is neither `vol` nor `total_variance`.
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
   * @returns Human-readable string form of this value.
   */
  toString(): string;
}

/**
 * Typed FX conversion policy wrapper for WASM callers.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const policy = core.FxConversionPolicy.cashflowDate();
 * console.log(policy.toString());
 * ```
 */
export interface FxConversionPolicyConstructor {
  /**
   * Use spot/forward on the cashflow date.
   * @returns An `FxConversionPolicy` handle.
   */
  cashflowDate(): FxConversionPolicy;
  /**
   * Use period end date.
   * @returns An `FxConversionPolicy` handle.
   */
  periodEnd(): FxConversionPolicy;
  /**
   * Use an average over the period.
   * @returns An `FxConversionPolicy` handle.
   */
  periodAverage(): FxConversionPolicy;
  /**
   * Parse from a string label such as ``\"cashflow_date\"``.
   * @returns An `FxConversionPolicy` handle.
   * @param name - Policy label: `cashflow_date`, `period_end`, or `period_average`.
   * @throws Error - Throws a JavaScript exception unless `name` is `cashflow_date`, `period_end`, or `period_average`.
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
 * const matrix = new core.FxMatrix();
 * matrix.setQuote("EUR", "USD", 1.1);
 * const result = matrix.rate(
 *   "EUR",
 *   "USD",
 *   "2026-01-02",
 *   core.FxConversionPolicy.cashflowDate()
 * );
 * console.log(result.rate, result.triangulated);
 * ```
 */
export interface FxRateResultConstructor {
  /**
   * JavaScript prototype of `FxRateResult`; instances come from `FxMatrix.rate`, not `new`.
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
   * @throws Error - Throws a JavaScript exception if either currency code is invalid or `rate` is non-finite or not strictly positive.
   */
  setQuote(base: string, quote: string, rate: number): void;
  /**
   * Set an authoritative quote scoped to one date and conversion policy.
   * @param base - Base currency code of the FX quote, where the rate is quote per base.
   * @param quote - Quote currency code of the FX rate, expressed per unit of base currency.
   * @param date - ISO-8601 date used by the calculation or market-data lookup.
   * @param policy - FX quote-selection policy for resolving direct, inverse, or triangulated rates.
   * @param rate - Finite positive quote-currency units per one base-currency unit.
   * @throws Error - Throws a JavaScript exception if either currency code is invalid, `date` is not a valid ISO date, or `rate` is non-finite or not strictly positive.
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
   * @returns Resolved FX rate, including whether it was triangulated.
   * @param base - Base (from) currency ISO code.
   * @param quote - Quote (to) currency ISO code.
   * @param date - ISO date string.
   * @param policy - Reusable conversion policy handle.
   * @throws Error - Throws a JavaScript exception if either currency code or `date` is invalid, no direct, inverse, or triangulated quote is available, or a resolved quote is non-finite or non-positive.
   */
  rate(base: string, quote: string, date: string, policy: FxConversionPolicy): FxRateResult;
  /**
   * Look up an FX rate using cashflow-date conversion semantics.
   * @returns Resolved FX rate, including whether it was triangulated.
   * @param base - Base currency code of the FX quote, where the rate is quote per base.
   * @param quote - Quote currency code of the FX rate, expressed per unit of base currency.
   * @param date - ISO-8601 date used by the calculation or market-data lookup.
   * @throws Error - Throws a JavaScript exception if either currency code or `date` is invalid, no direct, inverse, or triangulated cashflow-date quote is available, or a resolved quote is non-finite or non-positive.
   */
  rateDefault(base: string, quote: string, date: string): FxRateResult;
}

/**
 * Foreign-exchange rate matrix for currency conversion.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const matrix = new core.FxMatrix();
 * matrix.setQuote("EUR", "USD", 1.1);
 * console.log(matrix.rateDefault("EUR", "USD", "2026-01-02").rate);
 * ```
 */
export interface FxMatrixConstructor {
  /**
   * Create an empty FX matrix.
   * @returns An `FxMatrix` handle.
   */
  new (): FxMatrix;
}

/**
 * USD quotation style for a market FX pair (Direct or Indirect versus USD).
 *
 * **Direct** means USD is the quote currency (EURUSD, GBPUSD). **Indirect**
 * means USD is the base (USDJPY, USDCAD). Non-USD crosses inherit the USD
 * quotation of market CCY1 versus USD.
 */
export interface FxQuoteConvention extends WasmOwned {
  /**
   * String form of the USD quotation style (`"direct"` or `"indirect"`).
   * @returns Human-readable string form of this value.
   */
  toString(): string;
}

/**
 * USD quotation style for a market FX pair (Direct or Indirect versus USD).
 *
 * **Direct** means USD is the quote currency (EURUSD, GBPUSD). **Indirect**
 * means USD is the base (USDJPY, USDCAD). Non-USD crosses inherit the USD
 * quotation of market CCY1 versus USD.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const direct = core.FxQuoteConvention.direct();
 * direct.toString(); // "direct"
 * ```
 */
export interface FxQuoteConventionConstructor {
  /**
   * USD is the quote currency (units of USD per one unit of CCY1).
   * @returns An `FxQuoteConvention` handle.
   */
  direct(): FxQuoteConvention;
  /**
   * USD is the base currency (units of CCY2 per one USD).
   * @returns An `FxQuoteConvention` handle.
   */
  indirect(): FxQuoteConvention;
  /**
   * Parse from a string label such as `"direct"` or `"indirect"`.
   * @returns An `FxQuoteConvention` handle.
   * @param name - Convention label: `direct` or `indirect`.
   * @throws Error - Throws a JavaScript exception unless `name` is `direct` or `indirect`.
   */
  fromName(name: string): FxQuoteConvention;
}

/**
 * Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.
 *
 * Instances come from `fxPairConvention`. `base` / `quote` are always market
 * CCY1/CCY2, even when the lookup arguments were inverted.
 */
export interface FxPairConvention extends WasmOwned {
  /**
   * Market CCY1 (one unit of this currency in the screen pair).
   */
  readonly base: Currency;
  /**
   * Market CCY2 (units of this currency per one unit of CCY1).
   */
  readonly quote: Currency;
  /**
   * Direct if the USD leg quotes USD as CCY2; Indirect if USD is CCY1.
   */
  readonly usdQuotation: FxQuoteConvention;
  /**
   * Pip size in outright-rate units (`0.01` or `0.0001`).
   */
  readonly pipSize: number;
  /**
   * Standard spot lag in business days (T+1 or T+2).
   */
  readonly spotLagDays: number;
}

/**
 * Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.
 *
 * Instances come from `fxPairConvention`. `base` / `quote` are always market
 * CCY1/CCY2, even when the lookup arguments were inverted.
 *
 * @example
 * ```javascript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const conv = core.fxPairConvention("USD", "EUR");
 * conv.base.code;          // "EUR"
 * conv.usdQuotation.toString(); // "direct"
 * conv.pipSize;            // 0.0001
 * conv.spotLagDays;        // 2
 * ```
 */
export interface FxPairConventionConstructor {
  /**
   * JavaScript prototype of `FxPairConvention`; instances come from
   * `fxPairConvention`, not `new`.
   */
  readonly prototype: FxPairConvention;
}

/**
 * FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
 * 10-delta wings).
 *
 * Stores market-standard FX delta quotes (Wystup 2006, Clark 2011). Use
 * `models.volatility` for pillar recovery, conversion, and evaluation. The
 * delta convention is **forward delta (premium-unadjusted)**.
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
}

/**
 * FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
 * 10-delta wings).
 *
 * Stores market-standard FX delta quotes (Wystup 2006, Clark 2011). The delta
 * convention is **forward delta (premium-unadjusted)**.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * const surface = new core.FxDeltaVolSurface(
 *   "EURUSD-VOL",
 *   [1],
 *   [0.12],
 *   [0.01],
 *   [0.002]
 * );
 * console.log(surface.id, surface.numExpiries);
 * surface.free();
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
   * @returns An `FxDeltaVolSurface` handle.
   * @param id - Stable surface identifier.
   * @param expiries - Strictly increasing positive expiry times (years).
   * @param atmVols - ATM delta-neutral straddle vols per expiry.
   * @param rr25d - 25-delta risk reversal per expiry (call vol − put vol).
   * @param bf25d - 25-delta butterfly per expiry (wing avg − ATM).
   * @param rr10d - Optional 10-delta risk reversal per expiry.
   * @param bf10d - Optional 10-delta butterfly per expiry.
   * @throws Error - Throws a JavaScript exception if `rr10d` and `bf10d` are not both present or both absent; quote arrays are empty or have mismatched lengths; expiries are not finite, positive, and strictly increasing; ATM vols are not finite and positive; or any risk reversal or butterfly is non-finite.
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
}

/**
 * Monte Carlo pricer result (JSON object from Rust).
 */
export interface MonteCarloEstimateJson {
  /**
   * Discounted mean estimate in `currency` units.
   */
  mean: number;
  /**
   * ISO-4217 currency code of the estimate.
   */
  currency: string;
  /**
   * Standard error of the mean estimate, in the same units as `mean`.
   */
  stderr: number;
  /**
   * Sample standard deviation (absent when not computed).
   */
  std_dev?: number;
  /**
   * Lower bound of the reported confidence interval, in the same units as `mean`.
   */
  ci_lower: number;
  /**
   * Upper bound of the reported confidence interval, in the same units as `mean`.
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
   * ISO-4217 currency of every amount.
   */
  currency: string;
  /**
   * ISO-8601 calculation date.
   */
  date: string;
  /**
   * ISO-8601 settlement date after the contractual business-day lag.
   */
  settlement_date: string;
  /**
   * Signed mark-to-market exposure; positive means the counterparty owes the desk.
   */
  gross_exposure: number;
  /**
   * Signed net mark-to-market exposure, in the caller's currency.
   */
  net_exposure: number;
  /**
   * Amount the desk pays, including new postings and collateral returned.
   */
  post_amount: number;
  /**
   * Amount the desk receives, including collections and collateral returned to it.
   */
  collect_amount: number;
  /**
   * Net desk cash outflow: post minus collect.
   */
  net_margin: number;
  /**
   * True when the post or collect amount is strictly positive.
   */
  requires_call: boolean;
}

/**
 * Bilateral XVA result (JSON object from Rust).
 *
 * Adjustments are positive when they cost the desk and compose as
 * `total_xva = cva - dva + fva + mva`. Optional funding legs are absent when
 * they were not computed.
 */
export interface XvaResultJson {
  /**
   * CVA: expected loss from counterparty default.
   */
  cva: number;
  /**
   * DVA: own-default benefit. Absent when not computed.
   */
  dva?: number;
  /**
   * FVA: net funding cost/benefit. Absent when no funding config was given.
   */
  fva?: number;
  /**
   * MVA: funding cost of posted initial margin. Absent when no `im_profile`
   * was given.
   */
  mva?: number;
  /**
   * Required all-in adjustment = `cva - dva + fva + mva`.
   */
  total_xva: number;
  /**
   * Expected positive exposure profile as `[time, value]` pairs.
   */
  epe_profile: Array<[number, number]>;
  /**
   * Expected negative exposure profile as `[time, value]` pairs.
   */
  ene_profile: Array<[number, number]>;
  /**
   * Potential future exposure profile as `[time, value]` pairs.
   */
  pfe_profile: Array<[number, number]>;
  /**
   * Maximum PFE across the profile.
   */
  max_pfe: number;
  /**
   * Effective EPE profile as `[time, value]` pairs.
   */
  effective_epe_profile: Array<[number, number]>;
  /**
   * Time-weighted average effective EPE (regulatory scalar).
   */
  effective_epe: number;
}

/**
 * Forecast backtest metrics (JSON object from Rust).
 */
export interface BacktestForecastMetricsJson {
  /**
   * Mean absolute error of the forecast versus realized values.
   */
  mae: number;
  /**
   * Mean absolute percentage error, in percent; `NaN` when no sample had a
   * non-zero actual (see `mape_effective_n`).
   */
  mape: number;
  /**
   * Number of samples that contributed to `mape` (those with a non-zero actual).
   */
  mape_effective_n: number;
  /**
   * Symmetric mean absolute percentage error, in percent, bounded in `[0, 200]`.
   */
  smape: number;
  /**
   * Root-mean-square error of the forecast versus realized values.
   */
  rmse: number;
  /**
   * Number of forecast observations in the backtest window.
   */
  n: number;
}

/**
 * Gross-leverage impact of a liability management exercise.
 * Leverage is gross debt over EBITDA, so `8.0` reads as 8.0x.
 */
export interface LmeLeverageImpact {
  /**
   * Gross debt of the target instrument before the exercise.
   */
  pre_total_debt: number;
  /**
   * Gross debt of the target instrument after the exercise.
   */
  post_total_debt: number;
  /**
   * Gross debt over EBITDA before the exercise, as a multiple.
   */
  pre_leverage: number;
  /**
   * Gross debt over EBITDA after the exercise, as a multiple.
   */
  post_leverage: number;
  /**
   * Turns of leverage removed: `pre_leverage - post_leverage`.
   */
  leverage_reduction: number;
}

/**
 * Hold-versus-tender economics of a distressed exchange offer.
 */
export interface ExchangeOfferAnalysis {
  /**
   * Canonical offer structure, echoed back from the request.
   */
  exchange_type: 'par_for_par' | 'discount' | 'uptier' | 'downtier';
  /**
   * Present value of the existing claim if it is not tendered.
   */
  old_npv: number;
  /**
   * Present value of the new instrument received on tendering.
   */
  new_npv: number;
  /**
   * Cash consent or early-tender fee.
   */
  consent_fee: number;
  /**
   * Estimated value of attached equity or warrants.
   */
  equity_sweetener_value: number;
  /**
   * Total tender consideration: `new_npv + consent_fee + equity_sweetener_value`.
   */
  tender_total: number;
  /**
   * Tender consideration less the hold-out present value.
   */
  delta_npv: number;
  /**
   * Hold-out recovery fraction that matches the tender; capped at 1.0.
   */
  breakeven_recovery: number;
  /**
   * True when `tender_total` exceeds `old_npv * 1.02`.
   */
  tender_recommended: boolean;
}

/**
 * Issuer-side economics of a liability management exercise.
 */
export interface LmeAnalysis {
  /**
   * Canonical LME structure, echoed back from the request.
   */
  lme_type: 'open_market_repurchase' | 'tender_offer' | 'amend_and_extend' | 'dropdown';
  /**
   * Cash paid by the issuer, in the caller's monetary unit.
   */
  cost: number;
  /**
   * Face amount retired; zero for structures that do not extinguish debt.
   */
  notional_reduction: number;
  /**
   * Par retired less cash paid — the discount captured by the issuer.
   */
  discount_capture: number;
  /**
   * Discount captured as a fraction of par retired; zero when no par is retired.
   */
  discount_capture_pct: number;
  /**
   * Value fraction diverted from non-participating holders; nonzero only for a dropdown.
   */
  remaining_holder_impact_pct: number;
  /**
   * Gross-leverage block, or null when no positive EBITDA was supplied.
   */
  leverage_impact: LmeLeverageImpact | null;
}

/**
 * Namespaced TypeScript entry points for core calculations and types.
 * @example
 * ```typescript
 * import init, { core } from "finstack-quant-wasm";
 * await init();
 * console.log(core.mean([1, 2, 3]));
 * ```
 */
export interface CoreNamespace {
  /**
   * ISO-4217 currency constructor (`new core.Currency("USD")`).
   */
  Currency: CurrencyConstructor;
  /**
   * Currency-tagged decimal amount constructor.
   */
  Money: MoneyConstructor;
  /**
   * Decimal interest-rate constructor (0.05 is 5%).
   */
  Rate: RateConstructor;
  /**
   * Basis-point quantity constructor (1 is 0.01%).
   */
  Bps: BpsConstructor;
  /**
   * Percentage quantity constructor (5 is 5%).
   */
  Percentage: PercentageConstructor;
  /**
   * Day-count convention constructor and named factories.
   */
  DayCount: DayCountConstructor;
  /**
   * Optional market metadata for day-count calculations.
   */
  DayCountContext: DayCountContextConstructor;
  /**
   * Period-length constructor and named factories (`3M`, `1Y`).
   */
  Tenor: TenorConstructor;
  /**
   * Create a date and return it as epoch days (days since 1970-01-01).
   * @returns Days since 1970-01-01 (Unix epoch).
   * @param year - Four-digit calendar year component of the supplied date.
   * @param month - Calendar month number from 1 through 12.
   * @param day - Calendar day number within the selected month.
   * @throws Error - Throws a JavaScript exception if `month` is outside `1..=12` or the supplied year, month, and day do not form a representable calendar date.
   */
  createDate(year: number, month: number, day: number): number;
  /**
   * Convert epoch days back to `[year, month, day]` as a JS array-compatible triple.
   * @returns `[year, month, day]` as an `Int32Array`, with month in `1..=12`.
   * @param days - Number of days since 1970-01-01 to decompose into year, month, and day.
   * @throws Error - Throws a JavaScript exception if `days` is outside the representable date range.
   */
  dateFromEpochDays(days: number): Int32Array;
  /**
   * Adjust a date (epoch days) according to a business-day convention and calendar.
   *
   * Returns the adjusted date as epoch days.
   * @returns Adjusted date as days since 1970-01-01.
   * @param epochDays - Unadjusted date as days since 1970-01-01.
   * @param convention - Business-day adjustment convention string accepted by the date API.
   * @param calendarCode - Registered holiday-calendar identifier used to find business days.
   * @throws Error - Throws a JavaScript exception if `epochDays` is outside the representable date range, `convention` is unrecognized, `calendarCode` is unknown, or adjustment cannot produce a representable business date.
   */
  adjust(epochDays: number, convention: string, calendarCode: string): number;
  /**
   * Return the list of available calendar codes.
   * @returns Registered holiday-calendar identifiers, sorted alphabetically.
   */
  availableCalendars(): string[];
  /**
   * Discount-factor curve constructor.
   */
  DiscountCurve: DiscountCurveConstructor;
  /**
   * Credit hazard-rate curve constructor.
   */
  HazardCurve: HazardCurveConstructor;
  /**
   * Index forward-rate curve constructor.
   */
  ForwardCurve: ForwardCurveConstructor;
  /**
   * SABR swaption volatility cube constructor.
   */
  VolCube: VolCubeConstructor;
  /**
   * FX delta-quoted volatility surface constructor.
   */
  FxDeltaVolSurface: FxDeltaVolSurfaceConstructor;
  /**
   * FX conversion timing-policy constructor.
   */
  FxConversionPolicy: FxConversionPolicyConstructor;
  /**
   * Resolved FX quote result constructor.
   */
  FxRateResult: FxRateResultConstructor;
  /**
   * Cross-currency FX matrix constructor.
   */
  FxMatrix: FxMatrixConstructor;
  /**
   * USD quotation-style constructor (`direct` / `indirect` versus USD).
   */
  FxQuoteConvention: FxQuoteConventionConstructor;
  /**
   * Market FX pair-convention prototype; instances come from `fxPairConvention`.
   */
  FxPairConvention: FxPairConventionConstructor;
  /**
   * Order two currencies into the market CCY1/CCY2 pair.
   *
   * Priority is EUR > GBP > AUD > NZD > USD > other, with a stable ISO-4217
   * alphabetic tie-break when both sides share the same rank.
   * @param a - First currency ISO code of the unordered pair. Need not be market CCY1.
   * @param b - Second currency ISO code of the unordered pair. Need not be market CCY2.
   * @returns A two-element array `[CCY1, CCY2]` of `Currency` handles in market order.
   * @throws Error - Throws a JavaScript exception if either code is not a recognized ISO-4217 alphabetic currency.
   */
  fxMarketPair(a: string, b: string): Currency[];
  /**
   * Market convention for an unordered currency pair.
   *
   * Returned `base` / `quote` are always the market CCY1/CCY2, even when the
   * arguments are inverted.
   * @param base - One currency ISO code of the pair. Orientation is ignored.
   * @param quote - The other currency ISO code of the pair. Orientation is ignored.
   * @returns Market CCY1/CCY2, USD quotation, pip size, and standard spot lag.
   * @throws Error - Throws a JavaScript exception if either code is not a recognized ISO-4217 alphabetic currency.
   */
  fxPairConvention(base: string, quote: string): FxPairConvention;
  /**
   * Pip size in outright-rate units for a currency pair.
   *
   * Returns `0.01` when either side is JPY, KRW, or HUF; otherwise `0.0001`.
   * Argument order does not matter.
   * @param base - One currency ISO code of the pair. Order is not significant.
   * @param quote - The other currency ISO code of the pair. Order is not significant.
   * @returns Pip size as a decimal increment of the outright FX rate.
   * @throws Error - Throws a JavaScript exception if either code is not a recognized ISO-4217 alphabetic currency.
   */
  fxPipSize(base: string, quote: string): number;
  /**
   * Reciprocal of a strictly positive finite FX rate.
   * @param rate - Outright FX rate to invert, in quote-per-base units. Must be finite and strictly positive; the reciprocal must also be a valid FX rate.
   * @returns `1 / rate` when that reciprocal is a valid FX rate.
   * @throws Error - Throws a JavaScript exception if `rate` is non-finite, non-positive, or when `1 / rate` is not a usable FX rate (overflow to infinity, zero, or a negative value).
   */
  invertFxRate(rate: number): number;
  /**
   * Apply a lower-triangular factor L to a vector z, returning `L z`.
   *
   * This is the Cholesky "apply" step that turns independent standard normals
   * into correlated normals: if `A = L L^T` and `z ~ N(0, I)`, then
   * `L z ~ N(0, A)`. Accepts L as `n * n` row-major entries; only the lower
   * triangle is read and the upper triangle is assumed zero.
   * @returns Transformed vector `L z` as a `Float64Array` of length `n`.
   * @param l - Lower-triangular Cholesky factor as a flat row-major array of n × n entries.
   * @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
   * @param z - Vector of length n to transform, typically independent standard-normal draws.
   * @throws Error - Throws a JavaScript exception if `n * n` overflows, `l` does not contain exactly `n * n` entries, or `z` does not contain exactly `n` entries.
   */
  applyLowerTriangular(l: NumericArray, n: number, z: NumericArray): Float64Array;
  /**
   * Cholesky decomposition for a flat row-major matrix.
   *
   * Accepts a `Float64Array`/`number[]` containing `n * n` row-major entries
   * and returns a flat lower-triangular factor.
   * @param matrix - Flat row-major `n * n` entries of a symmetric positive-definite matrix.
   * @param n - Positive square-matrix dimension; `matrix` must contain exactly `n * n` entries.
   * @returns Lower-triangular factor L as a flat row-major `Float64Array`.
   * @throws Error - Throws a JavaScript exception if `n * n` overflows, `matrix` does not contain exactly `n * n` entries, or the matrix contains a non-finite value, is singular, or is not positive definite.
   */
  choleskyDecomposition(matrix: NumericArray, n: number): Float64Array;
  /**
   * Solve a symmetric positive-definite linear system from a flat Cholesky factor.
   * @returns Solution vector `x` as a `Float64Array` of length `n`.
   * @param chol - Lower-triangular Cholesky factor as a flat row-major `n * n` array.
   * @param b - Right-hand-side vector of a linear system, aligned with the Cholesky factor dimension.
   * @param n - Positive square-matrix dimension; flat arrays must contain n × n entries.
   * @throws Error - Throws a JavaScript exception if `n * n` overflows, `chol` does not contain exactly `n * n` entries, `b` does not contain `n` entries, or a diagonal factor is singular.
   */
  choleskySolve(chol: NumericArray, b: NumericArray, n: number): Float64Array;
  /**
   * Arithmetic mean over a typed numeric array.
   * @param data - Numeric observations in input order; an empty series yields 0.0.
   * @returns Arithmetic mean of `data`, or 0.0 when `data` is empty.
   */
  mean(data: NumericArray): number;
  /**
   * Sample variance over a typed numeric array.
   * @param data - Sample observations in input order; fewer than two points yield 0.0.
   * @returns Unbiased sample variance, or 0.0 when `data` has fewer than two points.
   */
  variance(data: NumericArray): number;
  /**
   * Population variance over a typed numeric array.
   * @param data - Observations in input order; fewer than two points yield 0.0.
   * @returns Population variance, or 0.0 when `data` has fewer than two points.
   */
  populationVariance(data: NumericArray): number;
  /**
   * Pearson correlation over typed numeric arrays.
   * @param x - First numeric series; must have the same length as `y`.
   * @param y - Second numeric series, aligned one-for-one with `x`.
   * @returns Sample correlation in `[-1, 1]`, or NaN when a series has fewer than two points.
   */
  correlation(x: NumericArray, y: NumericArray): number;
  /**
   * Sample covariance over typed numeric arrays.
   * @param x - First numeric series; must have the same length as `y`.
   * @param y - Second numeric series, aligned one-for-one with `x`.
   * @returns Unbiased sample covariance, or 0.0 when a series has fewer than two points.
   */
  covariance(x: NumericArray, y: NumericArray): number;
  /**
   * Empirical quantile over a typed numeric array.
   * @param data - Sample observations in input order; empty or non-finite data yields NaN.
   * @param q - Quantile probability in `[0, 1]`; values outside that range yield NaN.
   * @returns R-7 interpolated quantile, or NaN when `data` is empty or non-finite.
   */
  quantile(data: NumericArray, q: number): number;
  /**
   * Standard normal CDF Φ(x).
   * @param x - Real-valued point at which to evaluate Φ; any finite or infinite `x` is accepted.
   * @returns Probability in `(0, 1)` for finite `x`, with the usual ±∞ limits.
   */
  normCdf(x: number): number;
  /**
   * Standard normal PDF φ(x).
   * @param x - Real-valued point at which to evaluate φ.
   * @returns Density at `x`; φ(0) is `1/sqrt(2π)`.
   */
  normPdf(x: number): number;
  /**
   * Inverse standard normal CDF Φ⁻¹(p).
   * @param p - Probability input strictly between 0 and 1 for the inverse normal distribution.
   * @returns Standard-normal quantile for probability `p`.
   */
  standardNormalInvCdf(p: number): number;
  /**
   * Error function erf(x).
   * @param x - Real-valued argument to erf; the function is odd, so erf(-x) = -erf(x).
   * @returns erf(x) in `(-1, 1)` for finite `x`.
   */
  erf(x: number): number;
  /**
   * Natural logarithm of the Gamma function ln(Γ(x)).
   * @param x - Real argument; must be positive and away from the non-positive integers.
   * @returns ln(Γ(x)); ln(Γ(1)) is 0 and ln(Γ(n+1)) is ln(n!).
   */
  lnGamma(x: number): number;
  /**
   * Kahan compensated summation over a typed numeric array.
   * @param values - Finite numeric terms in summation or scan order.
   * @returns Compensated sum of `values` in input order.
   */
  kahanSum(values: NumericArray): number;
  /**
   * Neumaier compensated summation over a typed numeric array.
   * @param values - Finite numeric terms in summation or scan order.
   * @returns Compensated sum of `values`, robust to mixed-sign cancellation.
   */
  neumaierSum(values: NumericArray): number;
  /**
   * Count the longest consecutive run of strictly positive values in a typed array.
   * @param values - Finite numeric terms in summation or scan order.
   * @returns Length of the longest run of strictly positive observations.
   */
  longestPositiveRun(values: NumericArray): number;
}

/**
 * Namespaced TypeScript entry point for core APIs.
 */
export declare const core: CoreNamespace;

// --- analytics ------------------------------------------------------------

/**
 * JavaScript `number[]` or `Float64Array` accepted by numeric WASM entry points.
 */
export type NumericArray = number[] | Float64Array;
/**
 * Nested numeric arrays represented as an outer collection of numeric vectors.
 * Each API specifies the semantic meaning and required length of the outer and inner dimensions.
 */
export type NumericMatrix = NumericArray[];

/**
 * Descriptive statistics returned by `peerStats`.
 */
export interface PeerStatsJson {
  /**
   * Number of peer observations in the sample.
   */
  count: number;
  /**
   * Arithmetic mean of the peer metric, in the same units as the input.
   */
  mean: number;
  /**
   * Median of the peer metric, in the same units as the input.
   */
  median: number;
  /**
   * Sample standard deviation of the peer metric.
   */
  std_dev: number;
  /**
   * Minimum peer observation, in the same units as the input.
   */
  min: number;
  /**
   * Maximum peer observation, in the same units as the input.
   */
  max: number;
  /**
   * First quartile of the peer metric.
   */
  q1: number;
  /**
   * Third quartile of the peer metric.
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
   * OLS intercept of the fitted peer regression.
   */
  intercept: number;
  /**
   * OLS slope of the fitted peer regression.
   */
  slope: number;
  /**
   * Coefficient of determination of the fitted peer regression, in `[0, 1]`.
   */
  r_squared: number;
  /**
   * Regression prediction at the subject company's independent-variable value.
   */
  fitted_value: number;
  /**
   * Subject residual: observed dependent value minus the fitted value.
   */
  residual: number;
  /**
   * Number of paired peer observations used in the fit.
   */
  n: number;
}

/**
 * Per-dimension decomposition in a relative value score.
 */
export interface DimensionScoreJson {
  /**
   * Dimension name matching the requested relative-value metric.
   */
  label: string;
  /**
   * Peer percentile rank of the subject on this dimension, on a 0-1 scale.
   */
  percentile: number;
  /**
   * Standardized subject score versus the peer sample on this dimension.
   */
  z_score: number;
  /**
   * Optional OLS residual when this dimension was scored by regression; `null` otherwise.
   */
  regression_residual: number | null;
  /**
   * Optional regression R² when this dimension was scored by regression; `null` otherwise.
   */
  r_squared: number | null;
  /**
   * Weight of this dimension in the composite relative-value score.
   */
  weight: number;
}

/**
 * Composite relative value result returned by `scoreRelativeValue`.
 */
export interface RelativeValueResultJson {
  /**
   * Identifier of the subject company being scored.
   */
  company_id: string;
  /**
   * Weighted composite rich/cheap score across the requested dimensions.
   */
  composite_score: number;
  /**
   * Per-dimension percentile, z-score, and optional regression diagnostics.
   */
  dimensions: DimensionScoreJson[];
  /**
   * Score confidence in `[0, 1]` from peer coverage and dimension completeness.
   */
  confidence: number;
  /**
   * Number of peer companies included in the scoring sample.
   */
  peer_count: number;
}

/**
 * Structured formula explanation returned by `explainFormula`.
 */
export interface FormulaExplanationJson {
  /**
   * Statement-model node whose formula was explained.
   */
  node_id: string;
  /**
   * Period identifier at which the formula was evaluated.
   */
  period_id: string;
  /**
   * Evaluated node value after applying the formula in this period.
   */
  final_value: number;
  /**
   * Node kind in the statement model, such as input, formula, or calculated.
   */
  node_type: string;
  /**
   * Source formula text when the node is formula-driven; omitted otherwise.
   */
  formula_text?: string | null;
  /**
   * Ordered formula components that sum or combine to `final_value`.
   */
  breakdown: FormulaExplanationStepJson[];
}

/**
 * One component in a structured formula explanation.
 */
export interface FormulaExplanationStepJson {
  /**
   * Label of this formula term, such as a referenced node id or literal.
   */
  component: string;
  /**
   * Numeric contribution of this term in the explained period.
   */
  value: number;
  /**
   * Operator that combines this term with the running total, when present.
   */
  operation?: string | null;
}

/**
 * A single drawdown episode returned by `drawdownDetails`.
 */
export interface DrawdownEpisode {
  /**
   * ISO-8601 date when the drawdown episode began.
   */
  start: string;
  /**
   * ISO-8601 date of the trough (maximum drawdown) within the episode.
   */
  valley: string;
  /**
   * ISO-8601 recovery date, or `null` if the episode is still open.
   */
  end: string | null;
  /**
   * Episode length in calendar days from `start` to `end` or the series end.
   */
  duration_days: number;
  /**
   * Peak-to-trough decline as a negative decimal fraction (for example `-0.12`).
   */
  max_drawdown: number;
  /**
   * Recovery threshold used to mark the episode as nearly recovered.
   */
  near_recovery_threshold: number;
  /**
   * True when the episode is truncated because the series starts in drawdown.
   */
  truncated_at_start: boolean;
}

/**
 * Aggregate statistics for grouped periodic returns.
 */
export interface PeriodStats {
  /**
   * Best period return as a decimal fraction.
   */
  best: number;
  /**
   * Worst period return as a decimal fraction.
   */
  worst: number;
  /**
   * Longest run of strictly positive period returns.
   */
  consecutive_wins: number;
  /**
   * Longest run of strictly negative period returns.
   */
  consecutive_losses: number;
  /**
   * Share of periods with a strictly positive return, in `[0, 1]`.
   */
  win_rate: number;
  /**
   * Mean period return as a decimal fraction.
   */
  avg_return: number;
  /**
   * Mean of strictly positive period returns as a decimal fraction.
   */
  avg_win: number;
  /**
   * Mean of strictly negative period returns as a decimal fraction.
   */
  avg_loss: number;
  /**
   * Average win divided by the absolute average loss; may be infinite.
   */
  payoff_ratio: number;
  /**
   * Gross profits divided by gross losses; may be infinite.
   */
  profit_factor: number;
  /**
   * Count of profitable periods over count of losing periods; may be infinite.
   */
  cpc_ratio: number;
  /**
   * Kelly-criterion fraction from the period win rate and payoff ratio.
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
   * ISO-8601 dates aligned with the metric series, in chronological order.
   */
  dates: string[];
  /**
   * Rolling Sharpe ratio when produced by `rollingSharpe`.
   */
  sharpe?: Float64Array;
  /**
   * Rolling Sortino ratio when produced by `rollingSortino`.
   */
  sortino?: Float64Array;
  /**
   * Rolling volatility when produced by `rollingVolatility`.
   */
  volatility?: Float64Array;
  /**
   * Rolling compounded return when produced by `rollingReturns`.
   */
  return?: Float64Array;
}

/**
 * OLS beta result with standard error and 95% confidence interval.
 *
 * The interval uses Student-t critical values for finite samples and an
 * asymptotic normal approximation once n - 2 >= 240.
 */
export interface BetaResult {
  /**
   * OLS beta versus the benchmark.
   */
  beta: number;
  /**
   * Standard error of the beta estimate.
   */
  std_err: number;
  /**
   * Lower bound of the 95% confidence interval for beta.
   */
  ci_lower: number;
  /**
   * Upper bound of the 95% confidence interval for beta.
   */
  ci_upper: number;
}

/**
 * Single-factor greeks (annualized Jensen alpha, beta, R², adjusted R²).
 */
export interface GreeksResult {
  /**
   * Annualized Jensen alpha versus the benchmark, as a decimal fraction.
   */
  alpha: number;
  /**
   * OLS beta versus the benchmark.
   */
  beta: number;
  /**
   * Coefficient of determination of the benchmark regression.
   */
  r_squared: number;
  /**
   * Adjusted R² of the benchmark regression.
   */
  adjusted_r_squared: number;
}

/**
 * Rolling greeks output aligned with rolling-window end dates.
 */
export interface RollingGreeksResult {
  /**
   * ISO-8601 end dates of each rolling window, in chronological order.
   */
  dates: string[];
  /**
   * Annualized Jensen alpha at each window end, as decimal fractions.
   */
  alphas: Float64Array;
  /**
   * OLS beta at each window end.
   */
  betas: Float64Array;
}

/**
 * Multi-factor regression result. Alpha is the raw regression intercept, annualized.
 */
export interface MultiFactorResult {
  /**
   * Annualized regression intercept, as a decimal fraction.
   */
  alpha: number;
  /**
   * Factor betas, one per supplied factor series, in input order.
   */
  betas: number[];
  /**
   * Coefficient of determination of the multi-factor regression.
   */
  r_squared: number;
  /**
   * Adjusted R² of the multi-factor regression.
   */
  adjusted_r_squared: number;
  /**
   * Residual volatility of the regression, as an annualized decimal fraction.
   */
  residual_vol: number;
}

/**
 * Period-to-date lookback returns (per ticker) returned by `lookbackReturns`.
 */
export interface LookbackReturns {
  /**
   * Ticker names aligned with every per-ticker vector below.
   */
  ticker_names: string[];
  /**
   * Month-to-date simple returns per ticker in `tickerNames()` order.
   */
  mtd: number[];
  /**
   * Quarter-to-date simple returns per ticker in `tickerNames()` order.
   */
  qtd: number[];
  /**
   * Year-to-date simple returns per ticker in `tickerNames()` order.
   */
  ytd: number[];
  /**
   * Fiscal-year-to-date simple returns per ticker in `tickerNames()` order.
   */
  fytd: number[];
}

/**
 * One calendar-bucketed return emitted by `Performance.periodicReturns`.
 */
export interface PeriodicReturnPoint {
  /**
   * ISO-8601 date of the final observation in the calendar bucket.
   */
  date: string;
  /**
   * Compounded simple return as a decimal fraction (`0.01` means 1%).
   */
  value: number;
}

/**
 * Stateful performance analytics engine over a panel of ticker series.
 *
 * `Performance` is the single entry point exposed to JS. Construct from
 * a price matrix (`new Performance(...)`) or a return matrix
 * (`Performance.fromReturns(...)`); every metric is then reachable as
 * an instance method.
 *
 * The `prices` and `returns` supplied to the panel constructors are ticker-major
 * and column-oriented; matrix inputs to other methods follow their parameter
 * documentation.
 *
 * All multi-ticker scalar outputs come back as `number[]` indexed by the
 * panel's ticker order; vector / per-ticker / structured outputs are
 * serialized to plain JS objects (e.g. `DatedSeries`, `BetaResult[]`).
 */
export declare class Performance {
  /**
   * Construct from a ticker-major, column-oriented price matrix. The outer
   * element selects a ticker, and each inner series is aligned to `dates`.
   * @param dates - ISO-8601 observation dates in ascending order, with one entry per value in each inner price series.
   * @param prices - Ticker-major, column-oriented matrix where `prices[tickerIdx][dateIdx]` is the price for `tickerIdx` at `dates[dateIdx]`.
   * @param tickerNames - Ticker labels aligned with the outer elements of `prices`.
   * @param benchmarkTicker - Optional ticker label to use as the benchmark return series.
   * @param frequency - Optional observation frequency token; defaults to daily.
   * @throws Error - Rejects malformed dates or matrices, invalid prices, unsupported frequencies, and an unknown benchmark ticker.
   */
  constructor(
    dates: string[],
    prices: NumericMatrix,
    tickerNames: string[],
    benchmarkTicker?: string | null,
    frequency?: string
  );
  /**
   * Construct from a ticker-major, column-oriented return matrix. The outer
   * element selects a ticker, and each inner series is aligned to `dates`.
   * @param dates - ISO-8601 observation dates in ascending order, with one entry per value in each inner return series.
   * @param returns - Ticker-major, column-oriented simple decimal return matrix where `returns[tickerIdx][dateIdx]` is the return for `tickerIdx` at `dates[dateIdx]`.
   * @param tickerNames - Ticker labels aligned with the outer elements of `returns`.
   * @param benchmarkTicker - Optional ticker label to use as the benchmark return series.
   * @param frequency - Optional observation frequency token; defaults to daily.
   * @returns A `Performance` handle over the supplied return panel.
   * @throws Error - Rejects malformed dates or matrices and invalid benchmark or frequency inputs.
   */
  static fromReturns(
    dates: string[],
    returns: NumericMatrix,
    tickerNames: string[],
    benchmarkTicker?: string | null,
    frequency?: string
  ): Performance;
  /**
   * Restrict subsequent analytics to `[start, end]`.
   * @param start - Inclusive ISO-8601 start date for the active analysis window.
   * @param end - Inclusive ISO-8601 end date for the active analysis window.
   * @throws Error - Rejects `start` or `end` when it is not a valid ISO-8601 calendar date.
   */
  resetDateRange(start: string, end: string): void;
  /**
   * Change the benchmark ticker.
   * @param ticker - Existing ticker label to use as the benchmark return series.
   * @throws Error - Rejects `ticker` when it does not match a loaded ticker name.
   */
  resetBenchTicker(ticker: string): void;
  /**
   * Ticker names in column order.
   * @returns Ticker labels in column order as a JavaScript string array.
   * @throws Error - Rejects if the ticker-name vector cannot be serialized to JavaScript.
   */
  tickerNames(): string[];
  /**
   * Benchmark column index.
   * @returns Zero-based index of the benchmark ticker in `tickerNames()`.
   */
  benchmarkIdx(): number;
  /**
   * Observation frequency token.
   * @returns Frequency string such as `"daily"` or `"monthly"`.
   */
  frequency(): string;
  /**
   * Full return-aligned date grid as ISO date strings (independent of any active window).
   * @returns Full panel dates as ISO-8601 strings, ignoring any `resetDateRange` window.
   */
  dates(): string[];
  /**
   * Dates of the currently active analysis window as ISO date strings.
   * @returns ISO-8601 dates of the active analysis window, in chronological order.
   */
  activeDates(): string[];
  /**
   * Dates for one ticker's active return series as ISO date strings.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @returns ISO-8601 dates for that ticker's active return series, in chronological order.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns.
   */
  activeDatesForTicker(tickerIdx: number): string[];
  /**
   * Compound annual growth rate per asset.
   *
   * `dayCount` omitted or `"act365_25"` uses Act/365.25. Other values are
   * core DayCount names such as `"act_365f"` or `"bus_252"`. `bus_252`
   * requires `calendarId`.
   * @param dayCount - Optional day-count: `"act365_25"` or a core name such as `"act_365f"`; defaults to Act/365.25.
   * @param calendarId - Optional holiday-calendar id; required for `bus_252`.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects an unknown day-count or calendar id, a missing calendar when `bus_252` is requested, or a ticker whose active range has no positive holding period.
   */
  cagr(dayCount?: string, calendarId?: string): Float64Array;
  /**
   * Mean periodic return per asset (annualized by default).
   * @param annualize - Whether to annualize by the configured frequency; defaults to true.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  meanReturn(annualize?: boolean): Float64Array;
  /**
   * Return volatility per asset (annualized by default).
   * @param annualize - Whether to annualize by the configured frequency; defaults to true.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  volatility(annualize?: boolean): Float64Array;
  /**
   * Sharpe ratio per asset for the given risk-free rate.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  sharpe(riskFreeRate?: number): Float64Array;
  /**
   * Sortino ratio; mar is a per-period threshold.
   * @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  sortino(mar?: number): Float64Array;
  /**
   * Calmar ratio (CAGR / |max drawdown|) over the active window, not
   * Young's 36-month CTA definition.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  calmar(): Float64Array;
  /**
   * Maximum drawdown per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  maxDrawdown(): Float64Array;
  /**
   * Mean drawdown per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  meanDrawdown(): Float64Array;
  /**
   * Historical value-at-risk per asset at the given confidence level.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  valueAtRisk(confidence?: number): Float64Array;
  /**
   * Expected shortfall (CVaR) per asset at the given confidence level.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  expectedShortfall(confidence?: number): Float64Array;
  /**
   * Tracking error versus the benchmark per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  trackingError(): Float64Array;
  /**
   * Information ratio versus the benchmark per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  informationRatio(): Float64Array;
  /**
   * Return skewness per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  skewness(): Float64Array;
  /**
   * Excess kurtosis of returns per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  kurtosis(): Float64Array;
  /**
   * Geometric mean return per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  geometricMean(): Float64Array;
  /**
   * Downside deviation; mar is a per-period threshold.
   * @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  downsideDeviation(mar?: number): Float64Array;
  /**
   * Longest drawdown duration in calendar days per asset.
   * @returns Per-ticker longest drawdown duration in calendar days, as a JavaScript number array.
   * @throws Error - Rejects if the duration vector cannot be serialized to JavaScript.
   */
  maxDrawdownDuration(): number[];
  /**
   * Empyrical-style annualized geometric up-capture.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  upCapture(): Float64Array;
  /**
   * Empyrical-style annualized geometric down-capture.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  downCapture(): Float64Array;
  /**
   * Empyrical-style annualized geometric up/down capture ratio.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  captureRatio(): Float64Array;
  /**
   * Omega ratio per asset for the given threshold return.
   * @param threshold - Per-period threshold return as a decimal; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  omegaRatio(threshold?: number): Float64Array;
  /**
   * Treynor ratio per asset for the given risk-free rate.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  treynor(riskFreeRate?: number): Float64Array;
  /**
   * Gain-to-pain ratio per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  gainToPain(): Float64Array;
  /**
   * Ulcer index per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  ulcerIndex(): Float64Array;
  /**
   * Martin ratio (excess return over ulcer index) per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  martinRatio(): Float64Array;
  /**
   * Recovery factor (total return over max drawdown) per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  recoveryFactor(): Float64Array;
  /**
   * Pain index (mean drawdown magnitude) per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  painIndex(): Float64Array;
  /**
   * Pain ratio (excess return over pain index) per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  painRatio(riskFreeRate?: number): Float64Array;
  /**
   * Tail ratio of upper to lower return quantiles per asset.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  tailRatio(confidence?: number): Float64Array;
  /**
   * R-squared of returns against the benchmark per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  rSquared(): Float64Array;
  /**
   * Share of periods beating the benchmark per asset.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  battingAverage(): Float64Array;
  /**
   * Equal-weight Gaussian value-at-risk per asset.
   *
   * `horizonPeriods` omitted is one-period VaR. A positive `h` scales
   * mean by `h` and volatility by `√h`.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @param horizonPeriods - Optional horizon in observation periods; omitted is one-period VaR.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  parametricVar(confidence?: number, horizonPeriods?: number): Float64Array;
  /**
   * Cornish-Fisher adjusted value-at-risk per asset.
   *
   * `horizonPeriods` omitted is one-period VaR. A positive `h` scales
   * Cornish–Fisher moments to that horizon.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @param horizonPeriods - Optional horizon in observation periods; omitted is one-period VaR.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  cornishFisherVar(confidence?: number, horizonPeriods?: number): Float64Array;
  /**
   * Conditional drawdown-at-risk per asset at the given confidence level.
   * @param confidence - Tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  cdar(confidence?: number): Float64Array;
  /**
   * M-squared (Modigliani) risk-adjusted return per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   */
  mSquared(riskFreeRate?: number): Float64Array;
  /**
   * Modified Sharpe ratio using annualized excess return and corresponding-annual-horizon Cornish-Fisher VaR per asset.
   *
   * The panel frequency supplies the periods-per-year scaling for both terms, including the horizon decay of skewness and excess kurtosis; the denominator is not one-period VaR.
   * @param riskFreeRate - Annualized decimal risk-free rate, decompounded to the panel frequency before constructing annualized excess return; defaults to 0.0.
   * @param confidence - Annual-horizon tail confidence as a decimal probability; defaults to 0.95.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects a `confidence` outside the open interval (0, 1).
   */
  modifiedSharpe(riskFreeRate?: number, confidence?: number): Float64Array;
  /**
   * Sterling ratio over the `n` largest drawdowns per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @param n - Finite non-negative integer count of largest drawdowns; defaults to 5. Invalid numeric values are rejected.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  sterlingRatio(riskFreeRate?: number, n?: number): Float64Array;
  /**
   * Burke ratio over the `n` largest drawdowns per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @param n - Finite non-negative integer count of largest drawdowns; defaults to 5. Invalid numeric values are rejected.
   * @returns Per-ticker values as a Float64Array in `tickerNames()` order.
   * @throws Error - Rejects when any ticker's active range has no positive holding period and therefore cannot produce CAGR.
   */
  burkeRatio(riskFreeRate?: number, n?: number): Float64Array;
  /**
   * Per-period simple returns per asset, as decimal fractions (0.01 = +1%).
   *
   * Canonical accessor for the raw return panel over the active window; prefer
   * it over `excessReturns` with an all-zero risk-free series or un-compounding
   * `cumulativeReturns`. Series are span-aware and therefore ragged across
   * assets on edge-ragged panels.
   * @returns One Float64Array of simple decimal returns per ticker in `tickerNames()` order.
   */
  returns(): Float64Array[];
  /**
   * Per-period simple returns for one asset, as decimal fractions (0.01 = +1%).
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @returns Simple decimal returns for the selected ticker, in date order.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns.
   */
  returnsForTicker(tickerIdx: number): Float64Array;
  /**
   * Cumulative return series per asset.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   */
  cumulativeReturns(): Float64Array[];
  /**
   * The standard per-ticker summary as a table envelope: one row per ticker
   * with 22 metric columns plus a leading `ticker` dimension column. Same
   * rows as the Python `Performance.to_summary_dataframe`.
   * @returns Column-oriented table envelope of the summary metrics.
   * @param riskFreeRate - Annualized risk-free rate as a decimal (0.02 = 2%); affects only `sharpe`. Defaults to 0.0.
   * @param confidence - Tail confidence as a decimal probability applied to VaR, ES and tail ratio; defaults to 0.95.
   * @throws Error - Throws a JavaScript exception if the panel is too short to annualize (`cagr` / `calmar`) or the table cannot be converted.
   */
  summary(riskFreeRate?: number | null, confidence?: number | null): TableEnvelope;
  /**
   * Calendar-bucketed compounded returns for every ticker.
   *
   * The outer array is ticker-major in `tickerNames()` order. Each inner
   * array contains chronological period-end points. Chaining one ticker's
   * decimal `value` fields reconciles with its final `cumulativeReturns()`
   * value.
   *
   * @example
   * ```ts
   * const perf = Performance.fromReturns(
   *   ["2024-01-01", "2024-01-02"],
   *   [[0.01, 0.02]],
   *   ["FUND"],
   * );
   * const [[point]] = perf.periodicReturns();
   * console.log(point.date, point.value); // "2024-01-02", 0.0302
   * ```
   * @param frequency - Optional calendar frequency token: `"daily"`, `"weekly"`, `"monthly"`, `"quarterly"`, `"semi_annual"`, or `"annual"` (pandas offset aliases `D`/`B`, `W`, `M`, `Q`, `A`/`Y` are accepted too); defaults to `"monthly"`.
   * @returns Ticker-major nested arrays of chronological period-end points with simple decimal returns.
   * @throws Error - Rejects an unsupported frequency or a failure to create a point property on the JavaScript result object.
   */
  periodicReturns(frequency?: string): PeriodicReturnPoint[][];
  /**
   * Drawdown series per asset.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   */
  drawdownSeries(): Float64Array[];
  /**
   * Return correlation matrix across assets.
   *
   * Uses the complete-case common window when every ticker has at least
   * two overlapping points; otherwise pairwise intersecting spans, then
   * Higham repair.
   * @returns Square correlation matrix as nested Float64Array rows in `tickerNames()` order.
   * @throws Error - Rejects a degenerate pair or a matrix that cannot be repaired to a valid correlation matrix.
   */
  correlationMatrix(): Float64Array[];
  /**
   * `true` when `correlationMatrix()` had to be Higham-repaired to the nearest valid correlation matrix (ragged panels can yield a raw pairwise estimate that is not positive semi-definite).
   * @returns `true` when the estimate was projected to the nearest correlation matrix.
   * @throws Error - Rejects the same degenerate-pair conditions as `correlationMatrix`.
   * @throws Error - Rejects when a ticker pair is degenerate or Higham repair fails.
   */
  correlationMatrixRepaired(): boolean;
  /**
   * Cumulative outperformance versus the benchmark per asset.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   */
  cumulativeReturnsOutperformance(): Float64Array[];
  /**
   * Difference between asset and benchmark drawdown series.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   */
  drawdownDifference(): Float64Array[];
  /**
   * Excess returns over the supplied risk-free series per asset.
   *
   * `rf` must have one value per active panel date. `nperiods` omitted
   * geometrically decompounds an annual series using the engine frequency;
   * pass `1` when `rf` is already periodic.
   * @param rf - Risk-free return series as decimal values aligned with active panel dates.
   * @param nperiods - Optional periods per year used to decompound annual `rf`; omit to use the engine frequency, or pass `1` for already-periodic `rf`.
   * @returns One Float64Array per ticker in `tickerNames()` order.
   * @throws Error - Rejects when `rf` is neither a numeric JavaScript array nor a `Float64Array`, or when its length differs from the active panel.
   */
  excessReturns(rf: NumericArray, nperiods?: number): Float64Array[];
  /**
   * OLS beta versus the benchmark per asset, with standard error and 95% CI.
   * @returns Per-ticker `{ beta, std_err, ci_lower, ci_upper }` objects in `tickerNames()` order.
   * @throws Error - Rejects if the beta results cannot be serialized to JavaScript.
   */
  beta(): BetaResult[];
  /**
   * Benchmark regression annualized Jensen alpha/beta statistics per asset.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns Per-ticker `{ alpha, beta, r_squared, adjusted_r_squared }` objects in `tickerNames()` order.
   * @throws Error - Rejects if the regression results cannot be serialized to JavaScript.
   */
  greeks(riskFreeRate?: number): GreeksResult[];
  /**
   * Rolling benchmark annualized Jensen alpha/beta for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns `{ dates, alphas, betas }` series for the selected ticker.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created.
   */
  rollingGreeks(tickerIdx: number, window?: number, riskFreeRate?: number): RollingGreeksResult;
  /**
   * Rolling volatility series for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
   * @returns `{ dates, volatility }` series for the selected ticker.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created.
   */
  rollingVolatility(tickerIdx: number, window?: number): DatedSeries;
  /**
   * Rolling Sortino ratio series for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
   * @param mar - Per-period minimum acceptable return as a decimal; defaults to 0.0.
   * @returns `{ dates, sortino }` series for the selected ticker.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created.
   */
  rollingSortino(tickerIdx: number, window?: number, mar?: number): DatedSeries;
  /**
   * Rolling Sharpe ratio series for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count; defaults to 63 periods. Invalid numeric values are rejected.
   * @param riskFreeRate - Annualized decimal risk-free rate; defaults to 0.0.
   * @returns `{ dates, sharpe }` series for the selected ticker.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created.
   */
  rollingSharpe(tickerIdx: number, window?: number, riskFreeRate?: number): DatedSeries;
  /**
   * Rolling compounded return series for one asset over a window.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param window - Finite positive integer observation count. Zero, fractional, non-finite, and out-of-range values are rejected.
   * @returns `{ dates, return }` series for the selected ticker.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the JavaScript result object's properties cannot be created. An overlong `window` returns an empty series; a zero window is rejected.
   */
  rollingReturns(tickerIdx: number, window: number): DatedSeries;
  /**
   * Details of the `n` largest drawdown episodes for one asset.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param n - Finite non-negative integer count of episodes; defaults to 5. Invalid numeric values are rejected.
   * @returns Drawdown episode objects for the selected ticker, largest first.
   * @throws Error - Rejects when `ticker_idx` is outside the loaded ticker columns or the drawdown details cannot be serialized to JavaScript.
   */
  drawdownDetails(tickerIdx: number, n?: number): DrawdownEpisode[];
  /**
   * Multi-factor regression statistics for one asset.
   *
   * Factor series are already-excess. `returnKind` `"excess"` leaves the
   * ticker series unchanged; `"total"` subtracts the geometrically
   * decompounded period risk-free rate from the ticker series only.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param factorReturns - Matrix of aligned already-excess decimal factor-return series, one row per factor.
   * @param returnKind - `"excess"` or `"total"`; defaults to `"excess"`.
   * @param riskFreeRate - Annualized decimal risk-free rate used when `returnKind` is `"total"`; defaults to 0.0.
   * @returns `{ alpha, betas, r_squared, adjusted_r_squared, residual_vol }` for the selected ticker.
   * @throws Error - Rejects a non-numeric `factor_returns` matrix, an unknown `returnKind`, an out-of-range `ticker_idx`, no factors, too few observations, non-finite or length-mismatched inputs, a singular factor design, or a result that cannot be serialized to JavaScript.
   */
  multiFactorGreeks(
    tickerIdx: number,
    factorReturns: NumericMatrix,
    returnKind?: string,
    riskFreeRate?: number
  ): MultiFactorResult;
  /**
   * Period-to-date lookback returns.
   *
   * FYTD is the first observation on or after the fiscal calendar start
   * through `refDate`. Holidays are not skipped. The first included
   * simple return still spans the prior close.
   * @param refDate - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end.
   * @param fiscalYearStartMonth - Optional fiscal-year start month from 1 through 12; defaults to January.
   * @param fiscalYearStartDay - Optional fiscal-year start day; defaults to the first day of the month.
   * @param refDate - ISO-8601 date on which MTD, QTD, YTD, and FYTD windows end.
   * @param fiscalYearStartMonth - Optional fiscal-year start month from 1 through 12; defaults to January.
   * @param fiscalYearStartDay - Optional fiscal-year start day; defaults to the first day.
   * @returns Per-ticker `{ mtd, qtd, ytd, fytd }` numeric arrays of lookback returns as decimal fractions; `fytd` is never null.
   * @throws Error - Rejects an invalid ISO `ref_date`, a fiscal month outside `1..=12`, a fiscal day outside `1..=31`, or a result that cannot be serialized to JavaScript.
   */
  lookbackReturns(
    refDate: string,
    fiscalYearStartMonth?: number,
    fiscalYearStartDay?: number
  ): LookbackReturns;
  /**
   * Aggregated period statistics for one asset at the given frequency.
   * @param tickerIdx - Finite non-negative integer column index in tickerNames order; fractional or out-of-range values are rejected.
   * @param aggregationFrequency - Optional aggregation frequency token; defaults to monthly.
   * @param fiscalYearStartMonth - Optional fiscal-year start month from 1 through 12.
   * @param fiscalYearStartDay - Optional fiscal-year start day within the selected month.
   * @returns Period statistics object for the selected ticker at the requested frequency.
   * @throws Error - Rejects an unsupported `aggregation_frequency`, a fiscal month outside `1..=12`, a fiscal day outside `1..=31`, an out-of-range `ticker_idx`, or period statistics that cannot be serialized to JavaScript.
   */
  periodStats(
    tickerIdx: number,
    aggregationFrequency?: string,
    fiscalYearStartMonth?: number,
    fiscalYearStartDay?: number
  ): PeriodStats;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Namespaced TypeScript entry points for analytics calculations and types.
 * @example
 * ```typescript
 * import init, { analytics } from "finstack-quant-wasm";
 * await init();
 * const factorReturns = analytics.constrainedLeastSquares(
 *   [1, 2],
 *   1,
 *   [0.01, 0.02],
 *   [0.5, 0.5]
 * );
 * console.log(factorReturns[0]);
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
  /**
   * Fit factor returns satisfying the equality constraint `w'Xf = w'r`.
   *
   * Binds Rust `constrained_least_squares` (Jeet & Partani 2023, Appendix
   * A): adds the minimal Lagrangian correction to an unconstrained OLS fit
   * so the corrected factor returns exactly reproduce the weighted
   * realized return `w'r`. Typically used to fit the benchmark factor
   * returns consumed by `portfolio.factorBrinsonAttribution`, which
   * requires factor returns satisfying that same completeness condition.
   * @param exposures - Row-major factor exposure matrix, `n_assets x n_factors`: asset i's exposure to factor j is `exposures[i * n_factors + j]`.
   * @param nFactors - Number of factor columns in `exposures`; must be a positive integer no greater than `4294967295`.
   * @param returns - Realized asset returns, length `n_assets` (defines `n_assets`).
   * @param weights - Holding weights whose weighted return `w'r` must be fully reproduced by `w'Xf` (e.g. benchmark weights for a benchmark-return attribution).
   * @returns Constrained factor returns `f`, one per factor, satisfying `w'Xf = w'r` to numerical precision.
   * @throws Error - If `nFactors` is non-finite, fractional, zero, negative, or exceeds the WebAssembly `usize` range; if vector dimensions are inconsistent (including an overflowing `n_assets * n_factors`); if any vector value is non-finite; if the design matrix is rank-deficient; if coefficient rescaling or the constraint correction produces a non-finite value; or if the correction direction is degenerate and OLS does not already satisfy the constraint.
   */
  constrainedLeastSquares(
    exposures: NumericArray,
    nFactors: number,
    returns: NumericArray,
    weights: NumericArray
  ): Float64Array;
  /**
   * Sharpe ratio of one return series (annualized excess mean over
   * annualized sample volatility; the same kernel as `Performance.sharpe`).
   * @param returns - Per-period simple decimal returns in date order.
   * @param rf - Annualized risk-free rate as a decimal (`0.02` for 2%); defaults to `0`.
   * @param periodsPerYear - Observations per year used to annualize; defaults to `252`.
   * @returns The Sharpe ratio; `±Infinity` when volatility is zero with a non-zero excess return, `NaN` for fewer than two observations, non-finite inputs, or an invalid `periodsPerYear`.
   * @throws Error - Rejects a `returns` value that is not a numeric array.
   */
  sharpe(returns: NumericArray, rf?: number, periodsPerYear?: number): number;
  /**
   * Annualized Sortino ratio of one return series.
   * @param returns - Per-period simple decimal returns in date order.
   * @param mar - Minimum acceptable return per period as a decimal (not annualized); defaults to `0`.
   * @param periodsPerYear - Observations per year used to annualize; defaults to `252`.
   * @returns The Sortino ratio; `±Infinity` with no downside deviation but a non-zero excess mean, `NaN` for fewer than two observations, non-finite inputs, or an invalid `periodsPerYear`.
   * @throws Error - Rejects a `returns` value that is not a numeric array.
   */
  sortino(returns: NumericArray, mar?: number, periodsPerYear?: number): number;
  /**
   * Annualized sample volatility (n−1 denominator) of one return series.
   * @param returns - Per-period simple decimal returns in date order.
   * @param periodsPerYear - Observations per year; the per-period standard deviation is scaled by its square root. Defaults to `252`.
   * @returns Annualized volatility as a decimal; `0` for an empty array, `NaN` for non-finite inputs or an invalid `periodsPerYear`.
   * @throws Error - Rejects a `returns` value that is not a numeric array.
   */
  volatility(returns: NumericArray, periodsPerYear?: number): number;
  /**
   * Maximum peak-to-trough drawdown of one return series.
   * @param returns - Per-period simple decimal returns in date order; they are compounded into a wealth path before the running-peak decline is measured.
   * @returns Non-positive fraction (`-0.25` is a 25% loss); `0` when the series never falls below its running peak or is empty. Non-finite returns or reconstructed drawdowns yield `NaN`.
   * @throws Error - Rejects a `returns` value that is not a numeric array.
   */
  maxDrawdown(returns: NumericArray): number;
}

/**
 * Namespaced TypeScript entry point for analytics APIs.
 */
export declare const analytics: AnalyticsNamespace;

// --- models.factor.credit ----------------------------------------------------

/**
 * Calibrated credit factor hierarchy artifact.
 *
 * Produced by `CreditCalibrator` or deserialized from JSON via `fromJson`.
 * Immutable once constructed.
 */
export declare class CreditFactorModel {
  private constructor();
  /**
   * Deserialize and validate a `CreditFactorModel` from JSON.
   * @returns A calibrated `CreditFactorModel` handle.
   * @param s - JSON-serialized CreditFactorModel to deserialize.
   * @throws Error - Throws if the JSON is malformed or fails validation.
   */
  static fromJson(s: string): CreditFactorModel;
  /**
   * Exact namespaced credit-factor-model schema marker.
   * @returns The string `"finstack_quant.credit_factor_model/1"`.
   */
  readonly schema: string;
  /**
   * Serialize to compact canonical JSON.
   * @returns Canonical JSON string.
   * @throws Error - Throws a JavaScript exception if the model cannot be serialized to JSON.
   */
  toJson(): string;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Deterministic calibrator that produces a `CreditFactorModel`.
 *
 * Configuration and inputs are passed as JSON strings.
 */
export declare class CreditCalibrator {
  /**
   * Construct a calibrator from a JSON-serialized `CreditCalibrationConfig`.
   * @param configJson - Credit-factor calibration configuration JSON controlling model fitting.
   * @throws Error - Throws if `config_json` is not a valid `CreditCalibrationConfig`.
   */
  constructor(configJson: string);
  /**
   * Run the calibration pipeline and return a `CreditFactorModel`.
   * @returns A calibrated `CreditFactorModel` handle.
   * @param inputsJson - Credit-factor calibration input JSON containing issuers, spreads, and observations.
   * @throws Error - Throws if inputs are structurally invalid or calibration fails.
   */
  calibrate(inputsJson: string): CreditFactorModel;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
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
  /**
   * Deserialize a hierarchy-level snapshot from canonical JSON.
   * @param json - Canonical `LevelsAtDate` JSON containing `date`, `generic`, `by_level`, and `adder` fields.
   * @param json - Canonical `LevelsAtDate` JSON.
   * @returns A validated `LevelsAtDate` handle.
   * @throws Error - Throws when the JSON is malformed or a numeric field is non-finite.
   */
  static fromJson(json: string): LevelsAtDate;
  /**
   * Observation date as an ISO-8601 string.
   */
  readonly date: string;
  /**
   * Generic factor level in basis points.
   */
  readonly generic: number;
  /**
   * Number of hierarchy levels.
   */
  readonly nLevels: number;
  /**
   * Return bucket values for a zero-based hierarchy level.
   * @param levelIndex - Zero-based hierarchy level index, which must be less than `nLevels`.
   * @param levelIndex - Zero-based hierarchy level index.
   * @returns A bucket-name to factor-level mapping in basis points.
   * @throws Error - Throws when `level_index` is outside the available levels or the map cannot be converted to a JavaScript object.
   */
  levelValues(levelIndex: number): Record<string, number>;
  /**
   * Return per-issuer residual adders in basis points.
   * @returns An issuer-ID to residual-adder mapping.
   * @throws Error - Throws when the mapping cannot be converted to a JavaScript object.
   */
  adder(): Record<string, number>;
  /**
   * Serialize the snapshot to compact canonical JSON.
   * @returns Canonical JSON string.
   * @throws Error - Throws if any numeric output field is non-finite (NaN/Inf), naming the offending field instead of silently serializing `null`.
   */
  toJson(): string;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Component-wise difference between two `LevelsAtDate` snapshots.
 *
 * Produced by `decomposePeriod`.
 */
export declare class PeriodDecomposition {
  private constructor();
  /**
   * Deserialize a period decomposition from canonical JSON.
   * @param json - Canonical `PeriodDecomposition` JSON containing `from`, `to`, `d_generic`, `by_level`, and `d_adder` fields.
   * @param json - Canonical `PeriodDecomposition` JSON.
   * @returns A validated `PeriodDecomposition` handle.
   * @throws Error - Throws when the JSON is malformed or a numeric field is non-finite.
   */
  static fromJson(json: string): PeriodDecomposition;
  /**
   * Earlier snapshot date as an ISO-8601 string.
   */
  readonly fromDate: string;
  /**
   * Later snapshot date as an ISO-8601 string.
   */
  readonly toDate: string;
  /**
   * Change in the generic factor in basis points.
   */
  readonly dGeneric: number;
  /**
   * Number of hierarchy levels.
   */
  readonly nLevels: number;
  /**
   * Return bucket deltas for a zero-based hierarchy level.
   * @param levelIndex - Zero-based hierarchy level index, which must be less than `nLevels`.
   * @param levelIndex - Zero-based hierarchy level index.
   * @returns A bucket-name to factor-change mapping in basis points.
   * @throws Error - Throws when `level_index` is outside the available levels or the map cannot be converted to a JavaScript object.
   */
  levelDeltas(levelIndex: number): Record<string, number>;
  /**
   * Return per-issuer residual-adder deltas in basis points.
   * @returns An issuer-ID to residual-adder-change mapping.
   * @throws Error - Throws when the mapping cannot be converted to a JavaScript object.
   */
  dAdder(): Record<string, number>;
  /**
   * Serialize the decomposition to compact canonical JSON.
   * @returns Canonical JSON string.
   * @throws Error - Throws if any numeric output field is non-finite (NaN/Inf), naming the offending field instead of silently serializing `null`.
   */
  toJson(): string;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Validated factor covariance matrix in deterministic row-major order.
 */
export interface FactorCovarianceMatrix {
  /**
   * Factor identifiers in row and column order.
   */
  factor_ids: string[];
  /**
   * Matrix dimension, equal to `factor_ids.length`.
   */
  n: number;
  /**
   * Annualized covariance values in row-major order.
   */
  data: number[];
}

/**
 * Canonical broad factor classification.
 */
export type FactorTypeValue =
  | 'rates'
  | 'credit'
  | 'equity'
  | 'fx'
  | 'volatility'
  | 'commodity'
  | 'inflation'
  | { custom: string };

/**
 * Canonical risk-factor definition used by a factor-model configuration.
 */
export interface FactorDefinition {
  /**
   * Stable factor identifier.
   */
  id: string;
  /**
   * Canonical factor classification.
   */
  factor_type: FactorTypeValue;
  /**
   * Structured mapping from factor moves to market-data perturbations.
   */
  market_mapping: Record<string, unknown>;
  /**
   * Optional human-readable description.
   */
  description?: string;
}

/**
 * Risk measure used when aggregating factor exposures.
 */
export type FactorRiskMeasure =
  | 'variance'
  | 'volatility'
  | { var: { confidence: number } }
  | { expected_shortfall: { confidence: number } };

/**
 * Finite-difference bump magnitudes in each factor type's canonical units.
 */
export interface FactorBumpSizeConfig {
  /**
   * Rates bump in basis points.
   */
  rates_bp: number;
  /**
   * Credit bump in basis points.
   */
  credit_bp: number;
  /**
   * Equity and commodity spot bump in percent.
   */
  equity_pct: number;
  /**
   * FX spot bump in percent.
   */
  fx_pct: number;
  /**
   * Volatility bump in vol points.
   */
  vol_points: number;
  /**
   * Per-factor overrides in the factor type's canonical units.
   */
  overrides: Record<string, number>;
}

/**
 * Portfolio factor-model configuration assembled at a forecast horizon.
 */
export interface FactorModelConfig {
  /**
   * Ordered factor definitions spanning the model universe.
   */
  factors: FactorDefinition[];
  /**
   * Covariance matrix aligned to `factors`.
   */
  covariance: FactorCovarianceMatrix;
  /**
   * Declarative dependency-to-factor matching configuration.
   */
  matching: Record<string, unknown>;
  /**
   * Sensitivity extraction strategy.
   */
  pricing_mode: 'delta_based' | 'full_repricing';
  /**
   * Risk measure used for factor aggregation.
   */
  risk_measure: FactorRiskMeasure;
  /**
   * Optional finite-difference bump overrides.
   */
  bump_size?: FactorBumpSizeConfig;
  /**
   * Optional policy for unmatched dependencies.
   */
  unmatched_policy?: 'strict' | 'residual' | 'warn';
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
  /**
   * Wrap a `CreditFactorModel` for vol forecasting.
   * @param model - Calibrated CreditFactorModel used to produce the covariance forecast.
   */
  constructor(model: CreditFactorModel);
  /**
   * Build the factor covariance matrix at the requested horizon.
   * @param horizonJson - JSON-serialized forecast horizon defining the future covariance date or period.
   * @returns Structured covariance matrix with ordered factor axes and row-major data.
   * @throws Error - Throws if the horizon string is invalid or the model data is inconsistent.
   */
  covarianceAt(horizonJson: string): FactorCovarianceMatrix;
  /**
   * Idiosyncratic vol (std dev) for a specific issuer at the requested horizon.
   * @returns Issuer idiosyncratic volatility as a decimal standard deviation at `horizonJson`.
   * @param issuerId - Stable issuer identifier used to select the required domain object.
   * @param horizonJson - JSON-serialized forecast horizon defining the future covariance date or period.
   * @throws Error - Throws if the issuer is not present in the model's vol state or the calibrated variance is negative.
   */
  idiosyncraticVol(issuerId: string, horizonJson: string): number;
  /**
   * Build a portfolio-level `FactorModelConfig` at the given horizon and risk measure.
   * @param horizonJson - JSON-serialized forecast horizon defining the future covariance date or period.
   * @param riskMeasureJson - Risk-measure configuration JSON applied when constructing the horizon factor model.
   * @returns Structured factor-model configuration ready for portfolio risk workflows.
   * @throws Error - Throws if the horizon or risk measure is invalid, or the model builder rejects the assembled configuration.
   */
  factorModelAt(horizonJson: string, riskMeasureJson: string): FactorModelConfig;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Decompose observed issuer spreads at a point in time into per-level factor
 * values and per-issuer residual adders.
 *
 * @example
 * ```typescript
 * import init, {
 *   decomposeLevels,
 *   type CreditFactorModel,
 *   type LevelsAtDate
 * } from "finstack-quant-wasm";
 * await init();
 * function currentLevels(model: CreditFactorModel): LevelsAtDate {
 *   // Callers pass decimal spreads (0.012 = 120 bp). Returned levels are bp.
 *   return decomposeLevels(model, '{"ACME": 0.012}', 0.01, "2026-01-02");
 * }
 * ```
 * @returns Per-level factor values and residual adders at the requested date, in bp.
 * @param model - Calibrated credit factor hierarchy used for the peel.
 * @param observedSpreadsJson - JSON `{issuer_id: spread}` map in decimal (`0.012` = 120 bp). Values that look like bp (e.g. `100.0`) are rejected.
 * @param observedGeneric - Generic (PC) factor value at `as_of`, same decimal convention as the spreads.
 * @param asOf - ISO-8601 valuation date for the snapshot.
 * @param runtimeTagsJson - Optional JSON `{issuer_id: {dim_key: tag}}` for issuers not present in the model artifact.
 * @param model - Calibrated CreditFactorModel used for the peel.
 * @param observedSpreadsJson - JSON `{issuer_id: spread}` map in decimal (`0.012` = 120 bp). Returned levels are bp.
 * @param observedGeneric - Observed generic-market spread in decimal, aligned with the model factors.
 * @param asOf - ISO-8601 valuation date used to stamp the snapshot.
 * @param runtimeTagsJson - Optional runtime-tag JSON for issuers missing from the artifact.
 * @throws Error - Throws if an issuer has no model row and no `runtime_tags` entry, if `as_of` cannot be parsed, or if a spread is outside the decimal band.
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
 * import init, {
 *   decomposePeriod,
 *   type LevelsAtDate,
 *   type PeriodDecomposition
 * } from "finstack-quant-wasm";
 * await init();
 * function periodChange(
 *   fromLevels: LevelsAtDate,
 *   toLevels: LevelsAtDate
 * ): PeriodDecomposition {
 *   return decomposePeriod(fromLevels, toLevels);
 * }
 * ```
 * @returns Component-wise change between two hierarchy-level snapshots.
 * @param fromLevels - Credit-factor levels at the start of the attribution period.
 * @param toLevels - Credit-factor levels at the end of the attribution period.
 * @throws Error - Throws if `from_levels.date > to_levels.date` or the snapshots disagree on hierarchy depth.
 */
export declare function decomposePeriod(
  fromLevels: LevelsAtDate,
  toLevels: LevelsAtDate
): PeriodDecomposition;

/**
 * Namespaced TypeScript entry points for factor model credit calculations and types.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const config = JSON.stringify({
 *   policy: "globally_off",
 *   hierarchy: { levels: [] },
 *   min_bucket_size_per_level: { per_level: [] },
 *   vol_model: "sample",
 *   covariance_strategy: "diagonal",
 *   beta_shrinkage: "none",
 *   use_returns_or_levels: "returns",
 *   panel_frequency: "monthly",
 *   bucket_weighting: "equal"
 * });
 * const calibrator = new models.factor.credit.CreditCalibrator(config);
 * calibrator.free();
 * ```
 */
export interface FactorModelCreditNamespace {
  /**
   * Calibrated credit-factor hierarchy artifact.
   */
  CreditFactorModel: typeof CreditFactorModel;
  /**
   * Deterministic calibrator that produces a `CreditFactorModel`.
   */
  CreditCalibrator: typeof CreditCalibrator;
  /**
   * Hierarchy-level factor snapshot at a single date.
   */
  LevelsAtDate: typeof LevelsAtDate;
  /**
   * Component-wise difference between two level snapshots.
   */
  PeriodDecomposition: typeof PeriodDecomposition;
  /**
   * Horizon covariance view over a calibrated credit factor model.
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
   * @returns Per-level factor values and residual adders at the requested date, in bp.
   * @param model - Calibrated credit factor hierarchy used for the peel.
   * @param observedSpreadsJson - JSON `{issuer_id: spread}` map in decimal (`0.012` = 120 bp). Values that look like bp (e.g. `100.0`) are rejected.
   * @param observedGeneric - Generic (PC) factor value at `as_of`, same decimal convention as the spreads.
   * @param asOf - ISO-8601 valuation date for the snapshot.
   * @param runtimeTagsJson - Optional JSON `{issuer_id: {dim_key: tag}}` for issuers not present in the model artifact.
   * @param model - Calibrated CreditFactorModel used for the peel.
   * @param observedSpreadsJson - JSON `{issuer_id: spread}` map in decimal (`0.012` = 120 bp). Returned levels are bp.
   * @param observedGeneric - Observed generic-market spread in decimal, aligned with the model factors.
   * @param asOf - ISO-8601 valuation date used to stamp the snapshot.
   * @param runtimeTagsJson - Optional runtime-tag JSON for issuers missing from the artifact.
   * @throws Error - Throws if an issuer has no model row and no `runtime_tags` entry, if `as_of` cannot be parsed, or if a spread is outside the decimal band.
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
   * @returns Component-wise change between two hierarchy-level snapshots.
   * @param fromLevels - Credit-factor levels at the start of the attribution period.
   * @param toLevels - Credit-factor levels at the end of the attribution period.
   * @throws Error - Throws if `from_levels.date > to_levels.date` or the snapshots disagree on hierarchy depth.
   */
  decomposePeriod(fromLevels: LevelsAtDate, toLevels: LevelsAtDate): PeriodDecomposition;
}

/**
 * Product-independent factor and position risk decomposition kernels.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const result = models.factor.risk.parametricVarDecomposition(
 *   JSON.stringify(["A", "B"]),
 *   JSON.stringify([0.6, 0.4]),
 *   JSON.stringify([[0.04, 0.01], [0.01, 0.09]]),
 *   0.95
 * );
 * console.log(result.portfolio_var);
 * ```
 */
export interface FactorRiskNamespace {
  /**
   * Decompose portfolio VaR into position contributions via parametric Euler
   * allocation. Inputs mirror the Python binding's signature.
   *
   * `covariance_json` must deserialize to an `n x n` row-major nested array.
   * @returns Returns a structured `VarDecompositionResult` object.
   * @param positionIdsJson - JSON array of position identifiers.
   * @param weightsJson - Position or asset weight-vector JSON.
   * @param covarianceJson - Covariance-matrix JSON.
   * @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
   * @param computeIncremental - Optional; when `true`, also computes incremental VaR (one full repricing per position). Defaults to `false`, mirroring the Python `compute_incremental=` keyword.
   * @throws Error - Throws a JavaScript exception if any JSON input is malformed; identifier, weight, or covariance dimensions disagree; the covariance matrix is not finite, symmetric, and positive semidefinite; `confidence` is not finite and in `(0.5, 1)`; or the result cannot be converted to a JavaScript value.
   */
  parametricVarDecomposition(
    positionIdsJson: string,
    weightsJson: string,
    covarianceJson: string,
    confidence: number,
    computeIncremental?: boolean
  ): VarDecompositionResult;
  /**
   * Decompose portfolio Expected Shortfall into position contributions via
   * parametric Euler allocation.
   * @returns Returns a structured `EsDecompositionResult` object.
   * @param positionIdsJson - JSON array of position identifiers.
   * @param weightsJson - Position or asset weight-vector JSON.
   * @param covarianceJson - Covariance-matrix JSON.
   * @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
   * @throws Error - Throws a JavaScript exception if any JSON input is malformed; identifier, weight, or covariance dimensions disagree; the covariance matrix is not finite, symmetric, and positive semidefinite; `confidence` is not finite and in `(0.5, 1)`; or the result cannot be converted to a JavaScript value.
   */
  parametricEsDecomposition(
    positionIdsJson: string,
    weightsJson: string,
    covarianceJson: string,
    confidence: number
  ): EsDecompositionResult;
  /**
   * Decompose portfolio VaR and Expected Shortfall from per-position scenario
   * profit-and-loss series using historical simulation.
   * @returns Returns a structured `VarDecompositionResult` object.
   * @param positionIdsJson - JSON array of position identifiers.
   * @param positionPnlsJson - Per-position P&L JSON.
   * @param confidence - Tail confidence as a decimal probability, such as 0.95 for 95%.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, position or scenario dimensions disagree, `confidence` is not finite and in `(0.5, 1)`, too few scenarios resolve the requested tail, a P-and-L value is non-finite, or the result cannot be converted to a JavaScript value.
   */
  historicalVarDecomposition(
    positionIdsJson: string,
    positionPnlsJson: string,
    confidence: number
  ): VarDecompositionResult;
  /**
   * Evaluate per-position component VaRs against target risk-budget shares.
   * @returns Returns a structured `RiskBudgetResult` object.
   * @param positionIdsJson - JSON array of position identifiers.
   * @param actualVarJson - Actual component-VaR JSON.
   * @param targetVarPctJson - Target VaR-share JSON.
   * @param portfolioVar - Total portfolio VaR used to convert risk-budget shares into absolute amounts.
   * @param utilizationThreshold - Optional actual-to-target risk ratio that flags a budget breach; omit for the Rust default of 1.2.
   * @throws Error - Throws a JavaScript exception if any JSON input is malformed, actual or target arrays do not match the identifier count, a position id is duplicated, non-empty target shares do not sum to one within tolerance, nonzero component risk is paired with zero `portfolioVar`, or the result cannot be converted to a JavaScript value.
   */
  evaluateRiskBudget(
    positionIdsJson: string,
    actualVarJson: string,
    targetVarPctJson: string,
    portfolioVar: number,
    utilizationThreshold?: number
  ): RiskBudgetResult;
}

/**
 * Namespaced TypeScript entry points for factor model calculations and types.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const credit = models.factor.credit;
 * const config = JSON.stringify({
 *   policy: "globally_off",
 *   hierarchy: { levels: [] },
 *   min_bucket_size_per_level: { per_level: [] },
 *   vol_model: "sample",
 *   covariance_strategy: "diagonal",
 *   beta_shrinkage: "none",
 *   use_returns_or_levels: "returns",
 *   panel_frequency: "monthly",
 *   bucket_weighting: "equal"
 * });
 * const calibrator = new credit.CreditCalibrator(config);
 * calibrator.free();
 * ```
 */
export interface FactorNamespace {
  /**
   * Credit factor hierarchy artifacts, calibration, and decomposition.
   */
  credit: FactorModelCreditNamespace;
  /**
   * Pure factor and position risk decomposition kernels.
   */
  risk: FactorRiskNamespace;
}

// --- features ---------------------------------------------------------------

/**
 * Feature observation: a finite number, or `null` for a missing value.
 */
export type FeatureValue = number | null;
/**
 * Operation-specific parameter object passed to feature transforms.
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
 * const changes = features.transformTimeseries(
 *   [1, 3, 6],
 *   ["ACME", "ACME", "ACME"],
 *   ["2026-01-01", "2026-01-02", "2026-01-03"],
 *   "diff"
 * );
 * console.log(changes);
 * ```
 */
export interface FeaturesNamespace {
  /**
   * Transform a time-series panel column per entity.
   * Rolling windows count rows including gaps; min_periods counts finite inputs.
   * Aggregates may emit at missing rows. EWMA span must be at least 1; mature
   * constant series have zero volatility. String keys need consistent UTC and precision.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param entity - Entity identifier used to group ordered time-series observations.
   * @param order - Observation-order key used to sort each entity time series.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object. `rolling_sharpe` accepts optional `risk_free` (default `0.0`, same units as the return series).
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, an unsupported `op`, malformed operation parameters, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
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
   * cap_weights constrains final absolute weights with zero net and unit gross
   * exposure, preserving centered-signal signs; infeasible caps fail.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal `values` and `time_key` lengths, an unsupported `op`, malformed operation parameters, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
   */
  transformCrossSectional(
    values: FeatureValue[],
    timeKey: string[],
    op: string,
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Transform a cross-section within each time/group sub-partition.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param groups - Group labels aligned with values for within-group cross-sectional operations.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal `values`, `time_key`, and `groups` lengths, an unsupported `op`, malformed operation parameters, or a result that cannot be serialized to JavaScript.
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
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param exposures - Factor-exposure matrix aligned with the supplied observations.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, exposure columns whose lengths differ from `values`, a non-boolean `fit_intercept`, a singular or underdetermined cross-section, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
   */
  neutralize(
    values: FeatureValue[],
    timeKey: string[],
    exposures: FeatureValue[][],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Transform two time-series panel columns per entity.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param other - Second value series aligned with the primary series for a pairwise transformation.
   * @param entity - Entity identifier used to group ordered time-series observations.
   * @param order - Lexicographic observation-order key; use ISO-8601 for calendar chronology.
   * @param op - Transformation operation identifier supported by the feature-engineering API.
   * @param params - Operation-specific parameter object. `window` counts rows including gaps; `min_periods <= window` counts complete pairs.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, an unsupported `op`, non-positive or non-integer `window` or `min_periods` parameters, or a result that cannot be serialized to JavaScript.
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
   * window counts rows including gaps; min_periods counts complete rows.
   * Missing current responses or exposures and rank-deficient windows yield null.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param exposures - Factor-exposure matrix aligned with the supplied observations.
   * @param entity - Entity identifier used to group ordered time-series observations.
   * @param order - Observation-order key used to sort each entity time series.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, exposure columns whose lengths differ from `values`, malformed `window`, `min_periods`, or `fit_intercept` parameters, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
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
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric signal observations aligned with `timeKey` and `volatility`.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param volatility - Row-aligned nonnegative risk estimates in a common horizon and units, used as `signal / volatility`; negative estimates fail, while zero, missing, or non-finite values yield missing weights.
   * @throws Error - Rejects inputs that cannot be decoded into the declared arrays, unequal `values`, `time_key`, and `volatility` lengths, negative volatility, or a result that cannot be serialized to JavaScript.
   */
  riskScaledWeights(
    values: FeatureValue[],
    timeKey: string[],
    volatility: FeatureValue[]
  ): FeatureValue[];
  /**
   * Convert ranks into long/short weights.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @throws Error - Rejects inputs that cannot be decoded into the declared arrays, unequal `values` and `time_key` lengths, non-finite arithmetic, or a result that cannot be serialized to JavaScript.
   */
  rankToWeights(values: FeatureValue[], timeKey: string[]): FeatureValue[];
  /**
   * Neutralize a signal and z-score residuals.
   * fit_intercept must be true (the default) to preserve exposure neutrality.
   * @returns Transformed values aligned one-for-one with the input `values` rows.
   * @param values - Numeric observations in the shape and order required by the selected transformation.
   * @param timeKey - Cross-sectional time key shared by values evaluated in the same slice.
   * @param exposures - Factor-exposure matrix aligned with the supplied observations.
   * @param params - Operation-specific parameter object defining transformation settings.
   * @throws Error - Rejects values that cannot be decoded into the declared arrays or JSON parameters, unequal row counts, exposure columns whose lengths differ from `values`, a false or non-boolean `fit_intercept`, or a result that cannot be serialized to JavaScript.
   */
  neutralizeAndZscore(
    values: FeatureValue[],
    timeKey: string[],
    exposures: FeatureValue[][],
    params?: FeatureParams | null
  ): FeatureValue[];
  /**
   * Apply a JSON panel transform pipeline.
   * @returns JSON panel after applying the transform pipeline.
   * @param specJson - Canonical panel-transformation JSON. Each operation may set optional `input` (`undefined` default: previous column, or raw `values` for the first op).
   * @throws Error - Rejects malformed JSON or panel specifications, blank, reserved (`values`), or duplicate operation names, unknown `input` columns, missing partition columns, unequal row counts, malformed operation parameters, operations that cannot be evaluated, non-finite arithmetic, or a result that cannot be serialized to JSON.
   */
  transformPanelJson(specJson: string): string;
}

/**
 * Namespaced TypeScript entry point for features APIs.
 */
export declare const features: FeaturesNamespace;

// --- models.correlation -------------------------------------------------

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
   * @returns Conditional default probability in `[0, 1]`.
   * @param defaultThreshold - Latent-variable default threshold corresponding to the marginal default probability.
   * @param factorRealization - Realized systematic-factor value conditioning the default probability.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @throws Error - Throws a JavaScript exception if the factor count does not match the copula, any input is non-finite, `correlation` is outside `[0, 1]`, or the model produces a probability outside `[0, 1]`.
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
   * @returns Lower-tail dependence `λ_L` in `[0, 1]`, or `NaN` when the copula has no closed form.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
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
   * @returns Heuristic extra correlation mass in the high-loading tail; 0 in the Gaussian limit.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @throws Error - Throws a JavaScript exception if this copula is not a Random Factor Loading model.
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
   * @returns A concrete `Copula` handle.
   * @throws Error - Throws a JavaScript exception if a Student-t specification contains non-finite degrees of freedom or a value at most two.
   */
  build(): Copula;
}

/**
 * Copula model specification for configuration and deferred construction.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const copula = models.correlation.CopulaSpec.gaussian().build();
 * console.log(copula.modelName, copula.numFactors);
 * ```
 */
export interface CopulaSpecConstructor {
  /**
   * One-factor Gaussian copula (market standard).
   * @returns A `CopulaSpec` handle for deferred construction.
   */
  gaussian(): CopulaSpec;
  /**
   * Student-t copula with specified degrees of freedom (must be > 2).
   * @returns A `CopulaSpec` handle for deferred construction.
   * @param degreesOfFreedom - Student-t degrees of freedom controlling tail thickness; finite and strictly greater than two.
   * @throws Error - Throws a JavaScript exception if `degreesOfFreedom` is not finite and strictly greater than two.
   */
  studentT(degreesOfFreedom: number): CopulaSpec;
  /**
   * Random Factor Loading copula with stochastic correlation.
   * @returns A `CopulaSpec` handle for deferred construction.
   * @param loadingVol - Standard deviation used to randomize the factor loading.
   */
  randomFactorLoading(loadingVol: number): CopulaSpec;
  /**
   * Two-factor Gaussian copula with global and shared-sector factors.
   * @returns A `CopulaSpec` handle for deferred construction.
   */
  multiFactor(): CopulaSpec;
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
   * @returns Conditional recovery rate as a fraction of par in `[0, 1]`.
   * @param marketFactor - Realized standardized market factor used to condition recovery or loss given default.
   */
  conditionalRecovery(marketFactor: number): number;
  /**
   * Conditional LGD given market factor.
   * @returns Conditional loss-given-default as a fraction of par in `[0, 1]`.
   * @param marketFactor - Realized standardized market factor used to condition recovery or loss given default.
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
   * @returns A concrete `RecoveryModel` handle.
   */
  build(): RecoveryModel;
}

/**
 * Recovery model specification for configuration and deferred construction.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const recovery = models.correlation.RecoverySpec.constant(0.4).build();
 * console.log(recovery.expectedRecovery, recovery.lgd);
 * ```
 */
export interface RecoverySpecConstructor {
  /**
   * Constant recovery rate.
   *
   * Throws if `rate` is not finite or lies outside `[0, 1]`.
   * @returns A `RecoverySpec` handle for deferred construction.
   * @param rate - Constant recovery rate expressed as a fraction from 0 through 1.
   * @throws Error - Throws a JavaScript exception if `rate` is not finite or lies outside `[0, 1]`.
   */
  constant(rate: number): RecoverySpec;
  /**
   * Market-correlated (Andersen-Sidenius) stochastic recovery.
   *
   * Throws if `mean` is not finite or lies outside `[0, 1]`, or if `vol` /
   * `correlation` are not finite.
   * @returns A `RecoverySpec` handle for deferred construction.
   * @param mean - Mean recovery rate expressed as a fraction from 0 through 1.
   * @param vol - Recovery-rate volatility scale in the correlated recovery model.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @throws Error - Throws a JavaScript exception if `mean` is not finite or lies outside `[0, 1]`, or if `vol` or `correlation` is non-finite. Finite volatility and correlation inputs are clamped to their supported ranges.
   */
  marketCorrelated(mean: number, vol: number, correlation: number): RecoverySpec;
  /**
   * Market-standard stochastic recovery (40% mean, 25% vol, +40% corr —
   * recovery falls in stress under the canonical low-factor-stress
   * convention).
   * @returns A `RecoverySpec` handle for deferred construction.
   */
  marketStandardStochastic(): RecoverySpec;
}

/**
 * Exported class; construct instances via `CopulaSpec.build()` (no public `new`).
 */
export interface CopulaClass {
  /**
   * JavaScript prototype of `Copula`; construct instances via `CopulaSpec.build()`.
   */
  readonly prototype: Copula;
}

/**
 * Exported class; construct instances via `RecoverySpec.build()` (no public `new`).
 */
export interface RecoveryModelClass {
  /**
   * JavaScript prototype of `RecoveryModel`; construct instances via `RecoverySpec.build()`.
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
  /**
   * Tranche attachment point as a fraction of pool notional, in `[0, 1)`.
   */
  attachment: number;
  /**
   * Tranche detachment point as a fraction of pool notional, in `(0, 1]`.
   */
  detachment: number;
  /**
   * Tranche notional `(detachment - attachment) * poolNotional`.
   */
  tranche_notional: number;
  /**
   * Mean tranche loss as a fraction of tranche notional, in `[0, 1]`.
   */
  expected_loss_fraction: number;
  /**
   * Mean tranche loss in pool-notional units.
   */
  expected_loss_amount: number;
  /**
   * Nearest-rank tranche loss fraction at the distribution's confidence.
   */
  var_fraction: number;
  /**
   * Nearest-rank tranche loss amount at the distribution's confidence.
   */
  var_amount: number;
  /**
   * Probability-weighted mean tranche loss fraction in the worst confidence tail.
   */
  expected_shortfall_fraction: number;
  /**
   * Probability-weighted mean tranche loss amount in the worst confidence tail.
   */
  expected_shortfall_amount: number;
  /**
   * Share of paths whose pool loss fraction strictly exceeds `attachment`.
   */
  prob_attachment_breached: number;
  /**
   * Share of paths whose pool loss fraction reaches or exceeds `detachment`.
   */
  prob_full_writedown: number;
}

/**
 * Namespaced TypeScript entry points for correlation calculations and types.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const [lower, upper] = models.correlation.correlationBounds(0.1, 0.2);
 * console.log(lower, upper);
 * ```
 */
export interface CorrelationNamespace {
  /**
   * Copula specification constructor for correlation-sensitive pricing.
   */
  CopulaSpec: CopulaSpecConstructor;
  /**
   * Fitted copula handle used by correlation-sensitive pricing.
   */
  Copula: CopulaClass;
  /**
   * Recovery-model specification constructor.
   */
  RecoverySpec: RecoverySpecConstructor;
  /**
   * Recovery-model handle used with copula pricing.
   */
  RecoveryModel: RecoveryModelClass;
  /**
   * Fréchet-Hoeffding correlation bounds for two Bernoulli marginals.
   *
   * Returns `[rho_min, rho_max]`.
   * @returns `[rho_min, rho_max]` Fréchet-Hoeffding bounds for the two Bernoulli marginals.
   * @param p1 - First marginal default probability from 0 through 1.
   * @param p2 - Second marginal default probability from 0 through 1.
   * @throws Error - Throws a JavaScript exception if either marginal probability is non-finite or outside `[0, 1]`.
   */
  correlationBounds(p1: number, p2: number): Float64Array;
  /**
   * Joint probabilities for two correlated Bernoulli variables.
   *
   * Returns `[p11, p10, p01, p00]`.
   * @returns Joint probabilities `[p11, p10, p01, p00]` for the two correlated Bernoullis.
   * @param p1 - First marginal default probability from 0 through 1.
   * @param p2 - Second marginal default probability from 0 through 1.
   * @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
   * @throws Error - Throws a JavaScript exception if either marginal probability is non-finite or outside `[0, 1]`, or `correlation` is non-finite or outside `[-1, 1]`.
   */
  jointProbabilities(p1: number, p2: number, correlation: number): Float64Array;
  /**
   * Validate a flat row-major correlation matrix.
   *
   * Accepts a `Float64Array`/`number[]` of `n * n` row-major entries and
   * checks unit diagonal, off-diagonal in `[-1, 1]`, symmetry, and positive
   * semi-definiteness. Returns nothing on success; raises a descriptive error
   * (including the failing dimension or constraint) otherwise.
   * @param matrix - Flat row-major `n * n` correlation coefficients; unit diagonal, off-diagonals in `[-1, 1]`.
   * @param n - Positive square-matrix dimension; `matrix` must contain exactly `n * n` entries.
   * @throws Error - Throws a JavaScript exception if the flat length is not `n * n`, a diagonal entry is not one, an entry is outside the correlation bounds, the matrix is not symmetric, or the matrix is not positive semidefinite.
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
   * @returns Nearest valid correlation matrix as a flat row-major `Float64Array` of `n * n` entries.
   * @param matrix - Flat row-major `n * n` near-correlation matrix to project onto the correlation set.
   * @param n - Positive square-matrix dimension; `matrix` must contain exactly `n * n` entries.
   * @param maxIter - Maximum number of Higham nearest-correlation projection iterations.
   * @param tol - Positive convergence tolerance for the nearest-correlation projection.
   * @throws Error - Throws a JavaScript exception if the flat length is not `n * n`, the input has a gross diagonal or symmetry violation, or the projection does not converge within `maxIter` iterations at `tol`.
   */
  nearestCorrelation(matrix: NumericArray, n: number, maxIter?: number, tol?: number): Float64Array;
  /**
   * Tranche loss statistics over a simulated pool loss distribution.
   *
   * `attachment` and `detachment` are fractions of pool notional in `[0, 1]` —
   * a 0-3% equity tranche is `(0.0, 0.03)`, not `(0.0, 3.0)`. Each path's pool
   * loss fraction `L = loss / poolNotional` maps through
   * `clamp(L - attachment, 0, width) / width`, and the resulting distribution
   * uses loss-positive nearest-rank VaR and ES over exactly the worst
   * `1 - confidence` probability mass, with fractional boundary weight.
   * @returns Returns the tranche notional, expected loss, VaR, expected shortfall, and breach probabilities.
   * @param losses - Loss-positive path losses in one caller-defined unit, one entry per simulated path.
   * @param confidence - Loss-positive VaR and expected-shortfall confidence strictly between 0 and 1.
   * @param attachment - Lower tranche boundary as a fraction of pool notional from 0 through 1.
   * @param detachment - Upper tranche boundary as a fraction of pool notional, strictly above the attachment and at most 1.
   * @param poolNotional - Total pool notional, finite and strictly positive, in the same unit as the losses.
   * @throws Error - Throws a JavaScript exception if the loss distribution is empty or contains a non-finite or negative loss; `confidence` is outside `(0, 1)`; tranche boundaries are invalid; `poolNotional` is not finite and positive; a derived statistic is non-finite; allocation fails; or conversion to JavaScript fails.
   */
  trancheLossStatistics(
    losses: NumericArray,
    confidence: number,
    attachment: number,
    detachment: number,
    poolNotional: number
  ): TrancheLossStatisticsJson;
}

// --- models.monteCarlo ----------------------------------------------------------
// Host-neutral subset shared with Python: Heston Monte Carlo pricing. The
// closed-form Black-Scholes references live at `models.bsPrice`.

/**
 * Namespaced TypeScript entry points for Monte Carlo calculations.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const est = models.monteCarlo.priceHestonCall(
 *   100, 100, 0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04, 1.0, 5000, 42n,
 * );
 * console.log(est.mean);
 * ```
 */
export interface MonteCarloNamespace {
  /**
   * Price a European call under Heston stochastic volatility.
   * @returns Discounted Monte Carlo estimate with standard error and optional path statistics.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
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
   * @throws Error - Throws a JavaScript exception if `currency` is unknown; embedded defaults cannot be loaded when `num_steps` is omitted; `rate` or `div_yield` is non-finite; `kappa`, `theta`, `vol_of_vol`, or `v0` is non-finite or non-positive; `rho` is outside `[-1, 1]`; the expiry, step count, path count, or computed discount factor fails validation; a simulated discounted payoff is non-finite; or the result cannot be serialized.
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
   * @returns Discounted Monte Carlo estimate with standard error and optional path statistics.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
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
   * @throws Error - Throws a JavaScript exception if `currency` is unknown; embedded defaults cannot be loaded when `num_steps` is omitted; `rate` or `div_yield` is non-finite; `kappa`, `theta`, `vol_of_vol`, or `v0` is non-finite or non-positive; `rho` is outside `[-1, 1]`; the expiry, step count, path count, or computed discount factor fails validation; a simulated discounted payoff is non-finite; or the result cannot be serialized.
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
}

/**
 * Namespaced TypeScript entry point for monte carlo APIs.
 */

// --- margin ----------------------------------------------------------------

/**
 * Namespaced TypeScript entry points for margin calculations and types.
 * @example
 * ```typescript
 * import init, { margin } from "finstack-quant-wasm";
 * await init();
 * const csa = margin.csaUsdRegulatoryJson();
 * const vm = margin.calculateVm(csa, 1_000_000, 0, "USD", "2026-01-02");
 * console.log(vm.collect_amount);
 * ```
 */
export interface MarginNamespace {
  /**
   * Create a standard USD regulatory CSA specification as JSON.
   *
   * Returns the canonical ISDA-compliant CSA for USD OTC derivatives.
   * @returns Canonical ISDA USD regulatory CSA JSON.
   * @throws Error - Rejects if the embedded margin registry cannot be loaded or the resulting CSA cannot be serialized to JSON.
   */
  csaUsdRegulatoryJson(): string;
  /**
   * Create a standard EUR regulatory CSA specification as JSON.
   * @returns Canonical ISDA EUR regulatory CSA JSON.
   * @throws Error - Rejects if the embedded margin registry cannot be loaded or the resulting CSA cannot be serialized to JSON.
   */
  csaEurRegulatoryJson(): string;
  /**
   * Validate a CSA specification JSON string.
   *
   * Validates the JSON schema and CSA semantics, including amount currencies,
   * monetary bounds and calendar lookup. Returns canonical JSON on success.
   * @returns Canonical CSA JSON after schema validation.
   * @param json - CSA specification JSON to validate and normalize into canonical form.
   * @throws Error - Rejects malformed or schema-incompatible `json`, or failure to serialize the decoded CSA specification; also rejects invalid CSA terms or calendar identifiers.
   */
  validateCsaJson(json: string): string;
  /**
   * Calculate variation margin given exposure, posted collateral, and CSA JSON.
   *
   * Returns a JSON object with post_amount, collect_amount, net_exposure,
   * and requires_call fields.
   *
   * @param csaJson - CSA specification JSON governing thresholds, minimum transfer, and timing.
   * @param exposure - Signed mark-to-market in the supplied currency: positive means the counterparty owes the desk.
   * @param postedCollateral - Signed collateral balance: positive held, negative posted, including pending agreed calls.
   * @param currency - ISO-4217 currency code shared by exposure and collateral amounts.
   * @param asOf - ISO-8601 VM calculation date.
   * @returns Variation-margin call amount, currency, and CSA metadata as a plain object.
   * @throws Error - Rejects malformed or schema-incompatible `csa_json`, an unknown `currency`, non-finite exposure or collateral amounts, an invalid calendar date, a currency mismatch with the CSA, invalid VM parameters, calendar lookup or settlement-date adjustment failures, or failure to serialize the result.
   */
  calculateVm(
    csaJson: string,
    exposure: number,
    postedCollateral: number,
    currency: string,
    asOf: string
  ): VariationMarginJson;
  /**
   * Compute bilateral XVA: CVA, DVA, FVA, MVA, and the all-in adjustment.
   *
   * All legs are weighted by joint (first-to-default) survival. MVA is computed
   * only when `fundingJson` carries an `im_profile`; that posted IM also reduces
   * ENE for bilateral DVA.
   *
   * The returned object reports the required all-in amount as
   * `total_xva = CVA - DVA + FVA + MVA`. Optional funding legs are absent from
   * the payload when they were not computed.
   *
   * @example
   * ```javascript
   * import init, { core, margin } from "finstack-quant-wasm";
   * await init();
   * const df = new core.DiscountCurve("USD-OIS", "2025-01-01", [0.0, 1.0, 5.0, 1.0], "log_linear");
   * const hz = new core.HazardCurve("CPTY", "2025-01-01", [0.0, 0.02, 30.0, 0.02], 0.4);
   * const result = margin.computeBilateralXva(
   *   JSON.stringify({ times: [1, 2], mtm_values: [1e6, 1e6], epe: [1e6, 1e6], ene: [0, 0] }),
   *   hz, hz, df, 0.4, 0.4,
   *   JSON.stringify({ funding_spread_bp: 50.0 }),
   * );
   * result.total_xva; // CVA - DVA + FVA + MVA
   * ```
   * @param exposureProfileJson - `ExposureProfile` JSON with `times`, `mtm_values`, `epe`, and `ene` arrays of equal length.
   * @param counterpartyHazardCurve - Hazard curve for the counterparty's credit.
   * @param ownHazardCurve - Hazard curve for the institution's own credit.
   * @param discountCurve - Risk-free discount curve for present-valuing.
   * @param counterpartyRecoveryRate - Recovery on counterparty default, in `[0, 1]`.
   * @param ownRecoveryRate - Recovery on own default, in `[0, 1]`.
   * @param fundingJson - Optional strict `FundingConfig` JSON driving FVA and, when it carries `im_profile`, MVA; unknown fields are rejected. Omit for credit legs only.
   * @returns The `XvaResult` as a plain object.
   * @throws Error - If JSON is malformed or has unknown funding fields, a recovery rate is outside `[0, 1]`, a profile is invalid or has a mismatched IM horizon, or a curve evaluation is non-finite.
   */
  computeBilateralXva(
    exposureProfileJson: string,
    counterpartyHazardCurve: HazardCurve,
    ownHazardCurve: HazardCurve,
    discountCurve: DiscountCurve,
    counterpartyRecoveryRate: number,
    ownRecoveryRate: number,
    fundingJson?: string | null
  ): XvaResultJson;
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
 * const spec = JSON.stringify({
 *   notional: { initial: { amount: "1000000", currency: "USD" }, amort: "none" },
 *   issue: "2026-01-02",
 *   maturity: "2027-01-02",
 *   coupon_program: [{
 *     kind: "fixed",
 *     spec: {
 *       coupon_type: "cash",
 *       rate: "0.05",
 *       frequency: { count: 12, unit: "months" },
 *       day_count: "30_360",
 *       business_day_convention: "following",
 *       calendar_id: "weekends_only",
 *       stub: "none",
 *       end_of_month: false,
 *       payment_lag_days: 0
 *     }
 *   }]
 * });
 * const schedule = cashflows.buildCashflowScheduleJson(spec);
 * console.log(JSON.parse(cashflows.datedFlowsJson(schedule)).length);
 * ```
 */
export interface CashflowsNamespace {
  /**
   * Build a cashflow schedule from a `CashflowScheduleBuildSpec` JSON string.
   *
   * @param specJson - JSON-encoded `CashflowScheduleBuildSpec`. Optional `principal_exchange` is `"none"` or `"initial_and_final"` (default). `principal_events` entries require both economic `date` and cash `payment_date`.
   * @param marketJson - Optional JSON-encoded market context for floating-rate lookups.
   * @returns JSON-encoded `CashFlowSchedule`.
   * @throws If the spec or market JSON is malformed, or schedule construction fails.
   */
  buildCashflowScheduleJson(specJson: string, marketJson?: string | null): string;

  /**
   * Validate a cashflow schedule JSON string and return it canonicalized.
   *
   * @param scheduleJson - JSON-encoded `CashFlowSchedule`.
   * @returns Canonicalized JSON-encoded `CashFlowSchedule`.
   * @throws If the schedule JSON is malformed or fails validation.
   */
  validateCashflowScheduleJson(scheduleJson: string): string;

  /**
   * Extract dated flows from a cashflow schedule JSON string.
   *
   * @param scheduleJson - JSON-encoded `CashFlowSchedule`.
   * @returns JSON array of settlement cash entries. PIK and `DefaultedNotional` state rows are omitted; parse the full schedule JSON when flow classification is required.
   * @throws If the schedule JSON is malformed.
   */
  datedFlowsJson(scheduleJson: string): string;

  /**
   * Compute accrued interest from a cashflow schedule JSON string as of a given date.
   *
   * @param scheduleJson - JSON-encoded `CashFlowSchedule`.
   * @param asOf - ISO-8601 date (YYYY-MM-DD) for the accrual snapshot.
   * @param configJson - Optional JSON-encoded `AccrualConfig` overriding defaults.
   * @returns Accrued interest in the schedule's settlement currency as a JS number. The Rust engine computes from the canonical schedule and then crosses the WASM boundary as `f64`; for large notionals, compare with an absolute tolerance scaled to the schedule notional rather than expecting decimal-string equality.
   * @throws If any JSON input is malformed or the accrual computation fails.
   */
  accruedInterest(scheduleJson: string, asOf: string, configJson?: string | null): number;

  /**
   * Convert an annual CPR (constant prepayment rate) to a monthly SMM.
   *
   * Uses the standard relationship `SMM = 1 - (1 - CPR)^(1/12)`.
   *
   * @param cpr - Annualized CPR as a decimal in `[0, 1]` (0.06 means 6%).
   * @returns Monthly SMM as a decimal.
   * @throws If `cpr` is negative, non-finite, or above 1.0.
   */
  cprToSmm(cpr: number): number;

  /**
   * Convert a monthly SMM (single monthly mortality) to an annual CPR.
   *
   * Uses `CPR = 1 - (1 - SMM)^12`.
   *
   * @param smm - Monthly SMM as a decimal in `[0, 1]`.
   * @returns Annualized CPR as a decimal.
   * @throws If `smm` is negative, non-finite, or above 1.0.
   */
  smmToCpr(smm: number): number;

  /**
   * Convert an annual CDR (constant default rate) to a monthly MDR.
   *
   * Default and prepayment mortality rates share the same annual-to-monthly
   * conversion kernel: `MDR = 1 - (1 - CDR)^(1/12)`.
   *
   * @param cdr - Constant annual default rate as a decimal in `[0, 1]`.
   * @returns Monthly MDR as a decimal.
   * @throws If `cdr` is negative, non-finite, or above 1.0.
   */
  cdrToMdr(cdr: number): number;

  /**
   * Convert a monthly MDR (monthly default rate) to an annual CDR.
   *
   * Uses `CDR = 1 - (1 - MDR)^12`.
   *
   * @param mdr - Monthly default rate as a decimal in `[0, 1]`.
   * @returns Annualized CDR as a decimal.
   * @throws If `mdr` is negative, non-finite, or above 1.0.
   */
  mdrToCdr(mdr: number): number;
}

/**
 * Namespaced TypeScript entry point for cashflows APIs.
 */
export declare const cashflows: CashflowsNamespace;

// --- covenants -------------------------------------------------------------

/**
 * One evaluated covenant test, as returned by `covenants.evaluateEngine`.
 *
 * Mirrors the Python `CovenantReport` getters field-for-field.
 */
export interface CovenantReport {
  /**
   * Human-readable description of the test, e.g. `"Debt/EBITDA <= 5.00x"`.
   * The object key, not this field, carries the covenant instance key.
   */
  covenant_type: string;
  /**
   * Stable machine-readable covenant instance identifier, when the engine set one.
   */
  covenant_id?: string;
  /**
   * Whether the covenant passed at the evaluation date.
   */
  passed: boolean;
  /**
   * Observed metric value in the covenant's own units.
   */
  actual_value: number | null;
  /**
   * Threshold the metric was tested against.
   */
  threshold: number | null;
  /**
   * Human-readable explanation of the pass/fail decision.
   */
  details: string | null;
  /**
   * Cushion relative to the threshold; positive means a passing buffer.
   */
  headroom: number | null;
  /**
   * Audit stamp: numeric mode, rounding context, and FX policy in force.
   */
  meta: Record<string, unknown>;
}

/**
 * Namespaced TypeScript entry points for covenants calculations and types.
 */
/**
 * JSON bridge to the Rust `finstack-quant-covenants` crate.
 * @example
 * ```typescript
 * import init, { covenants } from "finstack-quant-wasm";
 * await init();
 * const engine = JSON.parse(covenants.lboStandardJson(6, 2, 1.5, 50_000_000));
 * console.log(engine);
 * ```
 */
export interface CovenantsNamespace {
  /**
   * Validate and canonicalize a covenant spec JSON string.
   * @returns Canonical covenant-spec JSON after schema validation.
   * @param specJson - JSON-serialized covenant specification to validate.
   * @throws Error - Throws a JavaScript exception if `specJson` is malformed, does not match the covenant-spec schema, violates covenant threshold or frequency invariants, or cannot be serialized to canonical JSON.
   */
  validateCovenantSpecJson(specJson: string): string;
  /**
   * Validate and canonicalize a covenant report JSON string.
   * @returns Canonical covenant-report JSON after schema validation.
   * @param reportJson - JSON-serialized covenant evaluation report to validate.
   * @throws Error - Throws a JavaScript exception if `reportJson` is malformed, does not match the covenant-report schema, or cannot be serialized to canonical JSON.
   */
  validateCovenantReportJson(reportJson: string): string;
  /**
   * Validate and canonicalize a covenant engine JSON string.
   * @returns Canonical covenant-engine JSON after schema validation.
   * @param engineJson - JSON-serialized covenant engine and its covenant definitions.
   * @throws Error - Throws a JavaScript exception if `engineJson` is malformed, does not match the covenant-engine schema, contains an invalid covenant package, violates engine invariants, or cannot be serialized to canonical JSON.
   */
  validateCovenantEngineJson(engineJson: string): string;
  /**
   * Evaluate a covenant engine JSON string against a JSON metric map.
   * @returns A plain object keyed by covenant instance key, each value a `CovenantReport`.
   * @param engineJson - JSON-serialized covenant engine and its covenant definitions.
   * @param metricsJson - JSON object of financial metrics referenced by the covenant engine.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed or has the wrong schema, a metric is non-numeric, `asOf` is not a valid ISO date, the engine or required metrics fail validation, or the reports cannot be serialized to JavaScript.
   */
  evaluateEngine(
    engineJson: string,
    metricsJson: string,
    asOf: string
  ): Record<string, CovenantReport>;
  /**
   * Standard leveraged-buyout covenant package as JSON.
   * @returns Standard leveraged-buyout covenant package as canonical JSON.
   * @param initialLeverage - Maximum leverage ratio permitted at the initial test date.
   * @param interestCoverage - Minimum EBIT-to-interest coverage ratio in turns.
   * @param fixedChargeCoverage - Minimum EBITDA-to-fixed-charges coverage ratio.
   * @param maxCapex - Maximum annual capital expenditure amount in the caller's reporting currency.
   * @throws Error - Throws a JavaScript exception if any threshold is `NaN`, infinite or negative, or if the generated covenant package cannot be serialized to JSON.
   */
  lboStandardJson(
    initialLeverage: number,
    interestCoverage: number,
    fixedChargeCoverage: number,
    maxCapex: number
  ): string;
  /**
   * Covenant-lite package as JSON.
   * @returns Covenant-lite package as canonical JSON.
   * @param maxLeverage - Maximum total debt-to-EBITDA leverage ratio.
   * @param maxSeniorLeverage - Maximum senior-debt-to-EBITDA leverage ratio.
   * @throws Error - Throws a JavaScript exception if any threshold is `NaN`, infinite or negative, or if the generated covenant package cannot be serialized to JSON.
   */
  covLiteJson(maxLeverage: number, maxSeniorLeverage: number): string;
  /**
   * Real-estate covenant package as JSON.
   * @returns Real-estate covenant package as canonical JSON.
   * @param minDscr - Minimum debt-service coverage ratio.
   * @param minDebtYield - Minimum net-operating-income debt yield expressed as a decimal.
   * @param maxLtv - Maximum loan-to-value ratio expressed as a decimal.
   * @throws Error - Throws a JavaScript exception if any threshold is `NaN`, infinite or negative, or if the generated covenant package cannot be serialized to JSON.
   */
  realEstateJson(minDscr: number, minDebtYield: number, maxLtv: number): string;
  /**
   * Project-finance covenant package as JSON.
   * @returns Project-finance covenant package as canonical JSON.
   * @param minDscr - Minimum debt-service coverage ratio.
   * @param distributionLockupDscr - DSCR threshold below which borrower distributions are locked up.
   * @param minLiquidity - Minimum required liquidity reserve in the model's monetary units.
   * @param maxNetLeverage - Maximum net-debt-to-EBITDA leverage ratio.
   * @throws Error - Throws a JavaScript exception if any threshold is `NaN`, infinite or negative, or if the generated covenant package cannot be serialized to JSON.
   */
  projectFinanceJson(
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

/**
 * Opaque handle wrapping a parsed [`MarketContext`].
 *
 * Construct once from JSON, then pass to `priceInstrumentWithMarket` and
 * other `*WithMarket` pricing entry points. Eliminates the per-call
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
export declare class Market {
  /**
   * Parse a MarketContext from its JSON representation.
   *
   * @param json - Canonical MarketContext JSON, the same payload accepted by pricing `marketJson` arguments.
   * @returns A `Market` handle that can be reused across pricing calls.
   * @throws If the JSON is malformed or does not match the MarketContext schema.
   */
  constructor(json: string);
  /**
   * Serialize the wrapped MarketContext back to JSON.
   * @returns Canonical JSON string.
   * @throws Error - Throws a JavaScript exception if the market context cannot be serialized to JSON.
   */
  toJson(): string;
}

/**
 * Typed bond instrument handle; serialize with `toJson()` for generic pricing entry points.
 *
 * Thin wrapper over the canonical Rust `Bond`. Serialize with `toJson()` and
 * pass the result to `valuations.instruments.priceInstrument` (or the other
 * generic pricing entry points) to price it.
 */
export interface Bond extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Stable instrument identifier.
   */
  readonly id: string;
  /**
   * Serialize to a canonical `finstack_quant.instrument/1` envelope.
   *
   * Pass the result to `valuations.instruments.priceInstrument` (or the
   * other generic pricing entry points) to price this bond.
   * @returns Canonical instrument envelope accepted by `priceInstrument` and `Bond.fromJson`.
   * @throws If serialization fails.
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
 *   "none",
 *   "USD-OIS"
 * );
 * const result = valuations.instruments.priceInstrument(bond.toJson(), marketJson, "2024-06-30", "default");
 * ```
 */
export interface BondConstructor {
  /**
   * Create a US corporate fixed-rate bond (semi-annual, 30/360, T+1).
   * Mirrors Rust `Bond::fixed` and requires an explicit stub policy.
   * @param id - Unique instrument identifier.
   * @param notional - Principal amount of the bond.
   * @param couponRate - Annual coupon rate.
   * @param issue - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param stub - Stub policy: `none`, `short_front`, `short_back`, `long_front`, or `long_back`.
   * @param discountCurveId - Discount curve identifier used for pricing.
   * @returns The validated fixed-rate bond.
   * @throws If validation fails (e.g. maturity not after issue).
   */
  fixed(
    id: string,
    notional: Money,
    couponRate: Rate,
    issue: string,
    maturity: string,
    stub: 'none' | 'short_front' | 'short_back' | 'long_front' | 'long_back',
    discountCurveId: string
  ): Bond;
  /**
   * Create a floating-rate bond (FRN) linked to a forward index. Mirrors Rust `Bond::floating`. Settlement, calendar, and business-day convention come from the notional currency: USD UsCorporate (T+1, usny), EUR EurCorporate (T+2, target2), GBP UkGilt (T+1), JPY Jgb (T+2). Unmapped currencies throw.
   * @param id - Unique instrument identifier.
   * @param notional - Principal amount of the bond.
   * @param indexId - Forward curve identifier (e.g. `"USD-SOFR-3M"`).
   * @param marginBp - Spread over the index in whole basis points (`Bps` rejects fractional values; use `Bond.fromJson` for sub-bp margins, which preserves the exact decimal spread).
   * @param issue - Issue date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param maturity - Maturity date as an ISO-8601 string (`"YYYY-MM-DD"`).
   * @param frequency - Payment frequency (e.g. `Tenor.quarterly()`).
   * @param dayCount - Day count convention (e.g. `DayCount.act360()`).
   * @param discountCurveId - Discount curve identifier used for pricing.
   * @returns The validated floating-rate note.
   * @throws If the notional currency has no mapped settlement convention or validation fails.
   */
  floating(
    id: string,
    notional: Money,
    indexId: string,
    marginBp: Bps,
    issue: string,
    maturity: string,
    frequency: Tenor,
    dayCount: DayCount,
    discountCurveId: string
  ): Bond;
  /**
   * Deserialize a bond from its canonical v1 instrument envelope.
   *
   * Bare payloads are rejected; the loader's validation runs on the result.
   * @param json - A `finstack_quant.instrument/1` envelope containing type `"bond"`.
   * @returns The validated bond.
   * @throws If the JSON is malformed, has a different instrument type, or fails validation.
   */
  fromJson(json: string): Bond;
}

/**
 * Typed term-loan instrument handle; serialize with `toJson()` for generic pricing entry points.
 *
 * Thin wrapper over the canonical Rust `TermLoan`. Serialize with `toJson()`
 * and pass the result to `valuations.instruments.priceInstrument` (or the
 * other generic pricing entry points) to price it.
 */
export interface TermLoan extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Stable instrument identifier.
   */
  readonly id: string;
  /**
   * Serialize to a canonical `finstack_quant.instrument/1` envelope.
   *
   * Pass the result to `valuations.instruments.priceInstrument` (or the
   * other generic pricing entry points) to price this loan.
   * @returns Canonical instrument envelope accepted by `priceInstrument` and `TermLoan.fromJson`.
   * @throws If serialization fails.
   */
  toJson(): string;
}

/**
 * Constructor surface for the typed `TermLoan` WebAssembly instrument.
 *
 * Rust has no `fixed`/`floating` convenience constructors for term loans;
 * construct via `fromJson` with a canonical v1 instrument envelope or start
 * from `example()`.
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
   * Deserialize a term loan from its canonical v1 instrument envelope.
   *
   * Bare payloads are rejected; the loader's validation runs on the result.
   * @param json - A `finstack_quant.instrument/1` envelope containing type `"term_loan"`.
   * @returns The validated term loan.
   * @throws If the JSON is malformed, has a different instrument type, or fails validation.
   */
  fromJson(json: string): TermLoan;
  /**
   * Canonical example term loan (mirrors Rust `TermLoan::example`).
   *
   * Returns a 5-year USD fixed-rate loan (6%, quarterly, Act/360, 2.5%
   * per-period amortization) useful as a starting point and in tests.
   * @returns The example loan.
   * @throws If construction fails (should not occur).
   */
  example(): TermLoan;
}

/**
 * Borrowing base of a collateral pool on the closing balances, as returned by
 * `AssetBackedFacility.borrowingBase`. Money values are plain
 * `{ amount, currency }` objects.
 */
export interface BorrowingBaseReport {
  /**
   * Collateral balance that meets the eligibility criteria, before the
   * concentration limits.
   */
  eligible_collateral: MoneyValue;
  /**
   * Eligible balance excluded by the concentration limits.
   */
  concentration_excess: MoneyValue;
  /**
   * Advance-rate-weighted eligible collateral after the limits.
   */
  borrowing_base: MoneyValue;
}

/**
 * Typed asset-backed facility handle (a warehouse line against a collateral pool); serialize with `toJson()` for generic pricing entry points.
 *
 * Thin wrapper over the canonical Rust `AssetBackedFacility`. Serialize with
 * `toJson()` and pass the result to `valuations.instruments.priceInstrument`
 * (or the other generic pricing entry points) to price the lender's flows;
 * the facility's borrowing-base metrics (`abf_borrowing_base`,
 * `abf_borrowing_base_cushion`, `abf_advance_rate_utilization`,
 * `abf_facility_irr`, `abf_residual_irr`) are available there too.
 */
export interface AssetBackedFacility extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Stable instrument identifier.
   */
  readonly id: string;
  /**
   * Serialize to a canonical `finstack_quant.instrument/1` envelope.
   * @returns Canonical instrument envelope accepted by `priceInstrument` and `AssetBackedFacility.fromJson`.
   * @throws If serialization fails.
   */
  toJson(): string;
  /**
   * Borrowing base on the closing collateral.
   * @returns Plain `BorrowingBaseReport` object with `eligible_collateral`, `concentration_excess` and `borrowing_base` Money values.
   * @throws If the borrowing-base rules are malformed.
   */
  borrowingBase(): BorrowingBaseReport;
}

/**
 * Constructor surface for the typed `AssetBackedFacility` WebAssembly instrument.
 *
 * Construct via `fromJson` with a canonical v1 instrument envelope or start
 * from `example()`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const facility = valuations.instruments.AssetBackedFacility.example();
 * const base = facility.borrowingBase();
 * const result = valuations.instruments.priceInstrument(facility.toJson(), marketJson, "2024-01-15", "default");
 * ```
 */
export interface AssetBackedFacilityConstructor {
  /**
   * Parse a canonical `finstack_quant.instrument/1` envelope whose instrument is an `asset_backed_facility`.
   * @param json - Canonical instrument envelope JSON.
   * @returns The typed facility.
   * @throws If the JSON is malformed, has a different instrument type, or fails validation.
   */
  fromJson(json: string): AssetBackedFacility;
  /**
   * The canonical example facility: the example CLO pool financed by a USD 80M commitment drawn USD 70M.
   * @returns The example facility.
   * @throws If construction fails (should not occur).
   */
  example(): AssetBackedFacility;
}

/**
 * Typed revolving-credit facility handle; serialize with `toJson()` for generic pricing entry points.
 *
 * Thin wrapper over the canonical Rust `RevolvingCredit`. Serialize with
 * `toJson()` and pass the result to `valuations.instruments.priceInstrument`
 * (or the other generic pricing entry points) to price it.
 */
export interface RevolvingCredit extends WasmOwned {
  /**
   * Instrument identifier.
   * @returns Stable instrument identifier.
   */
  readonly id: string;
  /**
   * Serialize to a canonical `finstack_quant.instrument/1` envelope.
   *
   * Pass the result to `valuations.instruments.priceInstrument` (or the
   * other generic pricing entry points) to price this facility.
   * @returns Canonical instrument envelope accepted by `priceInstrument` and `RevolvingCredit.fromJson`.
   * @throws If serialization fails.
   */
  toJson(): string;
  /**
   * Price a stochastic facility with the path-retaining Monte Carlo engine.
   *
   * Mirrors Rust `RevolvingCreditPricer::price_with_paths` and Python
   * `RevolvingCredit.price_with_paths`: every simulated path is kept with
   * its present value, utilization and credit-spread samples, cashflows and
   * draw option cost, next to the antithetic-aware estimates. The draw
   * option cost is negative when draws at the fixed margin are worth less
   * than at the path's fair spread.
   * @param marketJson - Serialized `MarketContext` holding the facility's discount curve, its floating index forward curve and fixings, and the credit curve of a market-anchored spread process.
   * @param asOf - Valuation date as an ISO 8601 `YYYY-MM-DD` string.
   * @returns Plain `EnhancedMonteCarloResult` object with `mc_result`, one `path_results` entry per simulated path and the `draw_option_cost` estimate; path counts are plain numbers and `mc_result.run` is `null`.
   * @throws If the market JSON or date is malformed, a required curve or fixing is missing, or the facility has a deterministic draw schedule (only stochastic facilities simulate paths).
   */
  priceWithPaths(marketJson: string, asOf: string): EnhancedMonteCarloResult;
}

/**
 * Path-retaining Monte Carlo result of a stochastic revolving credit facility
 * (`RevolvingCredit.priceWithPaths`), in the serde wire shape of the Rust
 * `EnhancedMonteCarloResult`.
 */
export interface EnhancedMonteCarloResult {
  /**
   * Aggregate Monte Carlo result: the present-value `estimate` (`mean` money
   * value, `stderr`, `ci_95` bounds, `num_paths` antithetic pairs and
   * `num_simulated_paths` as plain numbers) plus `paths` and `run`, both
   * `null` for this pricer (serde shape of the Rust `MonteCarloResult`).
   */
  mc_result: Record<string, unknown>;
  /**
   * One entry per simulated path in path order: `pv` and `draw_option_cost`
   * money values, the utilization, credit-spread and rate samples under
   * `path_data`, and the path's dated `cashflows`.
   */
  path_results: Array<Record<string, unknown>>;
  /**
   * Draw option cost across paths: `mean` money value, `stderr`, `ci_95`
   * bounds and the path counts; negative when draws at the fixed margin are
   * worth less than at the path's fair spread.
   */
  draw_option_cost: Record<string, unknown>;
}

/**
 * Constructor surface for the typed `RevolvingCredit` WebAssembly instrument.
 *
 * Construct via `fromJson` with a canonical v1 instrument envelope or start
 * from `example()`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const facility = valuations.instruments.RevolvingCredit.example();
 * const result = valuations.instruments.priceInstrument(facility.toJson(), marketJson, "2024-06-30", "default");
 * ```
 */
export interface RevolvingCreditConstructor {
  /**
   * Deserialize a revolving credit facility from its canonical v1 instrument envelope.
   *
   * Bare payloads are rejected; the loader's validation runs on the result.
   * @param json - A `finstack_quant.instrument/1` envelope containing type `"revolving_credit"`.
   * @returns The validated facility.
   * @throws If the JSON is malformed, has a different instrument type, or fails validation.
   */
  fromJson(json: string): RevolvingCredit;
  /**
   * Canonical example facility (mirrors Rust `RevolvingCredit::example`).
   *
   * Returns a three-year USD 50M SOFR + 250bp facility with USD 10M drawn
   * and a scheduled draw and repayment, useful as a starting point and in tests.
   * @returns The example facility.
   * @throws If construction fails (should not occur).
   */
  example(): RevolvingCredit;
}

/**
 * Currency-tagged monetary amount as carried on the wire.
 *
 * `amount` is an exact decimal **string** (not a JS number) so no precision is
 * lost crossing the boundary; parse it with `Number(...)` when a float is
 * acceptable.
 */
export interface MoneyValue {
  /**
   * Exact decimal amount, rounded to the currency's ISO 4217 minor-unit scale.
   */
  amount: string;
  /**
   * ISO 4217 currency code, e.g. `"USD"`.
   */
  currency: string;
}

/**
 * Convergence and reproducibility diagnostics for a Monte Carlo valuation.
 */
export interface MonteCarloValuationDetails {
  /**
   * Registered model key used for the simulation.
   */
  model_key: string;
  /**
   * Sampling standard error of the discounted PV mean in the result currency.
   * For LSMC valuations, this measures pricing-path uncertainty under the
   * frozen fitted exercise policy. It excludes regression approximation,
   * time-grid discretization, and model error.
   */
  standard_error: number;
  /**
   * Configured independent paths used to fit the exercise or control policy.
   */
  training_paths: number;
  /**
   * Training factor paths including antithetic partners.
   */
  training_simulated_paths: number;
  /**
   * Configured independent paths used to fit state-conditional make-whole
   * reference values; zero when that stage is absent.
   */
  make_whole_training_paths: number;
  /**
   * Make-whole training paths including antithetic partners; zero when that
   * stage is absent.
   */
  make_whole_training_simulated_paths: number;
  /**
   * Independent path estimators contributing to the reported mean.
   */
  estimator_paths: number;
  /**
   * Total estimator paths including antithetic partners.
   */
  simulated_paths: number;
  /**
   * Deterministic random seed used for the run, preserved across the full
   * unsigned 64-bit range.
   */
  seed: bigint;
  /**
   * Simulation times in year fractions, including zero and maturity.
   */
  time_grid: number[];
  /**
   * Whether antithetic variates were enabled.
   */
  antithetic: boolean;
  /**
   * Whether Sobol quasi-random sampling was enabled.
   */
  sobol: boolean;
  /**
   * Whether Brownian-bridge ordering was enabled for Sobol paths.
   */
  brownian_bridge: boolean;
}

/**
 * Model-specific detail carried by a valuation result.
 */
export type ValuationDetails =
  | { type: 'monte_carlo'; data: MonteCarloValuationDetails }
  | {
      type: 'composite' | 'credit_derivative' | 'structured_credit_stochastic' | 'fx';
      data: unknown;
    };

/**
 * Valuation envelope returned by the `priceInstrument*` entry points.
 *
 * This is the same document Python callers hold as
 * `finstack_quant.valuations.ValuationResult` — field names are the canonical
 * Rust serde names. Monte Carlo details carry `seed` as a lossless `bigint`,
 * so stringify stochastic results with a BigInt-aware replacer such as
 * `JSON.stringify(result, (_, value) => typeof value === "bigint" ? value.toString() : value)`.
 * Python's `price` / `currency` getters correspond to `value.amount` /
 * `value.currency` here.
 */
export interface ValuationResult {
  /**
   * Wire-format schema version; only `1` is emitted.
   */
  schema_version: number;
  /**
   * Identifier of the priced instrument.
   */
  instrument_id: string;
  /**
   * ISO-8601 valuation date.
   */
  as_of: string;
  /**
   * Present value in the instrument's native currency.
   */
  value: MoneyValue;
  /**
   * Requested risk measures keyed by canonical metric ID.
   */
  measures: Record<string, number>;
  /**
   * Model-specific structured detail, when the pricer emits one.
   *
   * Stochastic `"rates_credit"` bond pricing emits `type: "monte_carlo"` with
   * convergence, path-count, seed, time-grid, and variance-reduction data.
   */
  details?: ValuationDetails;
  /**
   * Policy stamps: numeric mode, rounding context, FX policy, timing.
   */
  meta: Record<string, unknown>;
  /**
   * Covenant reports for instruments that carry covenants; `null` otherwise.
   */
  covenants: Record<string, unknown> | null;
  /**
   * Computation trace, present only when explain mode is enabled.
   */
  explanation?: unknown;
}

/**
 * Maintained valuation-routing row for one liquid exchange-listed derivative family.
 */
export interface ListedProductCoverage {
  /**
   * Exchange venue that lists the product family.
   */
  exchange: 'cme' | 'eurex' | 'montreal' | 'sgx';
  /**
   * Comma-separated exchange root symbols covered by this row.
   */
  symbols: string;
  /**
   * Human-readable name of the exchange product family.
   */
  name: string;
  /**
   * Broad asset class used to organize the product family.
   */
  asset_class: string;
  /**
   * Exchange form: future, option on future, or direct option.
   */
  product_kind: 'future' | 'option_on_future' | 'option';
  /**
   * Canonical Finstack instrument-type tag used for valuation dispatch.
   */
  instrument_type: string;
  /**
   * Core valuation readiness of the mapped instrument route.
   */
  status: 'native' | 'composed' | 'partial';
  /**
   * Exchange-contract features exercised by the mapped valuation route.
   */
  features: string[];
  /**
   * Residual exchange feature not included in the model value, when applicable.
   */
  residual_gap?: string;
  /**
   * Official exchange page used to verify the listed product family.
   */
  source_url: string;
}

/**
 * Listed-market product coverage and exchange routing metadata.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const rows = valuations.market.listedProductCatalog("cme");
 * console.log(rows.every((row) => row.exchange === "cme"));
 * ```
 */
export interface ValuationMarketNamespace {
  /**
   * Return the maintained liquid listed-derivatives coverage catalog.
   * @param exchange - Optional exact filter: `"cme"`, `"eurex"`, `"montreal"`, or `"sgx"`.
   * @returns Product-family coverage rows with instrument routes and official source URLs.
   * @throws Error - Throws when `exchange` is unsupported, the embedded listed-product sidecar is invalid, or rows cannot be converted to JavaScript.
   */
  listedProductCatalog(
    exchange?: 'cme' | 'eurex' | 'montreal' | 'sgx' | null
  ): ListedProductCoverage[];
}

/**
 * Namespaced TypeScript entry points for valuation instruments calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const models = valuations.instruments.listModels();
 * console.log(models.includes("discounting"));
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
   * Typed `RevolvingCredit` instrument class (see `RevolvingCreditConstructor`).
   */
  RevolvingCredit: RevolvingCreditConstructor;
  /**
   * Typed `AssetBackedFacility` instrument class (see `AssetBackedFacilityConstructor`).
   */
  AssetBackedFacility: AssetBackedFacilityConstructor;
  /**
   * Construct a canonical bond instrument envelope from a cashflow schedule.
   * @returns Canonical bond instrument envelope JSON.
   * @param instrumentId - Stable instrument identifier used for pricing and metric keys.
   * @param scheduleJson - Canonical cashflow-schedule JSON used to construct the fixed-income instrument.
   * @param discountCurveId - Market-context discount-curve identifier for the instrument currency.
   * @param quotedClean - Optional observed clean bond price in the schedule's documented price quotation convention.
   * @throws Error - Throws a JavaScript exception if `scheduleJson` is malformed or violates cash-flow invariants, bond construction fails, or the canonical bond envelope cannot be serialized.
   */
  bondFromCashflowsJson(
    instrumentId: string,
    scheduleJson: string,
    discountCurveId: string,
    quotedClean?: number | null
  ): string;
  /**
   * Validate a canonical v1 instrument envelope.
   *
   * Bare instrument payloads are rejected. Returns canonical re-serialized JSON.
   * @returns Canonical instrument envelope JSON after schema validation.
   * @param json - Required `finstack_quant.instrument/1` envelope.
   * @throws Error - Throws a JavaScript exception if `json` is malformed, is not a canonical v1 instrument envelope, fails instrument validation, or cannot be canonically serialized.
   */
  validateInstrumentJson(json: string): string;
  /**
   * Price an instrument from its canonical envelope and return a `ValuationResult` object.
   *
   * Pass `model = "default"` to use the instrument-native default model.
   * Fields are readable directly (`result.value.amount`,
   * `result.measures.dv01`). Monte Carlo results carry a lossless `bigint`
   * seed; use a BigInt-aware replacer when stringifying them.
   * For bonds, `"discounting"` is non-callable rates-only PV,
   * `"hazard_rate"` is non-callable fractional recovery of par, `"tree"`
   * values rates-only exercise rights, and `"rates_credit"` values joint
   * rates-credit bonds including call, put, and return floors. Stochastic `"rates_credit"` runs add
   * `type: "monte_carlo"` diagnostics to `result.details`.
   * @param instrumentJson - Required `finstack_quant.instrument/1` envelope.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Optional pricing-model identifier; omit for the instrument-native model.
   * @param metrics - Optional canonical metric IDs such as `"ytm"`, `"dv01"`, `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a valuation-only result. Mortgage OAS and CMO Z-spread consume clean prices per 100 current face and include settlement accrued interest. MBS DV01, bucketed DV01 and duration share rate-dependent prepayment assumptions. FI TRS duration DV01 requires `duration_id` and a finite signed scalar in years. Roll specialness is in basis points versus `repo_curve_id` (a forward curve), or the discount curve when omitted; implied financing is an ACT/360 decimal.
   * @param pricingOptions - Optional JSON metric-pricing overrides merged into the envelope before validation. Omit, `null`, or `undefined` to use the envelope as-is.
   * @param marketHistory - Optional serialized market-history JSON required by historical risk metrics such as historical VaR.
   * @returns Plain JavaScript `ValuationResult` (`instrument_id`, `as_of`, `value`, `measures`, `meta`, …).
   * @throws Error - Throws a JavaScript exception if an instrument, market, pricing-option, or market-history payload is invalid; `metrics` is not a string array; `asOf`, `model`, or a metric identifier is invalid; required market data is missing; pricing or a metric calculation fails; or the valuation cannot be converted to a JavaScript value.
   */
  priceInstrument(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    model?: string | null,
    metrics?: string[] | null,
    pricingOptions?: string | null,
    marketHistory?: string | null
  ): ValuationResult;
  /**
   * Price an instrument using a pre-parsed [`Market`].
   *
   * Avoids the per-call market-parse overhead of `priceInstrument`.
   * For bonds, `"discounting"` is non-callable rates-only PV,
   * `"hazard_rate"` is non-callable fractional recovery of par, `"tree"`
   * values rates-only exercise rights, and `"rates_credit"` values joint
   * rates-credit bonds including call, put, and return floors. Stochastic `"rates_credit"` runs add
   * `type: "monte_carlo"` diagnostics to `result.details`.
   * Their `seed` is a lossless `bigint`, so `JSON.stringify` requires a
   * BigInt-aware replacer.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param market - Pre-parsed `Market` handle supplying curves, quotes, and FX data for this call.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Pricing-model identifier; use `"default"` for the instrument-native model when supported.
   * @param metrics - Optional canonical metric IDs such as `"ytm"`, `"dv01"`, `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a valuation-only result.
   * @param pricingOptions - Optional JSON metric-pricing overrides merged into the envelope before validation. Omit, `null`, or `undefined` to use the envelope as-is.
   * @param marketHistory - Optional serialized market-history JSON required by historical risk metrics such as historical VaR.
   * @returns Plain JavaScript `ValuationResult` (`instrument_id`, `as_of`, `value`, `measures`, `meta`, …).
   * @throws Error - Throws a JavaScript exception if an instrument, pricing-option, or market- history payload is invalid; `metrics` is not a string array; `asOf`, `model`, or a metric identifier is invalid; required market data is missing; pricing or a metric calculation fails; or the valuation cannot be converted to a JavaScript value.
   */
  priceInstrumentWithMarket(
    instrumentJson: string,
    market: Market,
    asOf: string,
    model: string,
    metrics?: string[] | null,
    pricingOptions?: string | null,
    marketHistory?: string | null
  ): ValuationResult;
  /**
   * Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.
   *
   * `model` must be `"discounting"` or `"hazard_rate"`. Unsupported models or
   * incompatible instrument types throw. Hazard-rate export also rejects bonds
   * with call, put, or return-floor rights because static rows cannot represent
   * exercise-contingent value. For supported static-flow pairs, the envelope's
   * `total_pv` matches the instrument's `base_value` within rounding.
   * @returns Per-flow cashflow envelope JSON (discount factor, survival, PV).
   * @param instrumentJson - Required `finstack_quant.instrument/1` envelope.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON or `asOf` is invalid, `model` is unsupported or incompatible with the instrument, a bond with embedded exercise rights is requested under a static cashflow model, required curves are missing, the schedule mixes currencies, canonical pricing fails, or the cash-flow envelope cannot be serialized.
   */
  instrumentCashflowsJson(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    model: string
  ): string;
  /**
   * Per-flow cashflow envelope using a pre-parsed [`Market`]. Hazard-rate export
   * rejects bonds with call, put, or return-floor rights because static rows
   * cannot represent exercise-contingent value.
   * @returns Per-flow cashflow envelope JSON using the pre-parsed market.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param market - Market context or JSON payload supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param model - Must be `"discounting"` or `"hazard_rate"`; `"default"` is not accepted.
   * @throws Error - Throws a JavaScript exception if `instrumentJson` or `asOf` is invalid, `model` is unsupported or incompatible with the instrument, a bond with embedded exercise rights is requested under a static cashflow model, required curves are missing, the schedule mixes currencies, canonical pricing fails, or the cash-flow envelope cannot be serialized.
   */
  instrumentCashflowsWithMarketJson(
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
   * sorted array of canonical keys (`"discounting"`, `"rates_credit"`, …)
   * accepted by the `model` argument of `priceInstrument`.
   * @returns Returns the resulting `string[]` collection in ascending model-key order.
   * @throws Error - Throws a JavaScript exception if the model key list cannot be converted to a JavaScript value.
   */
  listModels(): string[];
  /**
   * List the standard registry's pricing models grouped by instrument type.
   *
   * Returns a JSON object `{ instrument_type: [model_key, ...], ... }`. Only
   * instrument types with at least one registered pricer appear, and each
   * entry lists only the models that can actually price that instrument. The
   * `"bond"` entry includes `"discounting"`, `"hazard_rate"`, `"tree"`, and
   * `"rates_credit"`.
   * @returns Returns the resulting `Record<string, string[]>` value keyed by instrument type.
   * @throws Error - Throws a JavaScript exception if the grouped model registry cannot be converted to a JavaScript value.
   */
  listModelsGrouped(): Record<string, string[]>;
  /**
   * List all metric IDs in the standard metric registry.
   * @returns Canonical metric identifiers, sorted alphabetically.
   * @throws Error - Throws a JavaScript exception if the metric identifier list cannot be converted to a JavaScript value.
   */
  listStandardMetrics(): string[];
  /**
   * List all standard metrics organized by group.
   *
   * Returns a JSON object `{ group_name: [metric_id, ...], ... }` where
   * each key is a human-readable group name (e.g. "Pricing", "Greeks",
   * "Sensitivity") and the value is a sorted array of metric ID strings.
   * @returns Metric identifiers grouped by human-readable group name.
   * @throws Error - Throws a JavaScript exception if the grouped metric registry cannot be converted to a JavaScript value.
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
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Identifier of the floating-rate tranche whose contractual cashflows are spread-discounted.
   * @param marketJson - Canonical market-context JSON supplying the discount curve and any forward curves or historical fixings required for cashflow projection.
   * @param asOf - ISO-8601 valuation date used for projection and discounting.
   * @param targetPv - Positive dirty settlement amount in tranche currency, including accrued interest once; settlement uses the deal's quote_settlement_date or the valuation date.
   * @returns The z-spread-equivalent discount margin in decimal units.
   * @throws Error - Thrown if JSON or the date is malformed, the deal is invalid, the tranche is missing or fixed-rate, target_pv is non-finite, required market data is unavailable, or the spread solve fails or exceeds ±5000 bp.
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
   * @returns Break-even constant default rate as a decimal, such as `0.02` for 2% CDR.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON is malformed; the instrument fails pricing validation or is not a structured-credit deal; `as_of` is invalid; the tranche or required market data is missing; or the break-even calculation fails.
   */
  structuredCreditTrancheBreakevenCdr(
    instrumentJson: string,
    trancheId: string,
    marketJson: string,
    asOf: string
  ): number;
  /**
   * Option-adjusted spread for a tranche; returns a typed `OasResult` object.
   *
   * The result is a plain object with snake_case fields — the same shape
   * Python exposes through its typed `OasResult` wrapper. Pass it to
   * `JSON.stringify` if a wire string is needed.
   *
   * `marketPricePct` is the quoted price as a percentage of original balance.
   * `config`, when present, is a JSON `OasConfig`; the default is used otherwise.
   * @returns Typed `OasResult` object for the tranche.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketPricePct - Clean settlement quote as a percentage of original balance; accrued interest is added once.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Throws a JavaScript exception if the instrument, market, or optional configuration JSON is malformed; the instrument fails pricing validation; `as_of` is invalid; the tranche or discount curve is missing; the OAS solve fails or produces a non-finite result; or the result cannot be converted to a JavaScript value.
   * @param config - Config used by this call.
   */
  structuredCreditTrancheOas(
    instrumentJson: string,
    trancheId: string,
    marketPricePct: number,
    marketJson: string,
    asOf: string,
    config?: string | null
  ): OasResult;
  /**
   * Scenario (CPR x CDR x severity) table for a tranche; returns a typed
   * `ScenarioTable` object. `grid` is a JSON `ScenarioGrid` (`cprs`, `cdrs`,
   * `severities`).
   *
   * The result is a plain object with snake_case fields — the same shape
   * Python exposes through its typed `ScenarioTable` wrapper. Pass it to
   * `JSON.stringify` if a wire string is needed.
   * @returns Typed `ScenarioTable` object over the CPR/CDR/severity grid.
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Throws a JavaScript exception if the instrument, market, or scenario-grid JSON is malformed; the instrument fails pricing validation; `as_of` is invalid; the tranche or required market data is missing; a scenario fails or produces a non-finite result; or the table cannot be converted to a JavaScript value.
   * @param grid - Grid as a string.
   */
  structuredCreditTrancheScenarioTable(
    instrumentJson: string,
    trancheId: string,
    marketJson: string,
    asOf: string,
    grid: string
  ): ScenarioTable;
  /**
   * Per-tranche risk/spread metrics (PV, price, WAL, z-spread, CS01, spread/
   * modified duration, convexity) computed from one tranche's own cashflows.
   *
   * `marketPricePct`, when provided, is the quoted price (% of original balance)
   * the z-spread and CS01 are solved against; otherwise the tranche's own model
   * price is used (zero z-spread). Returns a typed `TrancheMetrics` object —
   * a plain object with the same snake_case fields Python exposes through its
   * typed `TrancheMetrics` wrapper. Pass it to `JSON.stringify` if a wire
   * string is needed.
   * @returns Typed `TrancheMetrics` object (PV, price, WAL, z-spread, CS01, duration, convexity).
   * @param instrumentJson - Canonical instrument envelope JSON in the Finstack v1 schema.
   * @param trancheId - Stable tranche identifier used to select the required domain object.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param marketPricePct - Optional clean settlement quote as a percentage of original balance; omit to use the deal quote, or its model clean price when no quote is supplied.
   * @throws Error - Throws a JavaScript exception if the instrument or market JSON is malformed; the instrument fails pricing validation; `as_of` is invalid; the tranche or discount curve is missing; a metric fails or is non-finite; or the result cannot be converted to a JavaScript value.
   */
  structuredCreditTrancheMetrics(
    instrumentJson: string,
    trancheId: string,
    marketJson: string,
    asOf: string,
    marketPricePct?: number | null
  ): TrancheMetrics;
}

/**
 * Option-adjusted-spread result for a structured-credit tranche, as returned
 * by `valuations.instruments.structuredCreditTrancheOas`. Field names and
 * units match the Rust `OasResult` and Python's typed `OasResult` wrapper.
 */
export interface OasResult {
  /**
   * Option-adjusted spread, as an annual decimal (`0.01` = 100 bp).
   */
  oas: number;
  /**
   * Model price at the solved OAS, as a percentage of the tranche's current
   * balance (the factor-adjusted quote basis).
   */
  model_price: number;
  /**
   * Target market price, as a percentage of current balance.
   */
  market_price: number;
  /**
   * Number of Monte-Carlo scenarios used.
   */
  num_paths: number;
  /**
   * Monte-Carlo standard error of the mean price, as a percentage of
   * current balance.
   */
  price_std_error: number;
}

/**
 * Summary risk/pricing metrics for a structured-credit tranche, as returned
 * by `valuations.instruments.structuredCreditTrancheMetrics`. Field names and
 * units match the Rust `TrancheMetrics` and Python's typed wrapper.
 */
export interface TrancheMetrics {
  /**
   * Identifier of the tranche.
   */
  tranche_id: string;
  /**
   * ISO-4217 code of the currency `pv` and `cs01` are denominated in.
   */
  currency: string;
  /**
   * Present value of the tranche, in `currency` units.
   */
  pv: number;
  /**
   * Model clean price, as a percentage of the tranche's current balance (the
   * factor-adjusted secondary-market quote basis).
   */
  price_pct: number;
  /**
   * Pool factor of the note: current balance over original balance, so a
   * price on original face is `price_pct * factor`.
   */
  factor: number;
  /**
   * Weighted-average life, in years.
   */
  wal: number;
  /**
   * Z-spread to `target_price_pct`, in basis points.
   */
  z_spread_bp: number;
  /**
   * Credit-spread DV01 — currency change for a +1 bp z-spread shock, in
   * `currency` units. Negative for a long tranche.
   */
  cs01: number;
  /**
   * Spread duration, in years (`-cs01 / (pv * 1bp)`).
   */
  spread_duration: number;
  /**
   * Spread convexity at the solved z-spread, in years squared.
   */
  spread_convexity: number;
  /**
   * Effective (rate) duration, in years: the cashflows are re-projected with
   * every rate curve bumped ±1 bp, so a floater's coupon resets move with the
   * curve and its duration is short.
   */
  modified_duration: number;
  /**
   * Effective convexity from the same ±1 bp re-projection, in years squared.
   */
  convexity: number;
  /**
   * Price the z-spread/CS01 were solved against, as a percentage of current
   * balance.
   */
  target_price_pct: number;
  /**
   * Weighted-average life to the deal's assumed call, in years. Absent when
   * the deal carries no `call_assumption` covering this tranche.
   */
  wal_to_call?: number;
  /**
   * Z-spread of the to-call cashflows to `target_price_pct`, in basis
   * points. Absent without a call.
   */
  z_spread_to_call_bp?: number;
  /**
   * Discount margin to call for a floating-rate tranche, in basis points.
   * Absent for fixed-rate tranches or without a call.
   */
  dm_to_call_bp?: number;
  /**
   * Discount margin to maturity at `target_price_pct` for a floating-rate
   * tranche, in basis points. Absent for fixed-rate tranches.
   */
  dm_bp?: number;
}

/**
 * One evaluated scenario cell of a structured-credit tranche scenario table.
 */
export interface TrancheScenarioCell {
  /**
   * Constant prepayment rate for the cell, annual decimal.
   */
  cpr: number;
  /**
   * Constant default rate for the cell, annual decimal.
   */
  cdr: number;
  /**
   * Loss severity for the cell, decimal.
   */
  severity: number;
  /**
   * Tranche price, as a percentage of original balance.
   */
  price: number;
  /**
   * Weighted-average life, in years.
   */
  wal: number;
  /**
   * Principal writedown, in currency units.
   */
  writedown: number;
}

/**
 * Scenario (CPR x CDR x severity) table for a structured-credit tranche, as
 * returned by `valuations.instruments.structuredCreditTrancheScenarioTable`.
 * Field names and units match the Rust `ScenarioTable` and Python's typed
 * wrapper.
 */
export interface ScenarioTable {
  /**
   * Identifier of the tranche evaluated.
   */
  tranche_id: string;
  /**
   * Evaluated cells, in CPR-major, then CDR, then severity order.
   */
  cells: TrancheScenarioCell[];
}

/**
 * FX instrument specification as a JSON object or a canonical JSON string.
 */
export type FxInstrumentSpec = Record<string, unknown> | string;

/**
 * FX instrument handle priced against a market context.
 */
export interface FxInstrument extends WasmOwned {
  /**
   * Instrument identifier (mirrors the Python typed wrappers' `id` property).
   */
  readonly id: string;
  /**
   * Serialize this `FxInstrument` value to canonical JSON.
   * @returns Canonical JSON instrument specification.
   */
  toJson(): string;
  /**
   * Price this FX instrument against the supplied market.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @param metrics - Optional canonical metric IDs such as `"delta"`, `"vega"`, `"hvar"`, or `"expected_shortfall"`. Omit, `null`, or `undefined` for a valuation-only result.
   * @param pricingOptions - Optional JSON metric-pricing overrides merged into the envelope before validation. Omit, `null`, or `undefined` to use the envelope as-is.
   * @param marketHistory - Optional serialized market-history JSON required by historical risk metrics such as historical VaR.
   * @returns Structured `ValuationResult` for the selected model.
   * @throws Error - Throws a JavaScript exception if the instrument, market, pricing-option, or market-history JSON is invalid; `metrics` is not a string array; `asOf`, `model`, or a metric identifier is invalid; required market data is missing; pricing or a metric calculation fails; or the valuation cannot be converted to JavaScript.
   */
  price(
    marketJson: string,
    asOf: string,
    model?: string | null,
    metrics?: string[] | null,
    pricingOptions?: string | null,
    marketHistory?: string | null
  ): ValuationResult;
}

/**
 * FX vanilla option handle with first-order greeks.
 */
export interface FxOptionInstrument extends FxInstrument {
  /**
   * Spot delta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot delta: change in value per unit spot.
   */
  delta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Spot gamma of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot gamma: change in delta per unit spot.
   */
  gamma(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Vega of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vega: change in value per 1.0 absolute move in implied volatility.
   */
  vega(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Theta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Theta: change in value per year of calendar time.
   */
  theta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Domestic-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Domestic rho: change in value per 1.0 absolute move in the domestic rate.
   */
  rho(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Foreign-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Foreign rho: change in value per 1.0 absolute move in the foreign rate.
   */
  foreignRho(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Vanna of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vanna: cross sensitivity of delta to implied volatility.
   */
  vanna(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Volga of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Volga: change in vega per 1.0 absolute move in implied volatility.
   */
  volga(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Named first-order greeks produced by the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Map of greek name to value, such as `delta`, `gamma`, and `vega`.
   */
  greeks(marketJson: string, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * FX digital option handle with first-order greeks.
 */
export interface FxDigitalOptionInstrument extends FxInstrument {
  /**
   * Spot delta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot delta: change in value per unit spot.
   */
  delta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Spot gamma of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot gamma: change in delta per unit spot.
   */
  gamma(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Vega of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vega: change in value per 1.0 absolute move in implied volatility.
   */
  vega(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Theta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Theta: change in value per year of calendar time.
   */
  theta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Domestic-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Domestic rho: change in value per 1.0 absolute move in the domestic rate.
   */
  rho(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Named first-order greeks produced by the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Map of greek name to value, such as `delta`, `gamma`, and `vega`.
   */
  greeks(marketJson: string, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * FX touch option handle with first-order greeks.
 */
export interface FxTouchOptionInstrument extends FxInstrument {
  /**
   * Spot delta of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot delta: change in value per unit spot.
   */
  delta(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Spot gamma of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Spot gamma: change in delta per unit spot.
   */
  gamma(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Vega of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vega: change in value per 1.0 absolute move in implied volatility.
   */
  vega(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Domestic-rate rho of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Domestic rho: change in value per 1.0 absolute move in the domestic rate.
   */
  rho(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Named first-order greeks produced by the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Map of greek name to value, such as `delta`, `gamma`, and `vega`.
   */
  greeks(marketJson: string, asOf: string, model?: string | null): Record<string, number>;
}

/**
 * FX barrier option handle with vanna and volga in addition to touch greeks.
 */
export interface FxBarrierOptionInstrument extends FxTouchOptionInstrument {
  /**
   * Vanna of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Vanna: cross sensitivity of delta to implied volatility.
   */
  vanna(marketJson: string, asOf: string, model?: string | null): number;
  /**
   * Volga of the option under the selected model.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to select market inputs and date-dependent cashflows.
   * @param model - Optional pricing-model identifier; omit to use the instrument's default model.
   * @returns Volga: change in vega per 1.0 absolute move in implied volatility.
   */
  volga(marketJson: string, asOf: string, model?: string | null): number;
}

/**
 * Constructors and factories for typed FX instrument handles.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const spot = new valuations.fx.FxSpot({
 *   id: "EURUSD-SPOT",
 *   base_currency: "EUR",
 *   quote_currency: "USD",
 *   settlement: "2025-01-17",
 *   spot_rate: 1.1,
 *   notional: { amount: "1000000", currency: "EUR" },
 *   attributes: {},
 * });
 * console.log(JSON.parse(spot.toJson()).instrument.type);
 * spot.free();
 * ```
 */
export interface FxInstrumentConstructor<T extends FxInstrument> {
  /**
   * Construct an FX instrument from a JSON object or canonical JSON string.
   * @param spec - FX instrument specification as a JSON object or canonical JSON string.
   * @returns A typed FX instrument handle.
   */
  new (spec: FxInstrumentSpec): T;
  /**
   * Parse a `FxInstrument` value from canonical JSON.
   * @param json - Canonical FX instrument JSON accepted by `fromJson`.
   * @returns A typed FX instrument handle.
   */
  fromJson(json: string): T;
}

/**
 * Namespaced TypeScript entry points for fx calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const Spot = valuations.fx.FxSpot;
 * const spot = new Spot({
 *   id: "EURUSD-SPOT",
 *   base_currency: "EUR",
 *   quote_currency: "USD",
 *   settlement: "2025-01-17",
 *   spot_rate: 1.1,
 *   notional: { amount: "1000000", currency: "EUR" },
 *   attributes: {},
 * });
 * console.log(spot.toJson());
 * spot.free();
 * ```
 */
export interface FxNamespace {
  /**
   * Spot FX instrument constructor.
   */
  FxSpot: FxInstrumentConstructor<FxInstrument>;
  /**
   * FX forward instrument constructor.
   */
  FxForward: FxInstrumentConstructor<FxInstrument>;
  /**
   * FX swap instrument constructor.
   */
  FxSwap: FxInstrumentConstructor<FxInstrument>;
  /**
   * Non-deliverable forward constructor.
   */
  Ndf: FxInstrumentConstructor<FxInstrument>;
  /**
   * Vanilla FX option constructor.
   */
  FxOption: FxInstrumentConstructor<FxOptionInstrument>;
  /**
   * Digital FX option constructor.
   */
  FxDigitalOption: FxInstrumentConstructor<FxDigitalOptionInstrument>;
  /**
   * One-touch / no-touch FX option constructor.
   */
  FxTouchOption: FxInstrumentConstructor<FxTouchOptionInstrument>;
  /**
   * Barrier FX option constructor.
   */
  FxBarrierOption: FxInstrumentConstructor<FxBarrierOptionInstrument>;
  /**
   * FX variance-swap constructor.
   */
  FxVarianceSwap: FxInstrumentConstructor<FxInstrument>;
  /**
   * Quanto option constructor.
   */
  QuantoOption: FxInstrumentConstructor<FxOptionInstrument>;
}

// --- SABR (Stochastic Alpha Beta Rho) volatility -------------------------

/**
 * SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
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
   * @returns `true` when a SABR displacement shift is configured.
   */
  isShifted(): boolean;
}

/**
 * SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const params = new models.volatility.SabrParameters(0.2, 1.0, 0.3, -0.2);
 * console.log(params.alpha, params.rho);
 * params.free();
 * ```
 */
export interface SabrParametersConstructor {
  /**
   * Create SABR parameters from alpha, beta, nu, rho, and optional shift.
   * @returns A `SabrParameters` handle.
   * @param alpha - Positive SABR initial volatility scale parameter.
   * @param beta - SABR CEV elasticity parameter from 0 through 1.
   * @param nu - Positive SABR volatility-of-volatility parameter.
   * @param rho - Instantaneous correlation between the asset and variance shocks.
   * @param shift - Additive SABR rate shift applied to forward and strike before modelling.
   * @throws Error - Throws a JavaScript exception if `alpha` is not finite and positive, `beta` is outside `[0, 1]`, `nu` is negative or non-finite, `rho` is outside `[-1, 1]`, or a supplied `shift` is not finite and positive.
   */
  new (alpha: number, beta: number, nu: number, rho: number, shift?: number): SabrParameters;
  /**
   * Equity-standard defaults `(alpha=0.20, beta=1.0, nu=0.30, rho=-0.20)`.
   * @returns A `SabrParameters` handle.
   */
  equityDefault(): SabrParameters;
  /**
   * Rates-standard defaults `(alpha=0.02, beta=0.5, nu=0.30, rho=0.0)`.
   * @returns A `SabrParameters` handle.
   */
  ratesDefault(): SabrParameters;
}

/**
 * Hagan-2002 SABR volatility model.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 */
export interface SabrModel extends WasmOwned {
  /**
   * Black implied volatility for the given strike.
   * @returns Hagan-2002 Black implied volatility as a decimal.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param t - Time from the curve base date in years.
   * @throws Error - Throws a JavaScript exception if `t` is not positive, the forward or strike lies outside the selected shifted or unshifted SABR domain, or the Hagan expansion produces an undefined or non-finite volatility.
   */
  impliedVol(forward: number, strike: number, t: number): number;
  /**
   * Parameters used by this model.
   */
  readonly params: SabrParameters;
  /**
   * Whether the parameterization admits negative forwards.
   * @returns `true` when the SABR parameterization admits negative forwards.
   */
  supportsNegativeRates(): boolean;
}

/**
 * Hagan-2002 SABR volatility model.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const params = models.volatility.SabrParameters.equityDefault();
 * const model = new models.volatility.SabrModel(params);
 * console.log(model.impliedVol(100, 105, 1));
 * model.free();
 * params.free();
 * ```
 */
export interface SabrModelConstructor {
  /**
   * Create a Hagan-2002 SABR model from the supplied parameters.
   * @returns A `SabrModel` handle.
   * @param params - SABR parameter object containing alpha, beta, nu, rho, and optional shift.
   */
  new (params: SabrParameters): SabrModel;
}

/**
 * Butterfly and monotonicity diagnostics for a SABR smile.
 */
export interface SabrSmileArbitrageResult {
  /**
   * Whether the smile has no butterfly or calendar-spread monotonicity violations on the tested strikes.
   */
  arbitrage_free: boolean;
  /**
   * Strikes where butterfly convexity fails, with the butterfly value and severity.
   */
  butterfly_violations: Array<{
    strike: number;
    butterfly_value: number;
    severity_pct: number;
  }>;
  /**
   * Strike pairs where call prices increase with strike, violating call-price monotonicity.
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
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 */
export interface SabrSmile extends WasmOwned {
  /**
   * At-the-money implied volatility.
   * @returns ATM Black implied volatility as a decimal for this smile's `(forward, t)`.
   * @throws Error - Throws a JavaScript exception if the smile's expiry or effective forward is outside the model domain, or the ATM calculation produces an invalid volatility.
   */
  atmVol(): number;
  /**
   * Black implied volatility for the given strike.
   * @returns Black implied volatility as a decimal at `strike`.
   * @param strike - Option strike price in the same price units as the underlying.
   * @throws Error - Throws a JavaScript exception if the smile's expiry, forward, or requested `strike` is outside the model domain, the Hagan expansion fails, or no volatility is returned for the strike.
   */
  impliedVol(strike: number): number;
  /**
   * Implied volatilities for a strike grid.
   * @returns One Black implied vol per strike, in the same order as `strikes`.
   * @param strikes - Option strikes at which to evaluate the SABR volatility smile.
   * @throws Error - Throws a JavaScript exception if the smile's expiry or forward, or any supplied strike, is outside the model domain, or the Hagan expansion produces an invalid volatility.
   */
  generateSmile(strikes: number[]): Float64Array;
  /**
   * Butterfly + monotonicity arbitrage diagnostics.
   *
   * Returns a JSON object with `arbitrage_free`, `butterfly_violations`,
   * and `monotonicity_violations` arrays (snake_case keys matching the Rust
   * canonical fields and the Python binding).
   * @returns Butterfly and monotonicity diagnostics for the supplied strike grid.
   * @param strikes - Ordered option strikes used to test the calibrated smile for static arbitrage.
   * @param r - Continuously compounded risk-free rate, expressed as a decimal.
   * @param q - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @throws Error - Throws a JavaScript exception if volatility generation fails for the stored smile and supplied strikes, or the diagnostics cannot be converted to a JavaScript value.
   */
  arbitrageDiagnostics(strikes: number[], r?: number, q?: number): SabrSmileArbitrageResult;
}

/**
 * Volatility smile generator for a fixed `(forward, t)` pair.
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const params = models.volatility.SabrParameters.ratesDefault();
 * const smile = new models.volatility.SabrSmile(params, 0.03, 2);
 * console.log(smile.generateSmile([0.02, 0.03, 0.04]));
 * smile.free();
 * params.free();
 * ```
 */
export interface SabrSmileConstructor {
  /**
   * Create a SABR smile for a fixed forward and expiry.
   * @returns A `SabrSmile` handle for the supplied forward and expiry.
   * @param params - SABR parameter object containing alpha, beta, nu, rho, and optional shift.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param t - Time from the curve base date in years.
   */
  new (params: SabrParameters, forward: number, t: number): SabrSmile;
}

/**
 * SABR calibrator (Levenberg-Marquardt with beta fixed).
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 */
export interface SabrCalibrator extends WasmOwned {
  /**
   * Return a copy of this calibrator with an overridden convergence
   * tolerance, preserving all other settings (e.g. the iteration cap from
   * `highPrecision`).
   * @returns A `SabrCalibrator` handle.
   * @param tolerance - Positive finite maximum relative error of any final volatility quote; 1e-4 permits 0.01% of each quote. Invalid settings or a fit outside this budget throw during calibration.
   */
  withTolerance(tolerance: number): SabrCalibrator;
  /**
   * Return a copy of this calibrator with an overridden iteration cap,
   * preserving all other settings.
   * @returns A `SabrCalibrator` handle.
   * @param maxIterations - Positive cap on solver iterations before a non-convergence error; pair a tight tolerance with a larger budget.
   */
  withMaxIterations(maxIterations: number): SabrCalibrator;
  /**
   * Calibrate `(alpha, nu, rho)` to market vols with `beta` fixed.
   * @returns A `SabrParameters` handle.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strikes - Option strikes aligned one-for-one with market_vols.
   * @param marketVols - Market-implied annualized volatilities aligned one-for-one with strikes.
   * @param t - Time from the curve base date in years.
   * @param beta - SABR CEV elasticity parameter held fixed during calibration.
   * @throws Error - Throws a JavaScript exception if the strike and volatility lengths differ, the quote arrays are empty, the SABR inputs or fitted parameters are invalid, or the calibration solver does not converge.
   */
  calibrate(
    forward: number,
    strikes: number[],
    marketVols: number[],
    t: number,
    beta: number
  ): SabrParameters;
  /**
   * Return a copy of this calibrator with an overridden displacement policy,
   * preserving all other settings.
   * @returns A `SabrCalibrator` handle.
   * @param shift - `null`/`undefined` fits the quotes as-is; a number is a fixed additive shift in the forward's units (decimal rate or price); `"auto"` picks the smallest standardized shift (1-4%) that leaves 10bp of headroom above the most negative forward or strike, or none when every input is non-negative. The shift used is stored on the fitted `SabrParameters`.
   * @throws Error - Throws a JavaScript exception if `shift` is neither `null`, a number, nor the string `"auto"`.
   */
  withShift(shift: number | 'auto' | null | undefined): SabrCalibrator;
  /**
   * Return a copy of this calibrator with exact ATM pinning enabled or
   * disabled, preserving all other settings.
   * @returns A `SabrCalibrator` handle.
   * @param atmPinning - When `true`, alpha is solved analytically so the model reproduces the ATM volatility interpolated from the quotes exactly, and only nu and rho are fitted to the smile.
   */
  withAtmPinning(atmPinning: boolean): SabrCalibrator;
}

/**
 * SABR calibrator (Levenberg-Marquardt with beta fixed).
 *
 * Hagan SABR (2002): see docs/REFERENCES.md#hagan-2002-sabr.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const calibrator = models.volatility.SabrCalibrator.highPrecision();
 * const tighter = calibrator.withTolerance(1e-10);
 * tighter.free();
 * calibrator.free();
 * ```
 */
export interface SabrCalibratorConstructor {
  /**
   * Create a Levenberg-Marquardt SABR calibrator with default tolerances.
   * @returns A `SabrCalibrator` handle.
   */
  new (): SabrCalibrator;
  /**
   * Calibrator preset with tighter convergence tolerances.
   * @returns A `SabrCalibrator` handle.
   */
  highPrecision(): SabrCalibrator;
}

/**
 * Namespaced TypeScript entry points for structural-credit model calculations.
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const model = models.credit.mertonModelJson(100, 0.25, 60, 0.03);
 * console.log(models.credit.mertonDefaultProbability(model, 1));
 * ```
 */
export interface ModelCreditNamespace {
  /**
   * Compare hold-versus-tender economics for a distressed exchange offer.
   * Tendering is recommended only when total consideration exceeds the
   * hold-out present value by more than 2%.
   * @returns Tender total, NPV pickup, breakeven recovery, and recommendation.
   * @param oldPv - Present value of the existing claim if it is not tendered, in the caller's monetary unit.
   * @param newPv - Present value of the new instrument received on tendering, in the same unit as oldPv.
   * @param consentFee - Cash consent or early-tender fee paid to participating holders, in the same unit as oldPv.
   * @param equitySweetenerValue - Estimated value of equity or warrants attached to the new instrument, in the same unit as oldPv.
   * @param exchangeType - Canonical offer structure: par_for_par, discount, uptier, or downtier.
   * @throws Error - Throws a JavaScript exception if `exchangeType` is unrecognized, any monetary input is negative or non-finite, or the result cannot be converted to a JavaScript object.
   */
  analyzeExchangeOffer(
    oldPv: number,
    newPv: number,
    consentFee: number,
    equitySweetenerValue: number,
    exchangeType: string
  ): ExchangeOfferAnalysis;
  /**
   * Compute discount capture and leverage impact for an LME transaction.
   * @returns Cash cost, par retired, discount captured, holder impact, and optional leverage analysis.
   * @param lmeType - Canonical structure: open_market_repurchase, tender_offer, amend_and_extend, or dropdown.
   * @param notional - Outstanding face amount of the target instrument, in the caller's monetary unit; must be positive.
   * @param repurchasePricePct - Price as a fraction of par for repurchases and tenders, the extension fee for amend-and-extend, or the transferred-asset fraction for a dropdown.
   * @param optAcceptancePct - Fraction of holders participating, in [0, 1].
   * @param ebitda - EBITDA in the same unit as notional; a positive value adds the leverage_impact block, null or non-positive omits it.
   * @throws Error - Throws a JavaScript exception if `lmeType` is unrecognized, `notional` is non-positive or non-finite, `optAcceptancePct` is outside `[0, 1]`, or `repurchasePricePct` is outside the range accepted for the selected LME type: `(0, 1.5]` for repurchases and tenders, `[0, 0.1]` for amend-and-extend, and `[0, 1]` for dropdowns. It also throws if the result cannot be converted to a JavaScript object.
   */
  analyzeLme(
    lmeType: string,
    notional: number,
    repurchasePricePct: number,
    optAcceptancePct: number,
    ebitda?: number | null
  ): LmeAnalysis;
  /**
   * Build a structural Merton model JSON payload.
   * @returns Canonical Merton structural-model JSON.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal.
   * @param debtBarrier - Positive debt face value defining the structural-model default barrier.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @throws Error - Throws a JavaScript exception if `asset_value`, `asset_vol`, or `debt_barrier` is non-positive, or if the model cannot be serialized to JSON.
   */
  mertonModelJson(
    assetValue: number,
    assetVol: number,
    debtBarrier: number,
    riskFreeRate: number
  ): string;
  /**
   * Build a CreditGrades structural model JSON payload.
   * @returns Canonical CreditGrades structural-model JSON.
   * @param equityValue - Current market value of equity in the firm's monetary units.
   * @param equityVol - Annualized equity-return volatility expressed as a decimal.
   * @param totalDebt - Total debt face value in the firm's monetary units.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param barrierUncertainty - Lognormal dispersion of the CreditGrades default barrier, not a generic uncertainty score.
   * @param meanRecovery - Mean recovery rate at default expressed as a fraction from 0 through 1.
   * @throws Error - Throws a JavaScript exception if CreditGrades or Merton model validation rejects the supplied equity, volatility, debt, barrier-uncertainty, or recovery inputs, or if the model cannot be serialized to JSON.
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
   * @returns Risk-neutral default probability in `[0, 1]` over `horizon` years.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed or does not deserialize as a Merton model.
   */
  mertonDefaultProbability(modelJson: string, horizon: number): number;
  /**
   * Compute the physical-measure (Moody's KMV) default probability, the theoretical EDF, from a Merton model JSON payload.
   * @returns Physical-measure default probability in `[0, 1]` over `horizon` years.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param assetDrift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `asset_drift` is not finite, or if the model uses driftless CreditGrades dynamics.
   */
  mertonDefaultProbabilityWithDrift(modelJson: string, assetDrift: number, horizon: number): number;
  /**
   * Compute distance-to-default from a Merton model JSON payload.
   *
   * Distance-to-default is `ln(V/B)/(sigma*sqrt(T))` plus drift adjustments.
   * Lower values indicate higher default risk. This is the risk-neutral `d2`,
   * not the Moody's KMV distance-to-default.
   * @returns Distance-to-default in standard-deviation units over `horizon` years.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed or does not deserialize as a Merton model.
   */
  mertonDistanceToDefault(modelJson: string, horizon: number): number;
  /**
   * Compute the physical-measure (Moody's KMV) distance-to-default from a Merton model JSON payload.
   * @returns Physical-measure distance-to-default in standard-deviation units over `horizon` years.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param assetDrift - Expected physical total return on firm assets as a continuously compounded decimal, replacing the risk-free rate.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `asset_drift` is not finite, or if the model uses driftless CreditGrades dynamics.
   */
  mertonDistanceToDefaultWithDrift(modelJson: string, assetDrift: number, horizon: number): number;
  /**
   * Compute the Moody's KMV default point, short-term debt plus half of long-term debt, for use as a structural default barrier.
   * @returns Default point in the same monetary units as the debt inputs.
   * @param shortTermDebt - Liabilities due within one year, in the firm's monetary units.
   * @param longTermDebt - Liabilities maturing beyond one year, in the same units; half of it enters the default point.
   * @throws Error - Throws a JavaScript exception if either input is negative or non-finite, or if the resulting default point is zero.
   */
  mertonKmvDefaultPoint(shortTermDebt: number, longTermDebt: number): number;
  /**
   * Compute the zero-coupon bond credit spread (per year) from a Merton model
   * JSON payload, given an exogenous recovery rate paid at maturity.
   * @returns Zero-coupon credit spread per year as a decimal, such as `0.015` for 150 bp.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed or does not deserialize as a Merton model, `horizon` is non-finite or non-positive, or `recovery` is outside `[0, 1]`.
   */
  mertonImpliedSpread(modelJson: string, horizon: number, recovery: number): number;
  /**
   * Compute the Merton (1974) endogenous debt spread (per year) from a Merton
   * model JSON payload, where recovery is the firm's own terminal asset value.
   * @returns Endogenous debt spread per year as a decimal, such as `0.004` for 40 bp.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Maturity of the firm's debt measured in years.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `horizon` is non-positive, if the barrier type is not terminal, or if the implied debt value is non-positive.
   */
  mertonDebtSpread(modelJson: string, horizon: number): number;
  /**
   * Compute the ISDA-style CDS par spread (per year, as a decimal) implied by a Merton model's survival curve.
   * @returns CDS par spread per year as a decimal, such as `0.015` for 150 bp.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param maturity - CDS maturity in years; must be positive and finite.
   * @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `maturity` is non-positive, if `recovery` is outside `[0, 1]` or contradicts the model's CreditGrades `mean_recovery`, or if the implied survival curve cannot be bootstrapped.
   */
  mertonCdsParSpread(modelJson: string, maturity: number, recovery: number): number;
  /**
   * Build a Merton model JSON payload from observable equity inputs (KMV calibration).
   * @returns Canonical Merton model JSON calibrated from equity observables.
   * @param equityValue - Current market value of equity in the firm's monetary units.
   * @param equityVol - Annualized equity-return volatility expressed as a decimal.
   * @param totalDebt - Total debt face value used as the structural default barrier.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param payoutRate - Continuous dividend or payout yield on assets, expressed as a decimal.
   * @param maturity - Calibration horizon in years; must be positive and finite.
   * @throws Error - Throws a JavaScript exception if equity, volatility, debt, rate, or maturity inputs are invalid, or if the model cannot be serialized to JSON.
   */
  mertonFromEquityJson(
    equityValue: number,
    equityVol: number,
    totalDebt: number,
    riskFreeRate: number,
    payoutRate: number,
    maturity: number
  ): string;
  /**
   * Build a Merton model JSON payload from a target CDS par spread.
   *
   * The objective is a full ISDA-style par spread built from the model's
   * survival curve. A quote that no volatility in `[0.01, 2.0]` reproduces, or
   * one consistent with several volatilities, is rejected rather than resolved
   * arbitrarily.
   * @returns Canonical Merton model JSON calibrated to a CDS par spread.
   * @param cdsSpreadBp - Target CDS par spread in basis points.
   * @param recovery - Recovery rate at default expressed as a fraction from 0 through 1.
   * @param totalDebt - Total debt face value in the firm's monetary units.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param maturity - Calibration horizon in years; must be positive and finite.
   * @param assetValue - Assumed initial firm asset value in monetary units.
   * @param payoutRate - Continuous payout rate on assets, expressed as a decimal.
   * @throws Error - Throws a JavaScript exception if spread, recovery, debt, rate, maturity, asset value, or payout inputs are invalid, if the quote is unattainable or ambiguous, or if the model cannot be serialized to JSON.
   */
  mertonFromCdsSpreadJson(
    cdsSpreadBp: number,
    recovery: number,
    totalDebt: number,
    riskFreeRate: number,
    maturity: number,
    assetValue: number,
    payoutRate: number
  ): string;
  /**
   * Build a Merton model JSON payload calibrated to a target cumulative default probability.
   * @returns Canonical Merton model JSON calibrated to a target cumulative PD.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal; must be positive.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%. Pass the expected physical asset return to calibrate against a real-world default rate.
   * @param payoutRate - Continuous payout rate on assets, expressed as a decimal; it enters the calibration drift and is carried on the returned model.
   * @param targetPd - Target cumulative default probability in `(0, 1)`.
   * @param maturity - Calibration horizon in years; must be positive and finite.
   * @throws Error - Throws a JavaScript exception if asset value, volatility, rate, target PD, or maturity inputs are invalid, or if the model cannot be serialized to JSON.
   */
  mertonFromTargetPdJson(
    assetValue: number,
    assetVol: number,
    riskFreeRate: number,
    payoutRate: number,
    targetPd: number,
    maturity: number
  ): string;
  /**
   * Build a Merton model JSON payload with explicit barrier and asset-dynamics specifications.
   * @returns Canonical Merton model JSON with explicit barrier and asset dynamics.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal.
   * @param debtBarrier - Positive debt face value defining the structural-model default barrier.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param payoutRate - Continuous payout rate on assets, expressed as a decimal.
   * @param barrierTypeJson - Serialized `BarrierType` JSON (terminal or first-passage).
   * @param dynamicsJson - Serialized `AssetDynamics` JSON (GBM, jump-diffusion, or CreditGrades).
   * @throws Error - Throws a JavaScript exception if model inputs are invalid, if `barrier_type_json` or `dynamics_json` does not deserialize, or if the model cannot be serialized to JSON.
   */
  mertonModelWithDynamicsJson(
    assetValue: number,
    assetVol: number,
    debtBarrier: number,
    riskFreeRate: number,
    payoutRate: number,
    barrierTypeJson: string,
    dynamicsJson: string
  ): string;
  /**
   * Compute implied equity value and equity volatility from a Merton model JSON payload.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param horizon - Forward-looking model horizon measured in years.
   * @returns A `Float64Array` of length 2: `[equityValue, equityVolatility]`.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `horizon` is non-positive or non-finite, or if the inversion is numerically ill-conditioned.
   */
  mertonTryImpliedEquity(modelJson: string, horizon: number): Float64Array;
  /**
   * Bootstrap a hazard-curve JSON payload from structural default probabilities.
   * @returns Hazard-curve JSON bootstrapped from structural default probabilities.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param id - Hazard-curve identifier string.
   * @param baseDate - Valuation date in ISO-8601 form, such as `"2025-01-15"`.
   * @param tenors - Tenor grid in years as a `number[]` or `Float64Array`; entries must be positive and distinct.
   * @param recovery - Recovery rate at default expressed as a fraction from 0 through 1.
   * @param dayCount - Day-count convention the curve uses to turn dates into year fractions, such as `"act_365f"` or `"act_360"`.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if `base_date` is not a valid ISO-8601 calendar date (`YYYY-MM-DD`), if `tenors` is empty or contains non-positive values, if `recovery` is out of range or contradicts the model's CreditGrades `mean_recovery`, if `day_count` is not a recognized convention, if the implied survival curve is non-monotonic, or if the hazard curve cannot be serialized to JSON.
   */
  mertonToHazardCurveJson(
    modelJson: string,
    id: string,
    baseDate: string,
    tenors: NumericArray,
    recovery: number,
    dayCount: string
  ): string;
  /**
   * Simulate firm-asset paths and return a JSON payload with the time grid and row-major asset values.
   * @returns JSON payload with the time grid and row-major simulated asset values.
   * @param modelJson - Serialized Merton structural-credit model produced by this API's model builder.
   * @param numPaths - Number of Monte Carlo paths to simulate.
   * @param numSteps - Number of time steps per path; must be at least 1.
   * @param horizon - Simulation horizon in years; must be positive and finite.
   * @param seed - RNG seed for reproducible draws (`Pcg64Rng`).
   * @param antithetic - When `true`, use antithetic variates for variance reduction.
   * @throws Error - Throws a JavaScript exception if `model_json` is malformed, if path or step counts exceed the safe-integer range, if `num_steps` is zero, if `horizon` is non-positive or non-finite, or if the result cannot be serialized to JSON.
   */
  mertonSimulatePathsJson(
    modelJson: string,
    numPaths: number,
    numSteps: number,
    horizon: number,
    seed: bigint,
    antithetic: boolean
  ): string;
  /**
   * Evaluate a `DynamicRecoverySpec` JSON payload at a given accreted
   * notional, returning the implied recovery rate. Result is clamped to
   * `[0, base_recovery]`.
   * @returns Implied recovery rate as a fraction of par, clamped to `[0, base_recovery]`.
   * @param specJson - Serialized DynamicRecoverySpec JSON defining the notional-to-recovery mapping.
   * @param notional - Signed trade notional in the instrument's native currency units.
   * @throws Error - Throws a JavaScript exception if `spec_json` is malformed or does not deserialize as a dynamic-recovery specification.
   */
  dynamicRecoveryAtNotional(specJson: string, notional: number): number;
  /**
   * Evaluate an `EndogenousHazardSpec` JSON payload at a given leverage
   * level, returning the implied hazard rate. Floored at 0.
   * @returns Annualized hazard rate as a decimal, floored at 0.
   * @param specJson - Serialized EndogenousHazardSpec JSON defining the leverage-to-hazard mapping.
   * @param leverage - Debt-to-assets leverage ratio used by the structural credit model.
   * @throws Error - Throws a JavaScript exception if `spec_json` is malformed or does not deserialize as an endogenous-hazard specification.
   */
  endogenousHazardAtLeverage(specJson: string, leverage: number): number;
  /**
   * Convenience evaluator: hazard rate after a PIK accrual updates the
   * outstanding notional. Computes leverage = `accreted_notional / asset_value`
   * then evaluates the hazard mapping.
   * @returns Annualized hazard rate as a decimal after the PIK leverage update.
   * @param specJson - Serialized EndogenousHazardSpec JSON defining the leverage-to-hazard mapping.
   * @param accretedNotional - Outstanding notional after PIK accrual, in the debt's monetary units.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @throws Error - Throws a JavaScript exception if `spec_json` is malformed or does not deserialize as an endogenous-hazard specification.
   */
  endogenousHazardAfterPikAccrual(
    specJson: string,
    accretedNotional: number,
    assetValue: number
  ): number;
  /**
   * Build a constant dynamic-recovery spec JSON payload.
   * @returns Canonical constant dynamic-recovery specification JSON.
   * @param recovery - Recovery rate at default expressed as a fraction of par from 0 through 1.
   * @throws Error - Throws a JavaScript exception if `recovery` is outside `[0, 1]` or the specification cannot be serialized to JSON.
   */
  dynamicRecoveryConstantJson(recovery: number): string;
  /**
   * Build an endogenous hazard power-law spec JSON payload.
   * @returns Canonical endogenous-hazard power-law specification JSON.
   * @param baseHazard - Reference annual default intensity used by the leverage-to-hazard mapping.
   * @param baseLeverage - Positive reference debt-to-assets leverage ratio for the hazard mapping.
   * @param exponent - Power-law exponent in `lambda(L) = baseHazard * (L / baseLeverage)^exponent`.
   * @throws Error - Throws a JavaScript exception if `base_hazard` is negative, `base_leverage` is non-positive, or the specification cannot be serialized to JSON.
   */
  endogenousHazardPowerLawJson(baseHazard: number, baseLeverage: number, exponent: number): string;
  /**
   * Build a credit-state JSON payload for toggle-exercise decisions.
   *
   * Parameter order follows the canonical Rust `CreditState` field order
   * (and the Python binding): `hazardRate`, `distanceToDefault`, `leverage`,
   * `accretedNotional`, `couponDue`, `assetValue`.
   * @returns Canonical credit-state JSON for toggle-exercise decisions.
   * @param hazardRate - Annualized instantaneous default intensity, expressed as a decimal.
   * @param distanceToDefault - Optional distance to default, measured as standard deviations from the default point.
   * @param leverage - Debt-to-assets leverage ratio used by the structural credit model.
   * @param accretedNotional - Outstanding notional after PIK accrual, in the debt's monetary units.
   * @param couponDue - Cash coupon amount due at the toggle decision date, in debt monetary units.
   * @param assetValue - Current fair value of the firm's assets in monetary units.
   * @throws Error - Throws a JavaScript exception if the credit state cannot be serialized to JSON.
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
   * @returns Canonical threshold toggle-exercise model JSON.
   * @param variable - Credit-state variable: `"hazard_rate"`, `"distance_to_default"`, or `"leverage"`.
   * @param threshold - Threshold value in the units of the selected credit-state variable.
   * @param direction - Threshold comparison: `"above"` selects PIK above the level and `"below"` below it.
   * @throws Error - Throws a JavaScript exception if `variable` or `direction` is not a supported value, or if the model cannot be serialized to JSON.
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
   * @returns Canonical optimal toggle-exercise model JSON.
   * @param nestedPaths - Number of nested Monte Carlo paths for continuation-value estimation; must fit JavaScript's safe integer range.
   * @param equityDiscountRate - Annual equity-holder discount rate used in the nested toggle decision.
   * @param assetVol - Annualized volatility of firm-asset returns, expressed as a decimal.
   * @param riskFreeRate - Annualized risk-free rate expressed as a decimal, such as 0.05 for 5%.
   * @param horizon - Forward-looking model horizon measured in years.
   * @throws Error - Throws a JavaScript exception if `nested_paths` exceeds JavaScript's safe integer range or the model cannot be serialized to JSON.
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
 * const cds = JSON.parse(valuations.creditDerivatives.creditDefaultSwapExampleJson());
 * console.log(cds.instrument.type);
 * ```
 */
export interface CreditDerivativesNamespace {
  /**
   * Example tagged `CreditDefaultSwap` instrument JSON.
   * @returns Example tagged `CreditDefaultSwap` instrument JSON.
   * @throws Error - Throws a JavaScript exception if the example envelope cannot be serialized to JSON.
   */
  creditDefaultSwapExampleJson(): string;
  /**
   * Example tagged `CDSIndex` instrument JSON.
   * @returns Example tagged `CDSIndex` instrument JSON.
   * @throws Error - Throws a JavaScript exception if the example envelope cannot be serialized to JSON.
   */
  cdsIndexExampleJson(): string;
  /**
   * Example tagged `CDSTranche` instrument JSON.
   * @returns Example tagged `CDSTranche` instrument JSON.
   * @throws Error - Throws a JavaScript exception if the example envelope cannot be serialized to JSON.
   */
  cdsTrancheExampleJson(): string;
  /**
   * Example tagged `CDSOption` instrument JSON.
   * @returns Example tagged `CDSOption` instrument JSON.
   * @throws Error - Throws a JavaScript exception if the example option cannot be constructed or its envelope cannot be serialized to JSON.
   */
  cdsOptionExampleJson(): string;
}

/**
 * JSON object or pre-serialized JSON accepted by composite façade methods.
 */
export type CompositeJsonInput = Record<string, unknown> | string;

/**
 * Primitive execution delta emitted by composite initialization or rebalance.
 */
export interface CompositeTrade {
  /**
   * Primitive instrument identifier.
   */
  instrument_id: string;
  /**
   * Canonical primitive instrument discriminator.
   */
  instrument_type: string;
  /**
   * Signed primitive quantity change.
   */
  quantity_delta: number;
}

/**
 * Canonical resolved composite envelope plus primitive trade deltas.
 */
export interface CompositeRebalanceResult {
  /**
   * Canonical `finstack_quant.instrument/1` envelope accepted by `priceInstrument`.
   */
  instrument: Record<string, unknown>;
  /**
   * Net primitive quantity deltas required to establish the returned state.
   */
  trades: CompositeTrade[];
}

/**
 * One primitive exposure path in a resolved composite.
 */
export interface CompositePrimitivePath {
  /**
   * Composite/leg identifiers from root to primitive.
   */
  path: string[];
  /**
   * Primitive instrument identifier.
   */
  instrument_id: string;
  /**
   * Canonical primitive instrument discriminator.
   */
  instrument_type: string;
  /**
   * Signed frozen primitive quantity.
   */
  quantity: number;
  /**
   * Signed value in composite reporting currency.
   */
  value: MoneyValue;
  /**
   * Additive risk amounts keyed by canonical metric identifier.
   */
  measures: Record<string, number>;
}

/**
 * Net and gross concentration for one primitive identifier.
 */
export interface CompositePrimitiveAggregate {
  /**
   * Primitive instrument identifier.
   */
  instrument_id: string;
  /**
   * Canonical primitive instrument discriminator.
   */
  instrument_type: string;
  /**
   * Algebraic primitive quantity across paths.
   */
  net_quantity: number;
  /**
   * Sum of absolute path quantities.
   */
  gross_quantity: number;
  /**
   * Algebraic value in composite reporting currency.
   */
  net_value: MoneyValue;
  /**
   * Sum of absolute path values.
   */
  gross_value: MoneyValue;
  /**
   * Algebraic additive risk by metric.
   */
  net_measures: Record<string, number>;
  /**
   * Sum of absolute additive risk by metric.
   */
  gross_measures: Record<string, number>;
}

/**
 * Recursive primitive exposure report for one resolved composite.
 */
export interface CompositeExposureReport {
  /**
   * ISO reporting-currency code.
   */
  reporting_currency: string;
  /**
   * Path-level primitive exposures before overlap netting.
   */
  paths: CompositePrimitivePath[];
  /**
   * Net and gross aggregates ordered by primitive identifier.
   */
  aggregates: CompositePrimitiveAggregate[];
}

/**
 * One dated composite total-return and rebalance observation.
 *
 * The first row reports zero cashflows, P&L, and period return, with
 * `return_index` equal to 100. Later rows use `period_return = pnl / capital`
 * and chain `return_index *= 1 + period_return`. A scheduled rebalance is
 * close-effective: this row still uses pre-trade holdings, and the next
 * interval opens at the post-trade financed value.
 */
export interface CompositeHistoryRow {
  /**
   * ISO-8601 observation date of this close.
   */
  date: string;
  /**
   * Pre-rebalance close value in the composite reporting currency.
   */
  value: MoneyValue;
  /**
   * Signed primitive cashflows on `(previousDate, date]`; zero on the first row.
   */
  cashflows: MoneyValue;
  /**
   * `Δvalue + cashflows` versus the prior interval's financed opening value;
   * zero on the first row. External rebalance financing is excluded.
   */
  pnl: MoneyValue;
  /**
   * `pnl / capital` for one composite unit; zero on the first row.
   */
  period_return: number;
  /**
   * Chained total-return index, initialized to 100 on the first row.
   */
  return_index: number;
  /**
   * Effective date of quantities held into this close.
   */
  held_state_effective_date: string;
  /**
   * New close-effective state date, when a rebalance occurred.
   */
  next_state_effective_date?: string | null;
  /**
   * Primitive exposures under the held (pre-rebalance) state.
   */
  exposures: CompositeExposureReport;
  /**
   * Primitive quantity deltas emitted by a close-of-period rebalance.
   */
  rebalance_trades: CompositeTrade[];
}

/**
 * Composite-instrument construction, decomposition, execution, and history.
 *
 * Pricing uses frozen quantities. Only `initialize` and `rebalance` calculate
 * a new state. There is no `initializeFixed` export; `initialize` also
 * resolves `fixed_quantity` without history. Period return is `pnl / capital`.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * const fixed = { kind: "fixed_quantity" };
 * console.log(fixed.kind === "fixed_quantity");
 * ```
 */
export interface CompositeNamespace {
  /**
   * Resolve a bare specification into an immutable priceable envelope.
   *
   * Fixed-quantity specs do not require `history`. Volatility weighting
   * requires strictly increasing observations that end on `asOf`.
   * @param spec - Bare canonical `CompositeSpec` object or JSON string.
   * @param market - Complete market-context object or JSON string at `asOf`.
   * @param asOf - ISO-8601 state effective date; no later history is permitted.
   * @param history - Optional chronological market-observation array or JSON for volatility or expression inputs.
   * @returns Canonical resolved envelope plus primitive establishment trades.
   * @throws Error - Throws when JSON, dates, specifications, market inputs, history, metrics, notionals, or resolved quantities are invalid.
   */
  initialize(
    spec: CompositeJsonInput,
    market: CompositeJsonInput,
    asOf: string,
    history?: Record<string, unknown>[] | string
  ): CompositeRebalanceResult;
  /**
   * Explicitly resolve a distinct state without mutating prior quantities.
   *
   * Trades are net primitive quantity deltas from `instrument` to the new state.
   * @param instrument - Canonical resolved composite envelope object or JSON.
   * @param market - Complete rebalance-date market-context object or JSON.
   * @param asOf - ISO-8601 effective date for the new state.
   * @param history - Optional chronological market-observation array or JSON; required for volatility weighting and must end on `asOf`.
   * @returns New canonical envelope plus net primitive quantity deltas.
   * @throws Error - Throws for malformed inputs, invalid history, missing market data, or quantity-resolution failures.
   */
  rebalance(
    instrument: CompositeJsonInput,
    market: CompositeJsonInput,
    asOf: string,
    history?: Record<string, unknown>[] | string
  ): CompositeRebalanceResult;
  /**
   * Price frozen primitive paths and aggregate net/gross value and risk.
   *
   * Only additive metrics are accepted. Amounts are converted to the
   * composite reporting currency on `asOf`.
   * @param instrument - Canonical resolved composite envelope object or JSON.
   * @param market - Complete valuation and FX context object or JSON.
   * @param asOf - ISO-8601 valuation date used for prices, metrics, and FX.
   * @param metrics - Optional additive metric identifiers; omit or pass `[]` to report value only.
   * @returns Path-level primitive exposures and net/gross aggregates.
   * @throws Error - Throws for non-additive metrics, invalid state, missing market data, FX failures, or primitive pricing failures.
   */
  primitiveExposures(
    instrument: CompositeJsonInput,
    market: CompositeJsonInput,
    asOf: string,
    metrics?: string[]
  ): CompositeExposureReport;
  /**
   * Flatten target holdings or a transition into executable primitive deltas.
   * @param instrument - Canonical target resolved composite envelope.
   * @param previous - Optional prior resolved envelope; omit for establishment trades.
   * @returns Net primitive quantity-delta array.
   * @throws Error - Throws for malformed envelopes, invalid frozen states, or conflicting primitive definitions.
   */
  executionTrades(instrument: CompositeJsonInput, previous?: CompositeJsonInput): CompositeTrade[];
  /**
   * Initialize on the first supplied snapshot and calculate dated history.
   *
   * Warmup observations feed weighting only. The first output row has
   * `return_index = 100` and zero P&L. Scheduled rebalances are close-effective.
   * @param spec - Bare canonical `CompositeSpec` object or JSON string.
   * @param observations - Non-empty strictly increasing complete observation array or JSON.
   * @param warmup - Optional complete observations strictly before the output period.
   * @param metrics - Optional additive primitive metrics reported on every row; omit or pass `[]` for value only.
   * @returns Chronological value, cashflow, P&L, return, index, exposure, state, and trade rows.
   * @throws Error - Throws for empty, duplicate, unordered, or overlapping observations and any initialization, pricing, FX, or rebalance failure.
   */
  historyFromSpec(
    spec: CompositeJsonInput,
    observations: Record<string, unknown>[] | string,
    warmup?: Record<string, unknown>[] | string,
    metrics?: string[]
  ): CompositeHistoryRow[];
  /**
   * Calculate dated history from an already-resolved initial state.
   *
   * The initial effective date must be on or before the first observation.
   * Period return is `pnl / capital`; `return_index` starts at 100.
   * @param instrument - Canonical resolved composite envelope object or JSON.
   * @param observations - Non-empty strictly increasing complete observation array or JSON.
   * @param metrics - Optional additive primitive metrics reported on every row; omit or pass `[]` for value only.
   * @returns Chronological composite history rows.
   * @throws Error - Throws for invalid states or observations, missing inputs, or valuation and rebalance failures.
   */
  history(
    instrument: CompositeJsonInput,
    observations: Record<string, unknown>[] | string,
    metrics?: string[]
  ): CompositeHistoryRow[];
}

/**
 * Dynamic Nelson-Siegel and statistical yield-curve models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const yields = models.rates.dtsm.nelsonSiegelYields(
 *   0.7308, 0.03, -0.01, 0.005, [1, 5, 10]
 * );
 * ```
 */
export interface DtsmNamespace {
  /**
   * Evaluate the static Nelson-Siegel (1987) yield curve for one factor triple.
   *
   * @example
   * ```typescript
   * import init, { models } from "finstack-quant-wasm";
   * await init();
   * const yields = models.rates.dtsm.nelsonSiegelYields(
   *   0.7308, 0.03, -0.01, 0.005, [1, 5, 10]
   * );
   * ```
   * @param lambda - Exponential decay parameter for tenors in years; must be finite and greater than zero (0.7308 is the years-equivalent of Diebold-Li's 0.0609 months value).
   * @param level - Nelson-Siegel beta1, the long-run level factor in decimal yield units such as 0.06 for 6%.
   * @param slope - Nelson-Siegel beta2, the slope factor (negative of the short-minus-long spread) in decimal yield units.
   * @param curvature - Nelson-Siegel beta3, the hump-shaped curvature factor in decimal yield units.
   * @param tenors - Maturities in years, each finite and non-negative; output order matches this array.
   * @returns One decimal yield per tenor, in the same order as `tenors`.
   * @throws Error - Throws a JavaScript exception if `lambda` is non-finite or non-positive, any factor loading is non-finite, or any tenor is non-finite or negative.
   */
  nelsonSiegelYields(
    lambda: number,
    level: number,
    slope: number,
    curvature: number,
    tenors: NumericArray
  ): Float64Array;
}

/**
 * Product-independent interest-rate models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * const yields = models.rates.dtsm.nelsonSiegelYields(
 *   0.7308, 0.03, -0.01, 0.005, [1, 5, 10]
 * );
 * ```
 */
export interface RatesNamespace {
  /**
   * Dynamic term-structure models.
   */
  dtsm: DtsmNamespace;
}

/**
 * Product-independent volatility models and evaluators.
 *
 * Core owns the serializable surface, cube, and FX-delta artifacts. This
 * namespace owns SABR behavior and all currently bound artifact evaluation.
 * @example
 * ```typescript
 * import init, { core, models } from "finstack-quant-wasm";
 * await init();
 * const cube = new core.VolCube(
 *   "USD-SWAPTION", [1], [5], [0.03, 0.5, -0.2, 0.4, Number.NaN], [0.03]
 * );
 * console.log(models.volatility.getCubeVol(cube, 1, 5, 0.03));
 * cube.free();
 * ```
 */
export interface VolatilityNamespace {
  /**
   * Validated SABR parameters.
   */
  SabrParameters: SabrParametersConstructor;
  /**
   * Hagan-2002 SABR volatility model.
   */
  SabrModel: SabrModelConstructor;
  /**
   * SABR smile at a fixed forward and expiry.
   */
  SabrSmile: SabrSmileConstructor;
  /**
   * Levenberg-Marquardt SABR calibrator with fixed beta.
   */
  SabrCalibrator: SabrCalibratorConstructor;
  /**
   * Evaluate checked Black/lognormal volatility from a data-only SABR cube.
   * @returns Annualized Black volatility as a decimal.
   * @param cube - Structurally validated data-only volatility cube.
   * @param expiry - Positive option expiry in years within the cube grid.
   * @param tenor - Positive underlying tenor in years within the cube grid.
   * @param strike - Finite strike in the same rate units as the stored forwards.
   * @throws Error - Throws a JavaScript exception for out-of-grid coordinates or invalid SABR model inputs.
   */
  getCubeVol(cube: VolCube, expiry: number, tenor: number, strike: number): number;
  /**
   * Evaluate Black/lognormal cube volatility with flat coordinate clamping.
   * @returns Annualized Black volatility, or `NaN` when undefined.
   * @throws Never; invalid inputs are represented by `NaN`.
   * @param cube - Structurally validated data-only volatility cube.
   * @param expiry - Finite option expiry in years; clamped to the stored grid.
   * @param tenor - Finite underlying tenor in years; clamped to the stored grid.
   * @param strike - Finite strike in the same rate units as the stored forwards.
   */
  getCubeVolClamped(cube: VolCube, expiry: number, tenor: number, strike: number): number;
  /**
   * Evaluate checked normal/Bachelier volatility from a data-only SABR cube.
   * @returns Annualized normal volatility in absolute rate units.
   * @param cube - Structurally validated data-only volatility cube.
   * @param expiry - Positive option expiry in years within the cube grid.
   * @param tenor - Positive underlying tenor in years within the cube grid.
   * @param strike - Finite strike in the same rate units as the stored forwards.
   * @throws Error - Throws a JavaScript exception for out-of-grid coordinates, an invalid shifted-SABR domain, or a failed normal-volatility expansion.
   */
  getCubeNormalVol(cube: VolCube, expiry: number, tenor: number, strike: number): number;
  /**
   * Evaluate normal/Bachelier cube volatility with coordinate clamping.
   * @returns Annualized normal volatility, or `NaN` when undefined.
   * @throws Never; invalid inputs are represented by `NaN`.
   * @param cube - Structurally validated data-only volatility cube.
   * @param expiry - Finite option expiry in years; clamped to the stored grid.
   * @param tenor - Finite underlying tenor in years; clamped to the stored grid.
   * @param strike - Finite strike in the same rate units as the stored forwards.
   */
  getCubeNormalVolClamped(cube: VolCube, expiry: number, tenor: number, strike: number): number;
  /**
   * Return ATM, 25-delta put, and 25-delta call vols at a stored FX expiry.
   * @returns Three annualized decimal volatilities in ATM, put, call order.
   * @param surface - Structurally validated data-only FX delta surface.
   * @param expiryIndex - Zero-based stored expiry index.
   * @throws Error - Throws a JavaScript exception when `expiry_index` is outside the surface.
   */
  getFxDeltaPillarVols(surface: FxDeltaVolSurface, expiryIndex: number): Float64Array;
  /**
   * Evaluate an FX delta-quoted surface at an expiry, strike, and forward.
   * @returns Annualized Black volatility as a decimal.
   * @param surface - Structurally validated data-only FX delta surface.
   * @param expiry - Positive option expiry in years.
   * @param strike - Positive strike in the FX quote currency.
   * @param forward - Positive FX forward in quote currency per base currency.
   * @throws Error - Throws a JavaScript exception for invalid coordinates or a non-positive reconstructed wing volatility.
   */
  getFxDeltaVol(
    surface: FxDeltaVolSurface,
    expiry: number,
    strike: number,
    forward: number
  ): number;
  /**
   * Convert premium-unadjusted forward call delta to strike.
   * @returns Strike in the same units as `forward`.
   * @throws Never; invalid inputs propagate IEEE non-finite results.
   * @param delta - Forward call delta as a decimal probability in `(0, 1)`.
   * @param forward - Positive forward in the same units as the returned strike.
   * @param volatility - Positive annualized Black volatility as a decimal.
   * @param expiry - Positive option expiry in years.
   */
  deltaToStrike(delta: number, forward: number, volatility: number, expiry: number): number;
  /**
   * Convert strike to premium-unadjusted forward call delta.
   * @returns Forward call delta as a decimal probability.
   * @throws Never; invalid inputs propagate IEEE non-finite results.
   * @param strike - Positive strike in the same units as `forward`.
   * @param forward - Positive forward in the same units as `strike`.
   * @param volatility - Positive annualized Black volatility as a decimal.
   * @param expiry - Positive option expiry in years.
   */
  strikeToDelta(strike: number, forward: number, volatility: number, expiry: number): number;
  /**
   * Convert an ATM volatility quote between normal, lognormal and shifted-lognormal conventions.
   * @returns Volatility in the target convention (decimal Black vol, or absolute normal vol).
   * @param vol - Input volatility in the source convention: decimal Black vol for `"lognormal"` / shifted-lognormal, absolute vol in the forward's rate units for `"normal"`. Must be positive.
   * @param fromConvention - `"normal"`, `"lognormal"`, or `{"shifted_lognormal": {"shift": s}}` (serde form of `VolatilityConvention`).
   * @param toConvention - Target convention in the same encoding.
   * @param forwardRate - ATM forward rate or price; must satisfy the target convention's domain.
   * @param timeToExpiry - Time to expiry in years (non-negative).
   * @throws Error - Throws a JavaScript exception if a convention cannot be decoded, an input is outside its domain, or the price-matching solver fails to converge.
   */
  convertAtmVolatility(
    vol: number,
    fromConvention: VolatilityConvention,
    toConvention: VolatilityConvention,
    forwardRate: number,
    timeToExpiry: number
  ): number;
  /**
   * Calibrate Gatheral SVI parameters to a market smile.
   * @returns Validated SVI parameters `{ a, b, rho, m, sigma }`.
   * @param strikes - Positive strikes (at least five).
   * @param vols - Black implied vols (decimal) aligned one-for-one with `strikes`.
   * @param forward - Positive forward at `expiry`.
   * @param expiry - Positive time to expiry in years.
   * @throws Error - Throws a JavaScript exception if lengths differ, fewer than five quotes are supplied, an input is outside its domain, the optimizer fails to converge, or the fit violates the SVI no-arbitrage conditions.
   */
  calibrateSvi(strikes: number[], vols: number[], forward: number, expiry: number): SviParams;
  /**
   * Black implied volatility from SVI parameters at log-moneyness `k = ln(K / F)`.
   * @returns Annualized Black volatility as a decimal.
   * @param params - SVI parameter object `{a, b, rho, m, sigma}` (validated on decode).
   * @param k - Log-moneyness `ln(K / F)`.
   * @param t - Positive time to expiry in years.
   * @throws Error - Throws a JavaScript exception if `params` fails validation, `t` is not positive, or the total variance at `k` is negative.
   */
  sviImpliedVol(params: SviParams, k: number, t: number): number;
}

/**
 * Volatility quoting convention (serde form of the Rust `VolatilityConvention` enum).
 */
export type VolatilityConvention =
  | 'normal'
  | 'lognormal'
  | { shifted_lognormal: { shift: number } };

/**
 * Gatheral SVI total-variance parameters `w(k) = a + b (rho (k - m) + sqrt((k - m)^2 + sigma^2))`.
 */
export interface SviParams {
  /**
   * Overall total-variance level.
   */
  a: number;
  /**
   * Wing slope (non-negative).
   */
  b: number;
  /**
   * Rotation / asymmetry in `(-1, 1)`.
   */
  rho: number;
  /**
   * Log-moneyness translation of the minimum-variance point.
   */
  m: number;
  /**
   * Vertex smoothing (positive).
   */
  sigma: number;
}

/**
 * Product-independent liquidity risk and market-impact models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * console.log(models.liquidity.daysToLiquidate(1_000_000, 250_000, 0.20));
 * ```
 */
export interface LiquidityNamespace {
  /**
   * Estimate Roll effective spread from an ordered return series.
   * @param returnsJson - JSON array of decimal returns in time order.
   * @returns Effective spread in return units, or `undefined` when it cannot be estimated.
   * @throws Error - Throws a JavaScript exception if `returnsJson` is malformed or is not a numeric array. Invalid estimator samples return `undefined`.
   */
  rollEffectiveSpread(returnsJson: string): number | undefined;
  /**
   * Compute Amihud illiquidity from aligned returns and volumes.
   * @param returnsJson - JSON array of decimal returns in time order.
   * @param volumesJson - JSON array of positive volumes aligned with the returns.
   * @returns Mean absolute return per unit volume, or `undefined` for an invalid sample.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed or is not a numeric array. Invalid estimator samples return `undefined`.
   */
  amihudIlliquidity(returnsJson: string, volumesJson: string): number | undefined;
  /**
   * Calculate the trading days required to liquidate a position.
   * @param positionQuantity - Shares or contracts to liquidate; the absolute value is used.
   * @param adv - Average daily volume in the same quantity units.
   * @param participationRate - Fraction of ADV available for execution each trading day.
   * @returns Liquidation horizon in trading days, or infinity for non-positive capacity.
   */
  daysToLiquidate(positionQuantity: number, adv: number, participationRate: number): number;
  /**
   * Classify a liquidation horizon using the default model thresholds.
   * @param daysToLiquidate - Estimated unwind horizon in trading days.
   * @returns One of `tier1` through `tier5`, with Tier 1 the most liquid.
   */
  liquidityTier(daysToLiquidate: number): string;
  /**
   * Compute Bangia liquidity-adjusted VaR under the loss-sign convention.
   * @param spreadMean - Finite non-negative mean relative bid-ask spread as a decimal.
   * @param spreadVol - Finite non-negative volatility of the relative spread.
   * @param confidence - Confidence level strictly between 0.5 and 1.
   * @param positionValue - Finite current market value; only its magnitude is used.
   * @returns An object containing `var`, `spread_cost`, `lvar`, and `lvar_ratio`.
   * @throws Error - Throws a JavaScript exception if an input violates the stated finiteness, sign, or range contract, or if the result cannot be converted.
   * @param varValue - Loss-convention VaR in the same units as `positionValue`; must be non-positive.
   */
  lvarBangia(
    varValue: number,
    spreadMean: number,
    spreadVol: number,
    confidence: number,
    positionValue: number
  ): LvarBangiaResult;
  /**
   * Estimate uniform Almgren-Chriss execution-impact components.
   * @param positionSize - Finite signed quantity in shares or contracts.
   * @param avgDailyVolume - Positive finite ADV in matching quantity units.
   * @param volatility - Positive finite daily volatility as a decimal.
   * @param executionHorizonDays - Positive finite execution horizon in trading days.
   * @param permanentImpactCoef - Non-negative finite multiplier on permanent impact.
   * @param temporaryImpactCoef - Positive finite multiplier on temporary impact.
   * @param referencePrice - Optional positive finite price for notional and basis-point scaling.
   * @returns Permanent, temporary, total, basis-point, and execution-risk impact fields.
   * @throws Error - Throws a JavaScript exception if an input violates the stated finiteness, sign, or range contract, calculation fails, or conversion fails.
   */
  almgrenChrissImpact(
    positionSize: number,
    avgDailyVolume: number,
    volatility: number,
    executionHorizonDays: number,
    permanentImpactCoef: number,
    temporaryImpactCoef: number,
    referencePrice?: number | null
  ): ImpactEstimate;
  /**
   * Estimate price-space Kyle lambda using an Amihud-ratio proxy.
   *
   * Argument order matches `amihudIlliquidity`: returns first, then volumes.
   * @param returnsJson - JSON array of decimal returns in time order.
   * @param volumesJson - JSON array of positive volume observations aligned with the returns.
   * @param referencePrice - Positive price per share or contract.
   * @returns Estimated price-space impact coefficient, or `undefined` for invalid inputs.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed or is not a numeric array. Invalid estimator samples return `undefined`.
   */
  kyleLambda(returnsJson: string, volumesJson: string, referencePrice: number): number | undefined;
}

/**
 * Black-Scholes / Garman-Kohlhagen Greeks (canonical Rust `BsGreeks` fields).
 * `vega`, `rho_r`, `rho_q` are per 1% move; `theta` is per day.
 */
export interface BsGreeks {
  /**
   * Spot delta per unit of underlying.
   */
  delta: number;
  /**
   * Gamma per unit of underlying.
   */
  gamma: number;
  /**
   * Vega per 1% (0.01) move in volatility.
   */
  vega: number;
  /**
   * Theta per day under the `thetaDays` basis.
   */
  theta: number;
  /**
   * Rho to the domestic / risk-free rate per 1% move.
   */
  rho_r: number;
  /**
   * Rho to the dividend yield / foreign rate per 1% move.
   */
  rho_q: number;
}

/**
 * Undiscounted forward-measure Greeks returned by `black76Greeks` / `bachelierGreeks`.
 */
export interface ForwardGreeks {
  /**
   * First derivative of undiscounted option value with respect to the forward level.
   */
  delta: number;
  /**
   * Second derivative of undiscounted option value with respect to the forward level.
   */
  gamma: number;
  /**
   * Vega per unit (1.0) change in the model volatility.
   */
  vega: number;
}

/**
 * Namespaced TypeScript entry points for reusable quantitative models.
 *
 * @example
 * ```typescript
 * import init, { models } from "finstack-quant-wasm";
 * await init();
 * console.log(models.bsPrice(100, 100, 0.03, 0, 0.2, 1, true));
 * ```
 */
export interface ModelsNamespace {
  /**
   * Factor definitions, covariance, matching, and credit-factor calibration.
   */
  factor: FactorNamespace;
  /**
   * Product-independent liquidity risk and market-impact models.
   */
  liquidity: LiquidityNamespace;
  /**
   * Monte Carlo pricing engines.
   */
  monteCarlo: MonteCarloNamespace;
  /**
   * Structural-credit models and toggle-exercise helpers.
   */
  credit: ModelCreditNamespace;
  /**
   * Copula, recovery, and credit-correlation model infrastructure.
   */
  correlation: CorrelationNamespace;
  /**
   * Product-independent interest-rate models.
   */
  rates: RatesNamespace;
  /**
   * Product-independent volatility models and evaluators.
   */
  volatility: VolatilityNamespace;
  /**
   * Per-unit Black-Scholes / Garman-Kohlhagen price of a European option.
   *
   * Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
   * Merton (1973): see docs/REFERENCES.md#merton-1973.
   * Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
   *
   * @example
   * ```javascript
   * import init, { models } from "finstack-quant-wasm";
   * await init();
   * const price = models.bsPrice(
   *   100,    // spot
   *   100,    // strike (ATM)
   *   0.05,   // rate = 5%
   *   0.0,    // divYield = 0
   *   0.20,   // vol = 20%
   *   1.0,    // expiry = 1 year
   *   true,   // call
   * );
   * // price ≈ 10.45
   * ```
   *
   * @param spot - Spot price of the underlying.
   * @param strike - Strike of the option.
   * @param rate - Risk-free rate, **decimal** continuously compounded (e.g. `0.05` for 5%).
   * @param divYield - Continuous dividend yield (or foreign rate for FX), **decimal** continuously compounded.
   * @param vol - Annualized volatility, **decimal** (e.g. `0.20` for 20%).
   * @param expiry - Time to expiry in **years**.
   * @param isCall - `true` for a call, `false` for a put.
   * @returns Per-unit option price.
   * @throws If spot or strike is non-positive, volatility or expiry is negative, any numerical input is non-finite, or discounted legs or price overflow.
   */
  bsPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    isCall: boolean
  ): number;
  /**
   * Vanilla option payoff at expiry: `max(±(spot - strike), 0)`.
   *
   * @example
   * ```javascript
   * import init, { models } from "finstack-quant-wasm";
   * await init();
   * const payoff = models.vanillaExpiryPayoff(110, 100, true);
   * // payoff === 10
   * ```
   *
   * @param spot - Underlying level at expiry, in the same price units as `strike`. Must be finite and non-negative; zero spot is allowed.
   * @param strike - Exercise price; must be finite and strictly positive.
   * @param isCall - `true` for a call (`max(spot - strike, 0)`), `false` for a put (`max(strike - spot, 0)`).
   * @returns Undiscounted expiry payoff in the same units as `spot` and `strike`.
   * @throws If `spot` is non-finite or negative, or `strike` is non-finite or not strictly positive.
   */
  vanillaExpiryPayoff(spot: number, strike: number, isCall: boolean): number;
  /**
   * Black-Scholes / Garman-Kohlhagen Greeks as a `{delta, gamma, vega, theta, rho_r, rho_q}` object.
   *
   * Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
   * Merton (1973): see docs/REFERENCES.md#merton-1973.
   * Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
   *
   * @example
   * ```javascript
   * const g = models.bsGreeks(100, 100, 0.05, 0.0, 0.20, 1.0, true);
   * // g.delta ≈ 0.64, g.gamma ≈ 0.019, g.vega ≈ 0.38 (per 1% vol)
   * ```
   * @param spot - Spot price of the underlying.
   * @param strike - Strike of the option.
   * @param rate - Risk-free rate, **decimal** continuously compounded.
   * @param divYield - Dividend yield (or foreign rate for FX), **decimal** continuously compounded.
   * @param vol - Annualized volatility, **decimal**; must be positive.
   * @param expiry - Time to expiry in **years**; must be positive.
   * @param isCall - `true` for a call, `false` for a put.
   * @param thetaDays - Day-count denominator for theta. Default `365`. Pass `252` for trading-day theta.
   * @returns Object `{ delta, gamma, vega, theta, rho_r, rho_q }` (snake_case keys matching the Rust/Python canonical `BsGreeks` fields). `vega` and both rho values are **per 1% move**; `theta` is **per day** under `thetaDays`.
   * @throws If serialization to JS fails (should not happen on valid inputs).
   */
  bsGreeks(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    isCall: boolean,
    thetaDays?: number
  ): BsGreeks;
  /**
   * Solve for Black-Scholes / Garman-Kohlhagen implied volatility.
   *
   * Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
   * Merton (1973): see docs/REFERENCES.md#merton-1973.
   * Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
   *
   * @example
   * ```javascript
   * const iv = models.bsImpliedVol(100, 100, 0.05, 0.0, 1.0, 10.45, true);
   * // iv ≈ 0.20
   * ```
   * @param spot - Spot price of the underlying.
   * @param strike - Strike of the option.
   * @param rate - Risk-free rate, **decimal** continuously compounded.
   * @param divYield - Dividend yield, **decimal** continuously compounded.
   * @param expiry - Time to expiry in **years**; must be positive.
   * @param price - Observed option price (per unit).
   * @param isCall - `true` for a call, `false` for a put.
   * @returns Annualized implied volatility, **decimal** (e.g. `0.20`).
   * @throws If `expiry` is not positive, `price` is below intrinsic value, above the no-arbitrage upper bound, or the solver fails to converge.
   */
  bsImpliedVol(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    expiry: number,
    price: number,
    isCall: boolean
  ): number;
  /**
   * Solve for Black-76 (forward-based) implied volatility.
   *
   * Black (1976): see docs/REFERENCES.md#black-1976.
   * @returns Annualized Black-76 implied volatility as a decimal.
   * @param forward - Forward price or rate in the same quote convention as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param df - Discount factor from valuation to expiry, expressed as a positive decimal.
   * @param expiry - Time to expiry in years; must be positive.
   * @param price - Observed option price in the same units as the forward.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a JavaScript exception if an input is non-finite; `expiry`, `forward`, `strike`, `df`, or `price` is not positive; the price is not above intrinsic value or cannot be bracketed; or the implied-volatility solver does not converge.
   */
  black76ImpliedVol(
    forward: number,
    strike: number,
    df: number,
    expiry: number,
    price: number,
    isCall: boolean
  ): number;
  /**
   * Black-76 per-unit price of a European option on a forward: `df * Black(F, K, vol, expiry)`.
   *
   * Black (1976): see docs/REFERENCES.md#black-1976.
   * @returns Discounted per-unit option price in the units of `forward`.
   * @param forward - Forward price or rate at expiry.
   * @param strike - Strike in the same units as `forward`.
   * @param df - Discount factor from valuation to expiry (positive decimal).
   * @param expiry - Time to expiry in years.
   * @param vol - Annualized lognormal (Black) volatility, decimal.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a JavaScript exception if the inputs produce a non-finite price.
   */
  black76Price(
    forward: number,
    strike: number,
    df: number,
    expiry: number,
    vol: number,
    isCall: boolean
  ): number;
  /**
   * Black-76 undiscounted forward Greeks `{ delta, gamma, vega }`.
   *
   * `delta` / `gamma` are with respect to the forward; `vega` is per unit (1.0) change in `vol`.
   * Black (1976): see docs/REFERENCES.md#black-1976.
   * @returns Object `{ delta, gamma, vega }`.
   * @param forward - Forward price or rate at expiry.
   * @param strike - Strike in the same units as `forward`.
   * @param expiry - Time to expiry in years.
   * @param vol - Annualized lognormal (Black) volatility, decimal.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a JavaScript exception if any Greek is non-finite.
   */
  black76Greeks(
    forward: number,
    strike: number,
    expiry: number,
    vol: number,
    isCall: boolean
  ): ForwardGreeks;
  /**
   * Bachelier (normal-model) undiscounted per-unit option price.
   *
   * Bachelier (1900): see docs/REFERENCES.md#bachelier-1900.
   * @returns Undiscounted per-unit option value in the units of `forward`.
   * @param forward - Forward price or rate at expiry (may be negative).
   * @param strike - Strike in the same units as `forward`.
   * @param normalVol - Annualized **absolute** (normal) volatility in the units of `forward` (e.g. `0.0075` for 75 bp on decimal rates).
   * @param expiry - Time to expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a JavaScript exception if the inputs produce a non-finite price.
   */
  bachelierPrice(
    forward: number,
    strike: number,
    normalVol: number,
    expiry: number,
    isCall: boolean
  ): number;
  /**
   * Bachelier (normal-model) undiscounted forward Greeks `{ delta, gamma, vega }`.
   *
   * `vega` is per unit (1.0) change in `normalVol` (absolute units).
   * Bachelier (1900): see docs/REFERENCES.md#bachelier-1900.
   * @returns Object `{ delta, gamma, vega }`.
   * @param forward - Forward price or rate at expiry (may be negative).
   * @param strike - Strike in the same units as `forward`.
   * @param normalVol - Annualized absolute (normal) volatility in the units of `forward`.
   * @param expiry - Time to expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a JavaScript exception if any Greek is non-finite.
   */
  bachelierGreeks(
    forward: number,
    strike: number,
    normalVol: number,
    expiry: number,
    isCall: boolean
  ): ForwardGreeks;
  /**
   * Shifted (displaced) Black undiscounted per-unit price for negative-rate markets.
   *
   * Prices `Black(forward + shift, strike + shift, vol, expiry)`.
   * @returns Undiscounted per-unit option value in the units of `forward`.
   * @param forward - Forward rate at expiry (decimal; may be negative).
   * @param strike - Strike (decimal, same units as `forward`).
   * @param vol - Annualized shifted-lognormal volatility, decimal.
   * @param expiry - Time to expiry in years.
   * @param shift - Displacement added to forward and strike, in rate units (e.g. `0.03` for a 3% shift); both shifted values must be positive.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a JavaScript exception if the inputs produce a non-finite price.
   */
  blackShiftedPrice(
    forward: number,
    strike: number,
    vol: number,
    expiry: number,
    shift: number,
    isCall: boolean
  ): number;
  /**
   * Shifted (displaced) Black vega per unit (1.0) change in `vol`, undiscounted.
   * @returns Undiscounted vega in the units of `forward` per unit vol.
   * @param forward - Forward rate at expiry (decimal; may be negative).
   * @param strike - Strike (decimal, same units as `forward`).
   * @param vol - Annualized shifted-lognormal volatility, decimal.
   * @param expiry - Time to expiry in years.
   * @param shift - Displacement added to forward and strike, in rate units.
   * @throws Error - Throws a JavaScript exception if the inputs produce a non-finite vega.
   */
  blackShiftedVega(
    forward: number,
    strike: number,
    vol: number,
    expiry: number,
    shift: number
  ): number;
  /**
   * Reiner-Rubinstein continuous-monitoring barrier call price.
   *
   * `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
   * Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991.
   * @returns Discounted barrier-call price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param barrier - Continuously monitored barrier level in the same price units as spot.
   * @param rate - Continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to expiry in years.
   * @param direction - Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.
   * @param knock - Barrier activation: `"in"` for knock-in or `"out"` for knock-out.
   * @throws Error - Throws a JavaScript exception if `direction` or `knock` is unsupported, or the supplied model inputs produce a non-finite barrier price.
   */
  barrierCall(
    spot: number,
    strike: number,
    barrier: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    direction: 'up' | 'down',
    knock: 'in' | 'out'
  ): number;
  /**
   * Reiner-Rubinstein continuous-monitoring barrier put price.
   *
   * `direction` is `"up"` or `"down"`, `knock` is `"in"` or `"out"`.
   * Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991.
   * @returns Discounted barrier-put price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param barrier - Continuously monitored barrier level in the same price units as spot.
   * @param rate - Continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to expiry in years.
   * @param direction - Barrier direction: `"up"` for an upper barrier or `"down"` for a lower barrier.
   * @param knock - Barrier activation: `"in"` for knock-in or `"out"` for knock-out.
   * @throws Error - Throws a JavaScript exception if `direction` or `knock` is unsupported, or the supplied model inputs produce a non-finite barrier price.
   */
  barrierPut(
    spot: number,
    strike: number,
    barrier: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    direction: 'up' | 'down',
    knock: 'in' | 'out'
  ): number;
  /**
   * Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option.
   *
   * Kemna-Vorst (1990): see docs/REFERENCES.md#kemna-vorst-1990.
   * Turnbull-Wakeman (1991): see docs/REFERENCES.md#turnbull-wakeman-1991.
   * @returns Discounted Asian option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to expiry in years.
   * @param numFixings - Positive number of equally spaced averaging observations before expiry.
   * @param averaging - Asian averaging convention: `"arithmetic"` (default) or `"geometric"`.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a JavaScript exception if `averaging` is not `"arithmetic"` or `"geometric"`, or the supplied model inputs produce a non-finite option price.
   */
  asianOptionPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    numFixings: number,
    averaging?: 'arithmetic' | 'geometric',
    isCall?: boolean
  ): number;
  /**
   * Conze-Viswanathan lookback option.
   *
   * `strike_type` is `"fixed"` (default) or `"floating"`. For `"floating"`,
   * `strike` is ignored and `extremum` is the observed min/max to date.
   * Conze-Viswanathan (1991): see docs/REFERENCES.md#conze-viswanathan-1991.
   * @returns Discounted lookback option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield or foreign rate, expressed as a decimal.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param expiry - Time to expiry in years.
   * @param extremum - Observed maximum for fixed calls or floating puts, minimum for fixed puts or floating calls, including current spot; in spot-price units.
   * @param strikeType - Lookback payoff convention: `"fixed"` (default) or `"floating"`.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws Error - Throws a JavaScript exception if `strikeType` is not `"fixed"` or `"floating"`, or the supplied model inputs produce a non-finite option price.
   */
  lookbackOptionPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    extremum: number,
    strikeType?: 'fixed' | 'floating',
    isCall?: boolean
  ): number;
  /**
   * Quanto option (FX-adjusted cross-currency) price in domestic currency.
   *
   * Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983.
   * Brigo-Mercurio (2006): see docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models.
   *
   * @returns Discounted quanto option price in domestic currency units.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param expiry - Time to expiry in years.
   * @param rateDomestic - Domestic continuously compounded risk-free rate, expressed as a decimal.
   * @param rateForeign - Foreign continuously compounded risk-free rate, expressed as a decimal.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param volAsset - Annualized asset-price volatility expressed as a decimal.
   * @param volFx - Annualized FX-rate volatility expressed as a decimal.
   * @param correlation - Instantaneous correlation between the asset and FX-rate shocks, from -1 to 1.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @throws If the inputs produce a non-finite price.
   */
  quantoOptionPrice(
    spot: number,
    strike: number,
    expiry: number,
    rateDomestic: number,
    rateForeign: number,
    divYield: number,
    volAsset: number,
    volFx: number,
    correlation: number,
    isCall?: boolean
  ): number;
  /**
   * Closed-form (Fourier) Heston price of a European option.
   *
   * Heston (1993): see docs/REFERENCES.md#heston-1993.
   * @returns Present-value per-unit option price.
   * @param spot - Current spot price in the same units as the strike.
   * @param strike - Option strike price.
   * @param expiry - Time to expiry in years; a non-positive value returns intrinsic.
   * @param rate - Continuously compounded risk-free rate, decimal.
   * @param divYield - Continuous dividend yield or foreign rate, decimal.
   * @param kappa - Mean-reversion speed of the variance process (per year).
   * @param theta - Long-run variance level (variance units).
   * @param sigmaV - Volatility of variance (vol-of-vol).
   * @param rho - Spot/variance correlation in `(-1, 1)`.
   * @param v0 - Initial instantaneous variance (variance, not volatility).
   * @param isCall - Whether to value a call (`true`, default) or put (`false`).
   * @throws Error - Throws a JavaScript exception if a parameter is non-finite or outside its domain, or the Fourier integration fails to produce a finite price.
   */
  hestonPrice(
    spot: number,
    strike: number,
    expiry: number,
    rate: number,
    divYield: number,
    kappa: number,
    theta: number,
    sigmaV: number,
    rho: number,
    v0: number,
    isCall?: boolean
  ): number;
  /**
   * Price a European option under the Black-Scholes model using the COS method.
   *
   * Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
   * Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973.
   * @returns Discounted European option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param vol - Annualized volatility expressed as a decimal, such as 0.20 for 20%; must be positive.
   * @param expiry - Time to option expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @param nTerms - Optional positive number of COS expansion terms; omit to use the pricer default.
   * @throws Error - Throws a JavaScript exception if `vol` is not positive, the model produces a degenerate or invalid COS truncation range, a non-finite characteristic-function value or forward moment, or a non-finite option price.
   */
  bsCosPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    vol: number,
    expiry: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /**
   * Price a European option under the Variance Gamma model using the COS method.
   *
   * Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
   * Madan-Carr-Chang (1998): see docs/REFERENCES.md#madan-carr-chang-1998.
   * @returns Discounted European option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param theta - Variance-Gamma drift parameter controlling skew in log returns.
   * @param nu - Variance-Gamma variance-rate parameter; larger values increase tail thickness.
   * @param expiry - Time to option expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @param nTerms - Optional positive number of COS expansion terms; omit to use the pricer default.
   * @throws Error - Throws a JavaScript exception if the model produces a degenerate or invalid COS truncation range, a non-finite characteristic-function value or forward moment, or a non-finite option price.
   */
  vgCosPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    sigma: number,
    theta: number,
    nu: number,
    expiry: number,
    isCall: boolean,
    nTerms?: number
  ): number;
  /**
   * Price a European option under Merton (1976) jump-diffusion using the COS method.
   *
   * Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008.
   * Merton jump-diffusion (1976): see docs/REFERENCES.md#merton-1976-jump.
   * @returns Discounted European option price in the same units as `spot`.
   * @param spot - Current spot price or exchange rate in the same units as the strike.
   * @param strike - Option strike price in the same price units as the underlying.
   * @param rate - Interest rate expressed as a decimal, such as 0.05 for 5%.
   * @param divYield - Continuous dividend yield expressed as a decimal, such as 0.02 for 2%.
   * @param sigma - Annualized volatility expressed as a decimal, such as 0.20 for 20%.
   * @param muJump - Mean log jump size in the Merton jump-diffusion model.
   * @param sigmaJump - Standard deviation of log jump sizes in the Merton jump-diffusion model.
   * @param lambda - Annual jump-arrival intensity in the Merton jump-diffusion model.
   * @param expiry - Time to option expiry in years.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @param nTerms - Optional positive number of COS expansion terms; omit to use the pricer default.
   * @throws Error - Throws a JavaScript exception if the model produces a degenerate or invalid COS truncation range, a non-finite characteristic-function value or forward moment, or a non-finite option price.
   */
  mertonJumpCosPrice(
    spot: number,
    strike: number,
    rate: number,
    divYield: number,
    sigma: number,
    muJump: number,
    sigmaJump: number,
    lambda: number,
    expiry: number,
    isCall: boolean,
    nTerms?: number
  ): number;
}

/**
 * Namespaced TypeScript entry point for models APIs.
 */
export declare const models: ModelsNamespace;

/**
 * Quote ingestion, market construction, and explicit model calibration.
 * @example
 * ```typescript
 * import init, { calibration } from "finstack-quant-wasm";
 * await init();
 * const report = calibration.dryRun({
 *   schema: "finstack_quant.calibration/1",
 *   plan: { id: "smoke", description: null, quote_sets: {}, steps: [], settings: {} }
 * });
 * console.log(JSON.parse(report).errors);
 * ```
 */
export interface CalibrationNamespace {
  /**
   * Execute a calibration envelope and return its fitted market and reports.
   * @param envelope - Typed calibration envelope or its serialized JSON form.
   * @returns Calibration result including the materialized market and per-step reports.
   * @throws Error - Throws a JavaScript exception if `envelopeJson` is malformed or violates the calibration schema or static plan contract (fail-fast: first static error; `dryRun` lists every static error), market context construction or a calibration step fails, a solver does not converge, or the result envelope cannot be converted to a JavaScript value.
   */
  calibrate(envelope: CalibrationEnvelope | string): CalibrationResultEnvelope;
  /**
   * Validate and canonicalize a calibration envelope without solving it.
   * @param envelope - Typed calibration envelope or its serialized JSON form.
   * @returns Canonical pretty-printed calibration-envelope JSON.
   * @throws Error - Throws a JavaScript exception if `json` is malformed, its calibration schema marker is missing, malformed, or unsupported, static envelope validation fails (fail-fast: first error; `dryRun` lists every static error), or the canonical envelope cannot be serialized.
   */
  validateCalibrationJson(envelope: CalibrationEnvelope | string): string;
  /**
   * Return all static plan errors and dependencies without running solvers.
   * @param envelope - Typed calibration envelope or its serialized JSON form.
   * @returns JSON `CalibrationValidationReport` containing errors and a dependency graph.
   * @throws Error - Throws a JavaScript exception if `envelopeJson` is malformed, its schema marker is missing, malformed, or unsupported, the envelope structure is invalid, or the validation report cannot be serialized. Semantic findings are returned in the report rather than thrown.
   */
  dryRun(envelope: CalibrationEnvelope | string): string;
  /**
   * Fit the Bermudan LMM loading scale from the market swaption surface.
   * @param market - Reusable market handle containing discount and swaption-volatility inputs.
   * @param asOf - ISO-8601 valuation date.
   * @returns Positive finite LMM base volatility.
   * @throws Error - Throws if the envelope is not a Bermudan swaption, the date or market inputs are invalid, or the Rebonato calibration cannot be completed.
   * @param instrument - Instrument used by this call.
   */
  calibrateBermudanLmmBaseVol(
    instrument: Record<string, unknown> | string,
    market: Market,
    asOf: string
  ): number;
}

/**
 * Namespaced TypeScript entry point for calibration APIs.
 */
export declare const calibration: CalibrationNamespace;

/**
 * Namespaced TypeScript entry points for valuations calculations and types.
 * @example
 * ```typescript
 * import init, { valuations } from "finstack-quant-wasm";
 * await init();
 * console.log(valuations.instruments.listStandardMetrics());
 * ```
 */
export interface ValuationsNamespace {
  /**
   * Generic cross-asset composite instruments, primitive exposures, and dated history.
   *
   * Frozen quantities are used for pricing. Host bindings expose `initialize`
   * for both fixed and dynamic weighting; there is no `initializeFixed` export.
   */
  composite: CompositeNamespace;
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
   * Listed-market coverage metadata and canonical instrument routing.
   */
  market: ValuationMarketNamespace;
  /**
   * Deserialize a `ValuationResult` from JSON and return the canonical JSON.
   *
   * Validates the input conforms to the `ValuationResult` schema.
   * @returns Canonical `ValuationResult` JSON after deserialization.
   * @param json - Canonical valuation-result JSON to validate and reserialize.
   * @throws Error - Throws a JavaScript exception if `json` is malformed or does not match the `ValuationResult` schema, or the canonical result cannot be serialized.
   */
  validateValuationResultJson(json: string): string;
  /**
   * Parsed `MarketContext` handle for reuse across pricing calls.
   */
  Market: typeof Market;
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
   * [`CumulativeCouponTracker`](finstack_quant_valuations::instruments::rates::hw1f::cumulative_coupon::CumulativeCouponTracker) configured with
   * `target_coupon`; once cumulative hits the target, the final coupon is
   * capped and the instrument is considered redeemed.
   * @returns Period coupons, running cumulative, redemption index, and whether the TARN redeemed early.
   * @param fixedRate - Fixed coupon rate in decimal form before subtracting each floating fixing.
   * @param couponFloor - Minimum period coupon rate in decimal form after the TARN rate calculation.
   * @param floatingFixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
   * @param targetCoupon - Cumulative coupon target, as a fraction of notional, that redeems the TARN.
   * @param dayCountFraction - Accrual year fraction applied to each coupon period.
   * @throws Error - Throws a JavaScript exception if `fixed_rate`, `coupon_floor`, `target_coupon`, `day_count_fraction`, or any fixing is non-finite; `coupon_floor` is negative; `target_coupon` or `day_count_fraction` is non-positive; or the result cannot be converted to a JavaScript object.
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
   * @returns One decimal coupon rate per fixing, in the same order as `floatingFixings`.
   * @param initialCoupon - Starting coupon rate before the first snowball update, in decimal form.
   * @param fixedRate - Fixed coupon rate in decimal form added at each snowball step.
   * @param floatingFixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
   * @param floor - Minimum permitted coupon rate in decimal form.
   * @param cap - Maximum permitted coupon rate in decimal form.
   * @throws Error - Throws a JavaScript exception if `initial_coupon` or `floor` is negative; `initial_coupon`, `fixed_rate`, `floor`, or any fixing is non-finite; or `cap` is NaN or is not greater than `floor`.
   */
  snowballCouponProfile(
    initialCoupon: number,
    fixedRate: number,
    floatingFixings: number[],
    floor: number,
    cap: number
  ): Float64Array;
  /**
   * Path-independent inverse-floater coupon schedule.
   * @returns One decimal coupon rate per fixing, in the same order as `floatingFixings`.
   * @param fixedRate - Fixed coupon rate in decimal form before the leveraged floating deduction.
   * @param floatingFixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
   * @param floor - Minimum permitted coupon rate in decimal form.
   * @param cap - Maximum permitted coupon rate in decimal form.
   * @param leverage - Positive multiplier applied to each floating fixing in the inverse-floater coupon.
   * @throws Error - Throws a JavaScript exception if `floor` is negative; `fixed_rate`, `floor`, `leverage`, or any fixing is non-finite; `leverage` is non-positive; or `cap` is NaN or is not greater than `floor`.
   */
  inverseFloaterCouponProfile(
    fixedRate: number,
    floatingFixings: number[],
    floor: number,
    cap: number,
    leverage: number
  ): Float64Array;
  /**
   * Intrinsic (undiscounted, unhedged) payoff of a CMS spread option.
   *
   * `call:  notional * max(long_cms - short_cms - strike, 0)`
   * `put:   notional * max(strike - (long_cms - short_cms), 0)`
   * @returns Undiscounted intrinsic payoff in the same units as `notional`.
   * @param longCms - Long-tenor CMS rate in decimal form.
   * @param shortCms - Short-tenor CMS rate in decimal form.
   * @param strike - CMS rate-spread strike in decimal form.
   * @param isCall - Whether to value a call (`true`) or put (`false`).
   * @param notional - Signed trade notional in the instrument's native currency units.
   * @throws Error - Throws a JavaScript exception if a CMS rate or `strike` is non-finite, or if `notional` is negative or non-finite.
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
   * @returns Accrued coupon as a decimal fraction of notional for the observation period.
   * @param lower - Inclusive lower bound of the observed-rate range, in decimal form.
   * @param upper - Inclusive upper bound of the observed-rate range, in decimal form.
   * @param observations - Observed floating rates in decimal form for the accrual period.
   * @param couponRate - Contractual coupon rate in decimal form before range weighting.
   * @param dayCountFraction - Accrual year fraction for the coupon period.
   * @throws Error - Throws a JavaScript exception if the range bounds are non-finite or not strictly ordered; `observations` is empty or contains a non-finite value; or `coupon_rate` or `day_count_fraction` is negative or non-finite.
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
 *
 * Optional `modelParamsT0Json` and `creditFactorModelJson` attach an opening
 * model-parameter snapshot and a credit-factor model after construction.
 */
export interface AttributionParams extends WasmOwned {
  /**
   * Optional serialized opening `ModelParamsSnapshot` JSON.
   */
  modelParamsT0Json?: string | null;
  /**
   * Optional serialized `CreditFactorModel` JSON.
   */
  creditFactorModelJson?: string | null;
}

/**
 * P&L attribution result returned by `attributePnl`.
 *
 * This is the same document Python callers hold as
 * `finstack_quant.attribution.PnlAttribution` — field names are the canonical
 * Rust serde names, so `JSON.stringify(result)` is byte-comparable with the
 * Python `to_json()` output and with `attributePnlJson`.
 */
export interface PnlAttribution {
  /**
   * Total P&L as reported by this attribution (total-return convention:
   * intra-period coupon income is included so the factor sum reconciles).
   */
  total_pnl: MoneyValue;
  /**
   * Pure mark-to-market change `val_t1 - val_t0` with no cashflow
   * adjustment; absent for attribution paths that cannot provide it.
   */
  mark_to_market_pnl?: MoneyValue;
  /**
   * Carry P&L (theta + accruals).
   */
  carry: MoneyValue;
  /**
   * Interest rate curves P&L.
   */
  rates_curves_pnl: MoneyValue;
  /**
   * Credit hazard curves P&L.
   */
  credit_curves_pnl: MoneyValue;
  /**
   * Inflation curves P&L.
   */
  inflation_curves_pnl: MoneyValue;
  /**
   * Base correlation curves P&L.
   */
  correlations_pnl: MoneyValue;
  /**
   * FX rate changes P&L (pricing impact on cross-currency instruments).
   */
  fx_pnl: MoneyValue;
  /**
   * FX translation P&L (reporting-currency component; zero unless an
   * explicit target currency differs from the native pricing currency).
   */
  fx_translation_pnl: MoneyValue;
  /**
   * Implied volatility changes P&L.
   */
  vol_pnl: MoneyValue;
  /**
   * Cross-factor interaction P&L.
   */
  cross_factor_pnl: MoneyValue;
  /**
   * Model parameter changes P&L.
   */
  model_params_pnl: MoneyValue;
  /**
   * Market scalar changes P&L.
   */
  market_scalars_pnl: MoneyValue;
  /**
   * Unexplained residual P&L.
   */
  residual: MoneyValue;
  /**
   * Gross carry decomposition, with funding reported separately as a financing overlay; or `null` when not produced.
   */
  carry_detail: Record<string, unknown> | null;
  /**
   * Detailed rates curves attribution, or `null` when not produced.
   */
  rates_detail: Record<string, unknown> | null;
  /**
   * Detailed credit curves attribution, or `null` when not produced.
   */
  credit_detail: Record<string, unknown> | null;
  /**
   * Detailed inflation curves attribution, or `null` when not produced.
   */
  inflation_detail: Record<string, unknown> | null;
  /**
   * Detailed correlations attribution, or `null` when not produced.
   */
  correlations_detail: Record<string, unknown> | null;
  /**
   * Detailed FX attribution, or `null` when not produced.
   */
  fx_detail: Record<string, unknown> | null;
  /**
   * Detailed volatility attribution, or `null` when not produced.
   */
  vol_detail: Record<string, unknown> | null;
  /**
   * Detailed cross-factor attribution, or `null` when not produced.
   */
  cross_factor_detail: Record<string, unknown> | null;
  /**
   * Detailed model parameters attribution, or `null` when not produced.
   */
  model_params_detail: Record<string, unknown> | null;
  /**
   * Detailed market scalars attribution, or `null` when not produced.
   */
  scalars_detail: Record<string, unknown> | null;
  /**
   * Credit-factor-hierarchy decomposition of `credit_curves_pnl`; present
   * only when a calibrated credit factor model was supplied.
   */
  credit_factor_detail?: Record<string, unknown>;
  /**
   * Credit-factor-hierarchy decomposition of carry; present only when a
   * calibrated credit factor model was supplied.
   */
  credit_carry_decomposition?: Record<string, unknown>;
  /**
   * Attribution metadata: method, instrument id, tolerances, rounding
   * context, and policy stamps.
   */
  meta: Record<string, unknown>;
  /**
   * True when residual computation hit non-finite inputs; the attribution
   * should then be treated as invalid.
   */
  result_invalid: boolean;
}

/**
 * Namespaced TypeScript entry points for attribution calculations and types.
 * @example
 * ```typescript
 * import init, { attribution } from "finstack-quant-wasm";
 * await init();
 * console.log(attribution.defaultWaterfallOrder());
 * ```
 */
export interface AttributionNamespace {
  /**
   * Parameters constructor emitted by wasm-bindgen for attribution calls.
   *
   * `configJson` may include `{ "execution_policy": "parallel" }` to opt into
   * inner Rayon when the host is not already parallelizing attribution at the
   * portfolio or batch level. Serial is the default. Set
   * `modelParamsT0Json` / `creditFactorModelJson` on the constructed object
   * to attach an opening model-parameter snapshot or credit-factor model.
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
   * result as a structured object with the canonical Rust serde field names;
   * use `attributePnlJson` for the JSON wire string. `config_json` may include
   * `"execution_policy": "parallel"` to opt into inner Rayon when the host
   * is not already parallelizing attribution at a higher level. Serial is
   * the default.
   * @returns Structured `PnlAttribution` result object for the instrument.
   * @param params - Fully specified AttributionParams object containing instrument, markets, dates, and method.
   * @throws Error - Rejects malformed instrument, market, method, or configuration JSON; invalid ISO attribution dates; instrument or market reconstruction, pricing, FX, rounding, metric, or method-specific attribution failures; a caught attribution panic; or failure to convert the result to a JavaScript value.
   */
  attributePnl(params: AttributionParams): PnlAttribution;
  /**
   * Run P&L attribution for a single instrument and return wire JSON.
   *
   * Wire twin of `attributePnl`: same inputs, validation, and panic
   * containment, returning the `PnlAttribution` as a JSON string instead of
   * a structured object.
   * @returns JSON-serialized `PnlAttribution` wire document.
   * @param params - Fully specified AttributionParams object containing instrument, markets, dates, and method.
   * @throws Error - Rejects the same conditions as [`attribute_pnl`], plus failure to serialize the result to JSON.
   */
  attributePnlJson(params: AttributionParams): string;
  /**
   * Run attribution from a full JSON `AttributionEnvelope` and return JSON.
   *
   * Power-user variant for full envelope round-trip workflows.
   * @returns JSON attribution result envelope for the supplied spec.
   * @param specJson - JSON-serialized AttributionParams specification to validate and execute.
   * @throws Error - Rejects malformed, schema-incompatible, or unsupported-version `spec_json`; instrument or market reconstruction, pricing, FX, rounding, metric, or method-specific attribution failures; a caught parse or execution panic; or failure to serialize the result envelope.
   */
  attributePnlEnvelopeJson(specJson: string): string;
  /**
   * Validate an attribution specification JSON.
   *
   * Deserializes against the `AttributionEnvelope` schema, checks the
   * `schema` version tag (the same gate `execute` applies, so a payload that
   * validates here cannot later be rejected at execution), and returns the
   * canonical JSON.
   * @returns Canonical attribution-envelope JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed, schema-incompatible, or unsupported-version `json`, or failure to serialize the canonical attribution envelope.
   */
  validateAttributionJson(json: string): string;
  /**
   * Return the default waterfall factor ordering as canonical snake-case values.
   * @returns Default waterfall factor names in execution order.
   * @throws Error - Rejects if the default factor identifiers cannot be serialized to JavaScript.
   */
  defaultWaterfallOrder(): string[];
  /**
   * Return the default metric IDs used by metrics-based attribution.
   * @returns Default metric identifiers used by metrics-based attribution.
   * @throws Error - Rejects if the default metric identifiers cannot be serialized to JavaScript.
   */
  defaultAttributionMetrics(): string[];
}

/**
 * Namespaced TypeScript entry point for attribution APIs.
 */
export declare const attribution: AttributionNamespace;

// --- statements ------------------------------------------------------------

/**
 * Evaluated statement model, as returned by `statements.evaluateModel` and
 * `statements.evaluateModelWithMarket`.
 *
 * Structurally identical to the Rust `StatementResult` serde form (the same
 * payload the Python binding exposes as a typed `StatementResult`); pass it
 * back to the JSON-taking analytics entry points via `JSON.stringify`.
 */
export interface StatementResultJson {
  /**
   * Evaluated values keyed by node id and period. Missing/non-finite numeric
   * results use canonical strings "nan", "inf", or "-inf" and survive JSON.stringify.
   */
  nodes: Record<string, Record<string, number | 'nan' | 'inf' | '-inf'>>;
  /**
   * Audit stamp: numeric mode, rounding context, and FX policy in force.
   */
  meta?: Record<string, unknown>;
  [key: string]: unknown;
}

/**
 * Namespaced TypeScript entry points for statements calculations and types.
 * @example
 * ```typescript
 * import init, { statements } from "finstack-quant-wasm";
 * await init();
 * console.log(statements.parseFormulaText("revenue - expenses"));
 * ```
 */
export interface StatementsNamespace {
  /**
   * Validate a `FinancialModelSpec` JSON string.
   *
   * Deserializes the input against the model schema, runs semantic validation,
   * and returns the canonical (re-serialized) JSON.
   * @returns Canonical financial-model JSON after semantic validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, an empty or invalid period timeline, reserved node identifiers, incompatible node fields or value types, invalid formulas or dimensions, an invalid waterfall, or failure to serialize the normalized model.
   */
  validateFinancialModelJson(json: string): string;
  /**
   * Get the node identifiers from a model specification JSON.
   *
   * Returns a JS array of node ID strings in declaration order.
   * @returns Node identifiers in model-declaration order.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, or if the node identifiers cannot be serialized to JavaScript.
   */
  modelNodeIds(json: string): string[];
  /**
   * Validate a `CheckSuiteSpec` JSON string.
   *
   * Deserializes the spec, re-serializes to canonical form, and
   * returns the JSON string. Useful for client-side validation.
   * @returns Canonical check-suite JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, or failure to serialize the decoded check-suite specification.
   */
  validateCheckSuiteSpecJson(json: string): string;
  /**
   * Validate a `CapitalStructureSpec` JSON string.
   * @returns Canonical capital-structure JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, or failure to serialize the decoded capital-structure specification.
   */
  validateCapitalStructureSpecJson(json: string): string;
  /**
   * Validate a `WaterfallSpec` JSON string.
   *
   * Performs both serde deserialization and the waterfall's internal
   * consistency check (for example rejecting `Sweep` ordered after `Equity`
   * when an ECF sweep is configured).
   * @returns Canonical waterfall JSON after schema validation.
   * @param json - Canonical JSON string for a `WaterfallSpec`, including `priority_of_payments`, `available_cash_node`, optional `ecf_sweep`, `pik_toggle`, `payment_classes`, `mandatory_prepay_node`, and `voluntary_prepay_node`.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`; duplicate or inconsistent payment priorities; incomplete available-cash priorities; invalid PIK, payment-class, prepay-node, or ECF-sweep settings; or failure to serialize the validated waterfall.
   */
  validateWaterfallSpecJson(json: string): string;
  /**
   * Validate an `EcfSweepSpec` JSON string.
   * @returns Canonical ECF-sweep JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, or failure to serialize the decoded ECF-sweep specification.
   */
  validateEcfSweepSpecJson(json: string): string;
  /**
   * Validate a `PikToggleSpec` JSON string.
   * @returns Canonical PIK-toggle JSON after schema validation.
   * @param json - Canonical JSON string defining the object to deserialize or normalize.
   * @throws Error - Rejects malformed or schema-incompatible `json`, or failure to serialize the decoded PIK-toggle specification.
   */
  validatePikToggleSpecJson(json: string): string;
  /**
   * Evaluate a `FinancialModelSpec` and return the `StatementResult` JSON.
   * @returns Evaluated statement result with node values and optional audit metadata.
   * @param modelJson - JSON-serialized FinancialModelSpec to evaluate across its statement periods.
   * @throws Error - Rejects malformed `model_json`, model semantic failures, invalid formula or dependency graphs, missing evaluation inputs, unsupported capital-structure requirements, or failure to serialize the statement result to JavaScript.
   */
  evaluateModel(modelJson: string): StatementResultJson;
  /**
   * Evaluate a `FinancialModelSpec` against a `MarketContext` as of a given date.
   *
   * Required for capital-structure-aware models. The `as_of` argument is an
   * ISO 8601 date string (e.g. `"2025-01-15"`).
   * @returns Evaluated statement result using the supplied market as of `asOf`.
   * @param modelJson - JSON-serialized FinancialModelSpec to evaluate across its statement periods.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Rejects malformed model or market JSON, model semantic failures, an invalid ISO `as_of` date, invalid formulas or dependencies, missing market data, or failure to serialize the statement result to JavaScript.
   */
  evaluateModelWithMarket(modelJson: string, marketJson: string, asOf: string): StatementResultJson;
  /**
   * Run Monte Carlo simulation on a financial model (JSON in/out).
   * @returns Canonical JSON containing percentile summaries and optional path data.
   * @param modelJson - Financial-model specification JSON.
   * @param configJson - Monte Carlo configuration JSON.
   * @throws Error - Rejects malformed model or configuration JSON, model semantic failures, zero simulation paths, a model containing capital structure, model compilation or dependency failures, any path-evaluation failure, or failure to serialize the results to JavaScript.
   */
  runMonteCarlo(modelJson: string, configJson: string): Record<string, unknown>;
  /**
   * Parse a DSL formula and return a human-readable rendering of its AST.
   *
   * Useful for previewing expression structure in UI tooling before
   * committing a formula to a model. The returned string is a debug rendering,
   * **not** JSON: the canonical `StmtExpr` AST deliberately does not implement
   * `serde::Serialize`, so there is no structured wire form to return. Treat
   * the output as display text and do not parse it.
   * @returns Returns a human-readable text rendering, not JSON.
   * @param formula - Financial-model formula string to parse into its canonical expression representation.
   * @throws Error - Rejects trailing tokens, malformed or incomplete syntax, or a formula that exceeds the parser's nesting or term limits.
   */
  parseFormulaText(formula: string): string;
  /**
   * Validate that a DSL formula parses and compiles successfully.
   *
   * Returns `undefined` when the formula is valid; throws a `FinstackError`
   * otherwise. This mirrors the Python `validate_formula` API, which returns
   * `None` — an invalid formula raises rather than returning a falsy value, so
   * `if (validateFormula(f))` is not a validity check.
   * @returns nothing; failure is reported by throwing.
   * @param formula - Financial-model formula string to parse and validate without evaluation.
   * @throws Error - Rejects any formula that cannot be parsed as one complete DSL expression or compiled because it contains an unsupported component, function, or operator form.
   */
  validateFormula(formula: string): void;
}

/**
 * Namespaced TypeScript entry point for statements APIs.
 */
export declare const statements: StatementsNamespace;

// --- statements_analytics -------------------------------------------------

/**
 * Enterprise-value swing for one shocked DCF assumption.
 */
export interface DcfSensitivityEntry {
  /**
   * Identifier of the shocked assumption.
   */
  parameter_id: string;
  /**
   * Enterprise-value delta at the downside shock.
   */
  downside: number;
  /**
   * Enterprise-value delta at the upside shock.
   */
  upside: number;
}

/**
 * Ranked DCF assumption sensitivities, as returned by
 * `statements_analytics.dcfSensitivity`.
 */
export interface DcfSensitivityResult {
  /**
   * Unshocked enterprise value as a `Money` wire object (exact decimal
   * `amount` string plus ISO-4217 `currency`).
   */
  baseline_enterprise_value: MoneyValue;
  /**
   * Tornado entries sorted by descending absolute swing.
   */
  entries: DcfSensitivityEntry[];
  /**
   * Effective downside WACC after any clamping.
   */
  wacc_down: number;
  /**
   * Whether the downside WACC hit the denominator floor.
   */
  wacc_down_clamped: boolean;
  /**
   * Effective upside terminal growth rate after any clamping.
   */
  terminal_growth_up: number;
  /**
   * Whether the upside terminal growth rate hit the denominator floor.
   */
  terminal_growth_up_clamped: boolean;
}

/**
 * Leveraged-buyout transaction result, as returned by
 * `statements_analytics.evaluateLbo`. Monetary fields are `Money` wire
 * objects (exact decimal `amount` string plus ISO-4217 `currency`).
 */
export interface LboResult {
  /**
   * Entry enterprise value priced at the model's first period.
   */
  entry_enterprise_value: MoneyValue;
  /**
   * Entry metric value read from the entry metric node.
   */
  entry_metric: number;
  /**
   * Total funded debt at close.
   */
  debt_total: MoneyValue;
  /**
   * Sponsor equity check solved as the sources-and-uses residual.
   */
  equity_check: MoneyValue;
  /**
   * Total sources at close.
   */
  sources_total: MoneyValue;
  /**
   * Total uses at close.
   */
  uses_total: MoneyValue;
  /**
   * Whether sources and uses balance within tolerance.
   */
  sources_uses_balanced: boolean;
  /**
   * Exit enterprise value at the exit period.
   */
  exit_enterprise_value: MoneyValue;
  /**
   * Exit metric value read from the exit metric node.
   */
  exit_metric: number;
  /**
   * Modelled net debt outstanding at the exit period.
   */
  exit_net_debt: MoneyValue;
  /**
   * Exit equity proceeds: exit enterprise value less exit net debt.
   */
  exit_equity_proceeds: MoneyValue;
  /**
   * Multiple on invested capital.
   */
  moic: number;
}

/**
 * Solved input and optional updated model from a goal-seek.
 */
export interface GoalSeekResult {
  /**
   * Input value that meets the goal-seek target within solver tolerance.
   */
  solved_value: number;
  /**
   * Canonical model JSON with the solved input substituted, when a model was supplied.
   */
  updated_model_json?: string;
}

/**
 * One parameter's downside and upside impact in a tornado chart.
 */
export interface TornadoEntry {
  /**
   * Parameter node identifier represented by this entry.
   */
  parameter_id: string;
  /**
   * Metric change at the parameter's minimum perturbation.
   */
  downside: number;
  /**
   * Metric change at the parameter's maximum perturbation.
   */
  upside: number;
}

/**
 * Severity assigned to one statement-check finding.
 */
export type CheckSeverity = 'info' | 'warning' | 'error';

/**
 * Category grouping for a statement check.
 */
export type CheckCategory =
  | 'accounting_identity'
  | 'cross_statement_reconciliation'
  | 'internal_consistency'
  | 'credit_reasonableness'
  | 'data_quality';

/**
 * Quantitative materiality context attached to a check finding.
 */
export interface CheckMateriality {
  /**
   * Absolute discrepancy in the compared nodes' units.
   */
  absolute: number;
  /**
   * Discrepancy as a percentage of the reference value.
   */
  relative_pct: number;
  /**
   * Denominator used to calculate `relative_pct`.
   */
  reference_value: number;
  /**
   * Human-readable denominator label.
   */
  reference_label: string;
}

/**
 * One diagnostic produced by a statement check.
 */
export interface CheckFinding {
  /**
   * Identifier of the check that produced this finding.
   */
  check_id: string;
  /**
   * Diagnostic severity.
   */
  severity: CheckSeverity;
  /**
   * Human-readable issue description.
   */
  message: string;
  /**
   * Optional statement period associated with the finding.
   */
  period?: string;
  /**
   * Optional quantitative materiality context.
   */
  materiality?: CheckMateriality;
  /**
   * Statement node identifiers involved in the finding.
   */
  nodes?: string[];
}

/**
 * Outcome of one statement-check execution.
 */
export interface CheckResult {
  /**
   * Stable check identifier.
   */
  check_id: string;
  /**
   * Human-readable check name.
   */
  check_name: string;
  /**
   * Category of this `CheckResult`.
   */
  category: CheckCategory;
  /**
   * Whether no error-severity finding was retained.
   */
  passed: boolean;
  /**
   * Retained findings after suite reporting filters.
   */
  findings: CheckFinding[];
}

/**
 * Aggregate counts for a completed statement-check run.
 */
export interface CheckSummary {
  /**
   * Number of checks executed.
   */
  total_checks: number;
  /**
   * Number of checks that passed.
   */
  passed: number;
  /**
   * Number of checks that failed.
   */
  failed: number;
  /**
   * Number of retained error findings.
   */
  errors: number;
  /**
   * Number of retained warning findings.
   */
  warnings: number;
  /**
   * Number of retained informational findings.
   */
  infos: number;
}

/**
 * Structured report returned by statement-check runners.
 */
export interface CheckReport {
  /**
   * One result per executed check.
   */
  results: CheckResult[];
  /**
   * Aggregate check and finding counts.
   */
  summary: CheckSummary;
}

/**
 * Namespaced TypeScript entry points for statements analytics calculations and types.
 * @example
 * ```typescript
 * import init, { statements_analytics } from "finstack-quant-wasm";
 * await init();
 * console.log(statements_analytics.wacc(0.6, 0.1, 0.4, 0.05, 0.25));
 * ```
 */
export interface StatementsAnalyticsNamespace {
  /**
   * Run a sensitivity analysis on a financial model.
   *
   * Accepts JSON strings for the model spec and sensitivity configuration,
   * evaluates all perturbation scenarios, and returns JSON results.
   * @returns Sensitivity results for each perturbation scenario.
   * @param modelJson - Financial-model specification JSON.
   * @param configJson - Configuration JSON for this call.
   * @throws Error - Rejects malformed model or configuration JSON, invalid sensitivity modes or parameter perturbations, missing model nodes or periods, model-evaluation failures, or failure to serialize the sensitivity result to JavaScript.
   */
  runSensitivity(modelJson: string, configJson: string): Record<string, unknown>;
  /**
   * Run a variance analysis comparing two evaluated statement results.
   *
   * Returns JSON-serialized variance report.
   * @returns Variance report comparing the two evaluated statement results.
   * @param baseJson - Base statement-result JSON.
   * @param comparisonJson - Comparison statement-result JSON.
   * @param configJson - Configuration JSON for this call.
   * @throws Error - Rejects malformed result or configuration JSON, empty metric or period selections, mismatched metric types or currencies, a requested value missing from either result, or failure to serialize the variance report to JavaScript.
   */
  runVariance(
    baseJson: string,
    comparisonJson: string,
    configJson: string
  ): Record<string, unknown>;
  /**
   * Evaluate all scenarios in a scenario set against a base model.
   *
   * Returns a JSON object mapping scenario names to their statement results.
   * @returns Statement results keyed by scenario name.
   * @param modelJson - Financial-model specification JSON.
   * @param scenarioSetJson - Scenario-set JSON keyed by scenario name.
   * @throws Error - Rejects malformed model or scenario-set JSON, an empty scenario set, invalid parent chains, overrides of missing nodes, failure to evaluate any scenario, or failure to serialize the result map to JavaScript.
   */
  evaluateScenarioSet(
    modelJson: string,
    scenarioSetJson: string
  ): Record<string, StatementResultJson>;
  /**
   * Compute forecast accuracy metrics (MAE, MAPE, RMSE).
   *
   * Takes two float arrays (actual, forecast) and returns the serde form of
   * the Rust `ForecastMetrics` (`mae`, `mape`, `mape_effective_n`, `smape`,
   * `rmse`, `n`).
   * @returns Backtest forecast accuracy metrics for the selected series.
   * @param actual - Actual realized values aligned one-for-one with the forecast series.
   * @param forecast - Forecast values aligned one-for-one with the actual realized series.
   * @throws Error - Rejects inputs that cannot be decoded as numeric JavaScript arrays, arrays with unequal lengths, empty arrays, or metrics that cannot be serialized to JavaScript.
   */
  backtestForecast(actual: number[], forecast: number[]): BacktestForecastMetricsJson;
  /**
   * Generate tornado chart entries for a sensitivity result.
   * @param resultJson - Result JSON produced by a prior call.
   * @param metricNode - Statement metric node identifier selected for the requested analysis.
   * @param period - Model period label for the requested statement value or calculation.
   * @returns Structured tornado entries sorted by descending absolute swing.
   * @throws Error - Rejects malformed `result_json`, an invalid optional `period` identifier, or failure to convert the entries to JavaScript. A missing metric produces no entry rather than rejecting.
   */
  generateTornadoEntries(resultJson: string, metricNode: string, period?: string): TornadoEntry[];
  /**
   * Find the driver value that makes a target node reach a target value.
   * @returns Solved input value and optional updated model JSON.
   * @param modelJson - Financial-model specification JSON.
   * @param targetNode - Statement node identifier whose value is driven toward the target.
   * @param targetPeriod - Model period label in which the goal-seek target is evaluated.
   * @param targetValue - Numeric target value the goal-seek routine attempts to reach.
   * @param driverNode - Statement node identifier adjusted by the goal-seek routine.
   * @param driverPeriod - Model period label of the adjustable goal-seek driver.
   * @param updateModel - Whether to return the model with the solved driver value applied.
   * @param boundsLo - Lower numeric bound allowed for the goal-seek driver.
   * @param boundsHi - Upper numeric bound allowed for the goal-seek driver.
   * @throws Error - Rejects malformed `model_json`, invalid target or driver period identifiers, exactly one supplied bound, missing target or driver nodes or periods, non-finite or unordered bounds, model-evaluation or solver-convergence failures, or failure to serialize the result or updated model.
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
   * @returns Ranked DCF-assumption impacts on enterprise value.
   * @param modelJson - Financial-model specification JSON.
   * @param wacc - Baseline weighted average cost of capital in decimal form (0.10 = 10%).
   * @param terminalValueJson - Terminal-value spec JSON selecting whether growth or the exit multiple is shocked.
   * @param ufcfNode - Node identifier holding unlevered free cash flow for the forecast periods.
   * @param netDebtOverride - Optional net debt in model currency; otherwise requires debt and cash in that currency from a period ending on or before valuation.
   * @param waccSensitivityBump - Absolute shock applied to WACC and to the terminal growth rate, in decimal (0.01 = +/-100 bp).
   * @param waccDenominatorEpsilon - Minimum spread preserved between WACC and the terminal growth rate so 1/(wacc - g) stays defined, in decimal.
   * @param maxStableGrowthRate - Maximum perpetual stable growth rate; omitted uses the canonical 5% default.
   * @param exitMultipleBump - Absolute shock applied to an exit multiple, in turns of the multiple (1.0 = +/-1.0x).
   * @param midYearConvention - Whether every DCF re-run uses the mid-year discounting convention.
   * @param marketJson - Optional canonical market-context JSON used for statement evaluation, not WACC discounting.
   * @throws Error - Rejects malformed model or terminal-value JSON, model-evaluation failures, a missing UFCF series or model currency, inconsistent WACC or terminal-value assumptions, missing bridge inputs, valuation failures, or failure to serialize the sensitivity result.
   */
  dcfSensitivity(
    modelJson: string,
    wacc: number,
    terminalValueJson: string,
    ufcfNode: string,
    netDebtOverride?: number | null,
    waccSensitivityBump?: number | null,
    waccDenominatorEpsilon?: number | null,
    maxStableGrowthRate?: number | null,
    exitMultipleBump?: number | null,
    midYearConvention?: boolean | null,
    marketJson?: string | null
  ): DcfSensitivityResult;
  /**
   * Evaluate a leveraged-buyout transaction against a statement model.
   *
   * Entry enterprise value is priced at the model's first period, the sponsor
   * equity check is solved as the sources-and-uses residual, and exit proceeds
   * are the exit enterprise value less the modelled net debt at the exit
   * period. IRR is out of scope: pair the returned `exit_equity_proceeds` with
   * the equity outflow at close and call `portfolio.mwrXirr`.
   * @returns Leveraged-buyout evaluation result against the statement model.
   * @param modelJson - Financial-model specification JSON.
   * @param entryMultiple - Entry valuation multiple applied to the entry metric (8.5 = 8.5x).
   * @param entryMetricNode - Monetary node in model currency supplying the entry metric at the first period.
   * @param exitMultiple - Exit valuation multiple applied to the exit metric (9.5 = 9.5x).
   * @param exitMetricNode - Monetary node in model currency supplying the exit metric at the exit period.
   * @param exitNetDebtNode - Monetary node in model currency supplying net debt at the exit period.
   * @param exitPeriod - Model period label at which the sponsor exits, e.g. "2029".
   * @param sourcesJson - Canonical JSON array of funded debt tranches at close, each {"name", "amount"} in the model currency.
   * @param transactionFees - Transaction fees and expenses funded at close, in the model currency.
   * @throws Error - Rejects malformed model or tranche JSON, an invalid `exit_period`, model evaluation or lookup failures, a missing model currency or period, non-finite transaction inputs or model values, negative tranche amounts, a non-positive sponsor equity check, check-suite failures, or failure to serialize the result to JavaScript. The result is a structured JavaScript object.
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
  ): LboResult;
  /**
   * Weighted-average cost of capital (WACC).
   *
   * Blends the required return on equity with the after-tax cost of debt:
   * `WACC = w_E * r_E + w_D * r_D * (1 - T)`.
   * @returns Returns the blended discount rate as a decimal fraction.
   * @param equityWeight - Equity share of total capital as a decimal fraction (0.6 = 60% equity-funded).
   * @param costOfEquity - Required return on equity in decimal form, typically from CAPM (0.115 = 11.5%).
   * @param debtWeight - Debt share of total capital as a decimal fraction; must sum with the equity weight to 1.0.
   * @param costOfDebt - Pre-tax marginal borrowing yield in decimal form, before the interest tax shield (0.06 = 6%).
   * @param taxRate - Marginal corporate tax rate as a decimal fraction in [0, 1] (0.25 = 25%).
   * @throws Error - Rejects any non-finite input, negative capital weights, weights that do not sum to one within tolerance, or a `tax_rate` outside `[0, 1]`.
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
   * @returns ASCII dependency tree for the selected node.
   * @param modelJson - Financial-model specification JSON.
   * @param nodeId - Stable node identifier used to select the required domain object.
   * @throws Error - Rejects malformed `model_json`, formulas or clauses whose dependencies cannot be parsed, unknown formula references, a missing `node_id` or reachable dependency, or a dependency cycle.
   */
  traceDependencies(modelJson: string, nodeId: string): string;
  /**
   * Explain a formula for a specific node and period (JSON in/out).
   * @returns Structured formula breakdown for the selected node and period.
   * @param modelJson - Financial-model specification JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param nodeId - Stable node identifier used to select the required domain object.
   * @param period - Model period label for the requested statement value or calculation.
   * @throws Error - Rejects malformed model or result JSON, an invalid `period` identifier, a missing model node or node-period result, an invalid formula used to build the breakdown, or failure to serialize the explanation to JavaScript.
   */
  explainFormula(
    modelJson: string,
    resultsJson: string,
    nodeId: string,
    period: string
  ): FormulaExplanationJson;
  /**
   * Explain a formula for a specific node and period as formatted text.
   * @returns Formatted formula explanation for the selected node and period.
   * @param modelJson - Financial-model specification JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param nodeId - Stable node identifier used to select the required domain object.
   * @param period - Model period label for the requested statement value or calculation.
   * @throws Error - Rejects malformed model or result JSON, an invalid `period` identifier, a missing model node or node-period result, or an invalid formula used to build the explanation breakdown.
   */
  explainFormulaText(
    modelJson: string,
    resultsJson: string,
    nodeId: string,
    period: string
  ): string;
  /**
   * Generate a P&L summary report as formatted text.
   * @returns Returns a human-readable text report, not JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param lineItems - Ordered statement line-item definitions included in the summary report.
   * @param periods - Ordered period labels or observations aligned with the supplied data.
   * @throws Error - Rejects malformed `results_json`, `line_items` or `periods` values that are not JavaScript string arrays, or any period string that is not a valid statement period identifier.
   */
  plSummaryReportText(resultsJson: string, lineItems: string[], periods: string[]): string;
  /**
   * Generate a credit assessment report as formatted text.
   * @returns Returns a human-readable text report, not JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param period - Statement period identifier, such as `2025Q4` or `2025A`.
   * @throws Error - Rejects malformed `results_json` or an `period` value that is not a valid statement period identifier.
   */
  creditAssessmentReportText(resultsJson: string, period: string): string;
  /**
   * Compute a credit assessment from statement results (JSON in/out).
   * @returns Credit-assessment result object from the statement results.
   * @param resultsJson - Evaluated statement-result JSON.
   * @param period - Statement period identifier, such as `2025Q4` or `2025A`.
   * @throws Error - Rejects malformed `results_json`, an `period` value that is not a valid statement period identifier, or failure to serialize the assessment to JavaScript.
   */
  creditAssessment(resultsJson: string, period: string): Record<string, unknown>;
  /**
   * Run checks from a suite spec against a model.
   *
   * Evaluates the model only when results are absent, then runs built-in and
   * formula checks against the canonical statement results.
   * @param modelJson - Financial-model specification JSON.
   * @param suiteSpecJson - Check-suite specification JSON.
   * @param resultsJson - Evaluated statement-result JSON.
   * @returns Structured check report with individual results and aggregate summary.
   * @throws Error - Rejects malformed model, suite, or supplied result JSON; check-suite resolution failures; model-evaluation failures when results are omitted; missing nodes, incompatible data, or invalid check configuration during execution; or failure to convert the report to JavaScript.
   */
  runChecks(modelJson: string, suiteSpecJson: string, resultsJson?: string | null): CheckReport;
  /**
   * Run three-statement checks using node mappings.
   *
   * Accepts a model and mapping JSON, builds the appropriate suite, and
   * evaluates the model only when results are absent.
   * @param modelJson - Financial-model specification JSON.
   * @param mappingJson - Node-mapping JSON from statement nodes to check inputs.
   * @param resultsJson - Evaluated statement-result JSON.
   * @returns Structured three-statement check report with results and aggregate summary.
   * @throws Error - Rejects malformed model, mapping, or supplied result JSON; model-evaluation failures when results are omitted; missing mapped nodes, incompatible data, or invalid check configuration; or failure to convert the report to JavaScript.
   */
  runThreeStatementChecks(
    modelJson: string,
    mappingJson: string,
    resultsJson?: string | null
  ): CheckReport;
  /**
   * Run credit underwriting checks using credit-specific mappings.
   * @param modelJson - Financial-model specification JSON.
   * @param mappingJson - Node-mapping JSON from statement nodes to check inputs.
   * @param resultsJson - Evaluated statement-result JSON.
   * @returns Structured credit-underwriting check report with results and aggregate summary.
   * @throws Error - Rejects malformed model, mapping, or supplied result JSON; model-evaluation failures when results are omitted; missing mapped nodes, incompatible data, or invalid check configuration; or failure to convert the report to JavaScript.
   */
  runCreditUnderwritingChecks(
    modelJson: string,
    mappingJson: string,
    resultsJson?: string | null
  ): CheckReport;
  /**
   * Render a check report as plain text.
   * @returns Plain-text check report.
   * @param reportJson - Check-report JSON.
   * @throws Error - Rejects `report_json` when it is malformed or incompatible with the check report schema.
   */
  renderCheckReportText(reportJson: string): string;
  /**
   * Render a check report as HTML.
   * @returns HTML check report.
   * @param reportJson - Check-report JSON.
   * @throws Error - Rejects `report_json` when it is malformed or incompatible with the check report schema.
   */
  renderCheckReportHtml(reportJson: string): string;
  // Comps — comparable company analysis
  /**
   * Percentile rank of `value` within `data` on a 0-1 scale (Rust `percentile_rank(values, value)` argument order).
   *
   * Returns `undefined` when `data` is empty rather than a synthetic 0.5.
   * @returns Percentile rank in `[0, 1]`, or `undefined` when `data` is empty.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @param value - Subject-company metric value to rank against the peer sample.
   * @throws Error - Rejects when `data` is not a numeric JavaScript array or the finite rank cannot be serialized. Empty/non-finite peer data or a non-finite `value` return `undefined` rather than rejecting.
   */
  percentileRank(data: number[], value: number): number | undefined;
  /**
   * Z-score of `value` within `data` (Rust `z_score(values, value)` argument order).
   *
   * Returns `undefined` when fewer than two observations are provided or the
   * peer variance is zero, instead of a synthetic zero.
   * @returns Standardized z-score, or `undefined` when variance is zero or the sample is too small.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @param value - Subject-company metric value to standardize against the peer sample.
   * @throws Error - Rejects when `data` is not a numeric JavaScript array or the computed score cannot be serialized. Insufficient data, zero variance, or a non-finite `value` return `undefined` rather than rejecting.
   */
  zScore(data: number[], value: number): number | undefined;
  /**
   * Descriptive statistics over a peer distribution.
   *
   * Returns `undefined` (matching the other comps helpers) when `data` is empty.
   * @returns Descriptive peer statistics, or `undefined` when `data` is empty.
   * @param data - Non-empty numeric observation array used by the requested statistic.
   * @throws Error - Rejects when `data` is not a numeric JavaScript array or the statistics cannot be serialized. No finite observations return `undefined`.
   */
  peerStats(data: number[]): PeerStatsJson | undefined;
  /**
   * Single-factor OLS fit of `y` on `x` evaluated at the subject observation.
   * @returns Fitted intercept, slope, R², subject fitted value and residual, or `undefined` if unidentifiable.
   * @param xValues - Comparable-company independent-variable values aligned with y_values.
   * @param yValues - Comparable-company dependent-variable values aligned with x_values.
   * @param subjectX - Subject company's independent-variable value for the fitted regression.
   * @param subjectY - Subject company's observed dependent-variable value for relative-value comparison.
   * @throws Error - Rejects when `x_values` or `y_values` is not a numeric JavaScript array, or the regression result cannot be serialized. Fewer than three paired values or an unidentifiable fit returns `undefined`, as do unequal lengths and non-finite numeric inputs or outputs.
   */
  regressionFairValue(
    xValues: number[],
    yValues: number[],
    subjectX: number,
    subjectY: number
  ): RegressionResultJson | undefined;
  /**
   * Compute a canonical valuation multiple for a company-metric bag.
   * @returns The requested multiple, or `undefined` when inputs are missing or the denominator is not positive.
   * @param companyMetrics - Company financial-metric object supplying numerator and denominator inputs.
   * @param multiple - Supported valuation multiple identifier, such as EV/EBITDA or P/E.
   * @throws Error - Rejects when `company_metrics` is not a string-to-number JavaScript object, `multiple` is not a supported canonical identifier, or the computed value cannot be serialized. Missing or non-finite inputs and non-positive denominators return `undefined`.
   */
  computeMultiple(companyMetrics: unknown, multiple: string): number | undefined;
  /**
   * Composite rich/cheap scoring across multiple dimensions.
   * @returns Composite rich/cheap score with per-dimension diagnostics.
   * @param peerSet - Comparable-company metric records used to score relative value.
   * @param dimensions - Metric dimensions and weights; each has one optional `x_extractor` for single-factor regression, or null for distribution scoring.
   * @throws Error - Rejects when `peer_set` or `dimensions` cannot be decoded into its declared schema, when no scoring dimensions are supplied, or when the result cannot be serialized to JavaScript.
   */
  scoreRelativeValue(peerSet: unknown, dimensions: unknown[]): RelativeValueResultJson;
}

/**
 * Namespaced TypeScript entry point for statements analytics APIs.
 */
export declare const statements_analytics: StatementsAnalyticsNamespace;

// --- portfolio -------------------------------------------------------------

/**
 * Revalued result and application report after a scenario.
 */
export interface ScenarioRevalueResult {
  /**
   * Revalued portfolio or instrument result after applying the scenario.
   */
  valuation: Record<string, unknown>;
  /**
   * Scenario application report describing effects applied and any warnings.
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
   * Scenario application report describing effects applied and any warnings.
   */
  report: Record<string, unknown>;
}

/**
 * First-order factor sensitivity matrix.
 *
 * `JSON.stringify` this value to feed it back into `decomposeFactorRisk`,
 * which takes the canonical JSON string.
 */
export interface SensitivityMatrixResult {
  /**
   * ISO reporting currency for all monetary sensitivity entries.
   */
  base_currency: string;
  /**
   * Ordered position identifiers, one per row of `data`.
   */
  position_ids: string[];
  /**
   * Ordered factor identifiers, one per column of `data`.
   */
  factor_ids: string[];
  /**
   * Row-major sensitivity matrix, `data[position][factor]`.
   */
  data: number[][];
}

/**
 * Repriced scenario P&L profile for one shocked factor.
 */
export interface FactorPnlProfile {
  /**
   * ISO reporting currency for all P&L entries.
   */
  base_currency: string;
  /**
   * Ordered position identifiers indexing each P&L row.
   */
  position_ids: string[];
  /**
   * Shocked factor identifier.
   */
  factor_id: string;
  /**
   * Scenario shift coordinates applied to the factor.
   */
  shifts: number[];
  /**
   * P&L rows indexed as `[shift_idx][position_idx]`.
   */
  position_pnls: number[][];
}

/**
 * Factor-level risk contribution row.
 */
export interface FactorRiskContribution {
  /**
   * Factor identifier.
   */
  factor_id: string;
  /**
   * Absolute risk attributed to the factor.
   */
  absolute_risk: number;
  /**
   * Share of total risk attributed to the factor.
   */
  relative_risk: number;
  /**
   * Marginal risk of the factor.
   */
  marginal_risk: number;
}

/**
 * Position x factor risk contribution row.
 */
export interface PositionFactorRiskContribution {
  /**
   * Position identifier.
   */
  position_id: string;
  /**
   * Factor identifier.
   */
  factor_id: string;
  /**
   * Risk contribution of this position/factor pair.
   */
  risk_contribution: number;
}

/**
 * Parametric (covariance-based) Euler risk decomposition.
 */
export interface FactorRiskDecomposition {
  /**
   * Total portfolio risk under the selected measure.
   */
  total_risk: number;
  /**
   * Canonical serde name of the risk measure, e.g. `"variance"`.
   */
  measure: string;
  /**
   * Residual (idiosyncratic) risk not attributed to any factor.
   */
  residual_risk: number;
  /**
   * Factor-level contributions.
   */
  factor_contributions: FactorRiskContribution[];
  /**
   * Position x factor contributions.
   */
  position_factor_contributions: PositionFactorRiskContribution[];
  /**
   * Per-position residual (idiosyncratic) variance contributions. Empty for
   * the parametric decomposer; populated only by credit-aware position
   * decomposers.
   */
  position_residual_contributions: {
    position_id: string;
    residual_variance: number;
    source: { kind: string; [key: string]: unknown };
  }[];
}

/**
 * Per-position VaR contribution row.
 */
export interface PositionVarContribution {
  /**
   * Position identifier.
   */
  position_id: string;
  /**
   * Component VaR allocated to the position.
   */
  component_var: number;
  /**
   * Marginal VaR, when the engine computed one.
   */
  marginal_var?: number | null;
  /**
   * Fraction of total VaR contributed by this position.
   */
  pct_contribution: number;
  /**
   * Incremental VaR, when the engine computed one.
   */
  incremental_var?: number | null;
}

/**
 * Position-level VaR decomposition.
 */
export interface VarDecompositionResult {
  /**
   * Total portfolio VaR.
   */
  portfolio_var: number;
  /**
   * Total portfolio Expected Shortfall.
   */
  portfolio_es: number;
  /**
   * Confidence level used for VaR.
   */
  confidence: number;
  /**
   * Number of positions in the decomposition.
   */
  n_positions: number;
  /**
   * Euler residual, when computed by the engine.
   */
  euler_residual?: number | null;
  /**
   * Per-position VaR contributions.
   */
  contributions: PositionVarContribution[];
}

/**
 * Per-position Expected Shortfall contribution row.
 */
export interface PositionEsContribution {
  /**
   * Position identifier.
   */
  position_id: string;
  /**
   * Component ES allocated to the position.
   */
  component_es: number;
  /**
   * Marginal ES, when the engine computed one.
   */
  marginal_es?: number | null;
  /**
   * Fraction of total ES contributed by this position.
   */
  pct_contribution: number;
}

/**
 * Position-level Expected Shortfall decomposition.
 */
export interface EsDecompositionResult {
  /**
   * Total portfolio VaR.
   */
  portfolio_var: number;
  /**
   * Total portfolio Expected Shortfall.
   */
  portfolio_es: number;
  /**
   * Confidence level used for ES.
   */
  confidence: number;
  /**
   * Number of positions in the decomposition.
   */
  n_positions: number;
  /**
   * Per-position ES contributions.
   */
  contributions: PositionEsContribution[];
}

/**
 * Per-position risk-budget row.
 */
export interface PositionBudgetEntry {
  /**
   * Position identifier.
   */
  position_id: string;
  /**
   * Actual component VaR.
   */
  actual_component_var: number;
  /**
   * Target component VaR.
   */
  target_component_var: number;
  /**
   * Target share of portfolio VaR.
   */
  target_pct: number;
  /**
   * Actual-to-target utilization ratio.
   */
  utilization: number;
  /**
   * Over-budget amount.
   */
  excess: number;
  /**
   * Whether utilization exceeds the configured threshold.
   */
  breach: boolean;
}

/**
 * Risk-budget evaluation across positions.
 */
export interface RiskBudgetResult {
  /**
   * Portfolio VaR used for target scaling.
   */
  portfolio_var: number;
  /**
   * Sum of over-budget amounts.
   */
  total_overbudget: number;
  /**
   * Whether any position breached the utilization threshold.
   */
  has_breach: boolean;
  /**
   * Utilization threshold used for breach classification.
   */
  utilization_threshold: number;
  /**
   * Per-position budget rows.
   */
  positions: PositionBudgetEntry[];
}

/**
 * Bangia, Diebold, Schuermann & Stroughair (1999) liquidity-adjusted VaR.
 *
 * Field-for-field identical to the Python binding's dict.
 */
export interface LvarBangiaResult {
  /**
   * Input VaR, echoed back (non-positive loss number).
   */
  var: number;
  /**
   * Non-negative magnitude of the Bangia spread-cost add-on.
   */
  spread_cost: number;
  /**
   * Bangia-adjusted LVaR; `lvar <= var <= 0`.
   */
  lvar: number;
  /**
   * Ratio `lvar / var`; `NaN` when `var` is zero.
   */
  lvar_ratio: number;
}

/**
 * Almgren-Chriss (2001) market-impact decomposition.
 *
 * Field-for-field identical to the Python binding's dict.
 */
export interface ImpactEstimate {
  /**
   * Permanent market impact in model cost units.
   */
  permanent_impact: number;
  /**
   * Temporary market impact in model cost units.
   */
  temporary_impact: number;
  /**
   * Total expected execution cost.
   */
  total_cost: number;
  /**
   * Expected cost in basis points.
   */
  cost_bp: number;
  /**
   * Timing-risk standard deviation of execution cost, in cost units.
   */
  execution_risk: number;
}

/**
 * Browser-native materialization input accepted without Node.js APIs.
 */
export type MaterializationBundleInput = string | Uint8Array;

/**
 * Typed error thrown when a persisted contract cannot be loaded.
 */
export interface ContractValidationError extends Error {
  /**
   * Stable public error class name.
   */
  name: 'ContractValidationError';
  /**
   * Typed Rust contract-error variant in snake_case.
   */
  kind:
    | 'unsupported_version'
    | 'missing_version'
    | 'malformed_schema'
    | 'limit_exceeded'
    | 'report'
    | 'core'
    | 'contract';
  /**
   * Structured diagnostics when `kind` is `"report"`.
   */
  report?: ValidationReport;
}

/**
 * Successful strict materialization result.
 */
export interface PortfolioMaterializationResult {
  /**
   * Reusable WebAssembly portfolio handle.
   */
  portfolio: Portfolio;
  /**
   * Structured counts, diagnostics, cache hits, and phase timings.
   */
  report: MaterializationReport;
}

/**
 * Reusable bounded cache of decoded content-addressed instruments.
 *
 * @example
 * ```typescript
 * const cache = new portfolio.InstrumentArtifactCache(5_000);
 * console.log(cache.size);
 * cache.free();
 * ```
 */
export declare class InstrumentArtifactCache {
  /**
   * Create an empty cache with explicit bounds.
   * @param capacity - Maximum retained artifacts. Omit, `null`, or `undefined` to use the native default of 4,096.
   * @returns A reusable cache with a 64 MiB encoded-source byte bound.
   */
  constructor(capacity?: number);
  /**
   * Number of decoded artifacts currently retained.
   * @returns A non-negative entry count.
   */
  readonly size: number;
  /**
   * Cumulative number of successful cache-miss decodes.
   * @returns A non-negative decode count for this cache instance.
   */
  readonly decodeCount: number;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle afterward.
   */
  free(): void;
}

/**
 * Typed handle to a built portfolio. Construct once via
 * `Portfolio.fromSpec` and reuse it across cashflow / valuation calls to
 * skip the per-call `PortfolioSpec` parse + rebuild cost.
 */
export declare class Portfolio {
  private constructor();
  /**
   * Build a runtime portfolio from a portable embedded specification.
   * @returns A reusable portfolio handle.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @throws Error - Throws a JavaScript exception if `specJson` is malformed or does not match the portfolio schema, a position has an invalid quantity or instrument specification, or portfolio validation finds duplicate identifiers or an unknown entity reference.
   */
  static fromSpec(specJson: string): Portfolio;
  /**
   * Build a runtime portfolio from one strict persisted materialization bundle.
   * @param bundle - Complete UTF-8 materialization JSON string or `Uint8Array`.
   * @param cache - Optional reusable decoded-artifact cache created outside any timed validation region.
   * @returns An object containing the reusable portfolio and load report.
   * @throws Error - Throws `TypeError` for unsupported input types. Contract failures throw `ContractValidationError` with typed `kind` and structured `report` properties if the persisted contract is malformed, invalid, unsupported, or exceeds a resource limit.
   */
  static fromMaterialization(
    bundle: MaterializationBundleInput,
    cache?: InstrumentArtifactCache
  ): PortfolioMaterializationResult;
  /**
   * Validate a materialization bundle and return diagnostics for form UIs.
   * @param bundle - Complete UTF-8 materialization JSON string or `Uint8Array`.
   * @param cache - Reusable decoded-artifact cache used while validating.
   * @returns A materialization report whose build/index phase counters are zero, or a `ValidationReport` when the contract is invalid but still reportable.
   * @throws Error - Throws `TypeError` for unsupported input types or a structured `ContractValidationError` when validation cannot produce a report.
   */
  static validateMaterialization(
    bundle: MaterializationBundleInput,
    cache?: InstrumentArtifactCache
  ): MaterializationReport | ValidationReport;
  /**
   * Portfolio identifier.
   * @returns Stable portfolio ID.
   */
  readonly id: string;
  /**
   * ISO-8601 valuation date.
   * @returns Portfolio as-of date.
   */
  readonly asOf: string;
  /**
   * Reporting currency.
   * @returns ISO-4217 base currency code.
   */
  readonly baseCurrency: string;
  /**
   * Return the number of positions.
   * @returns Non-negative position count.
   */
  numPositions(): number;
  /**
   * Serialize the portable portfolio specification.
   * @returns Canonical JSON string.
   * @throws Error - Throws a JavaScript exception if the canonical portfolio specification cannot be serialized to JSON.
   */
  toJson(): string;
  /**
   * Release the underlying wasm heap allocation. Do not use this handle after calling `free()`.
   */
  free(): void;
}

/**
 * Namespaced TypeScript entry points for portfolio calculations and types.
 * @example
 * ```typescript
 * import init, { portfolio } from "finstack-quant-wasm";
 * await init();
 * const cache = new portfolio.InstrumentArtifactCache(32);
 * console.log(cache.size);
 * cache.free();
 * ```
 */
export interface PortfolioNamespace {
  /**
   * Reusable bounded decoded-instrument cache constructor.
   */
  InstrumentArtifactCache: typeof InstrumentArtifactCache;
  /**
   * Typed handle for cached portfolio builds.
   */
  Portfolio: typeof Portfolio;
  /**
   * Parse and validate a portfolio specification from JSON.
   *
   * Wire/validator surface: returns the re-serialized canonical JSON
   * **string**, suitable for storage or re-ingest by `Portfolio.fromSpec`.
   * @returns Canonical portfolio-specification JSON after validation.
   * @param jsonStr - Canonical JSON string to validate and re-serialize.
   * @throws Error - Throws a JavaScript exception if `jsonStr` is malformed or does not match the `PortfolioSpec` schema, or if the canonical form cannot be serialized.
   */
  parsePortfolioSpecJson(jsonStr: string): string;
  /**
   * Compute a single-period Brinson-Fachler attribution from sector JSON.
   *
   * Accepts a JSON array of `SectorPeriod` objects and returns a structured
   * `BrinsonPeriodResult` object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param sectorsJson - Sector-classification JSON.
   * @throws Error - Throws a JavaScript exception if `sectorsJson` is malformed, contains no sectors or a non-finite weight or return, portfolio or benchmark weights do not sum to one, or the result cannot be converted to a JavaScript value.
   */
  brinsonFachler(sectorsJson: string): Record<string, unknown>;
  /**
   * Compute Carino-linked multi-period Brinson attribution from period JSON.
   *
   * Accepts a JSON array of periods, where each period is an array of
   * `SectorPeriod` objects, and returns a structured `CarinoLinkedAttribution`
   * object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param periodsJson - Chronological period-result JSON array.
   * @throws Error - Throws a JavaScript exception if `periodsJson` is malformed, any period fails Brinson validation, the sequence is empty or changes sector ordering, a period return is non-finite or at most `-1`, or the result cannot be converted to a JavaScript value.
   */
  carinoLink(periodsJson: string): Record<string, unknown>;
  /**
   * Compute a single-period Campisi fixed-income attribution from JSON.
   *
   * Decomposes both sides into carry / treasury / spread / selection and
   * splits the active return into allocation plus four active component
   * effects (Campisi 2000). Returns a structured `FiAttributionResult` object;
   * `JSON.stringify` it to chain into `campisiCarinoLink` or
   * `campisiReconciliationCheck`.
   *
   * Every snapshot must use the quote-reproducing `z_spread` basis:
   * `spread_duration` is the canonical Z-spread duration, and `spread` plus
   * `delta_spread` are the matching Z-spread level and move. OAS, G-spread, or
   * discount-margin values are incompatible. The numeric JSON shape has no
   * metric IDs, so this boundary cannot detect mislabeled spread provenance.
   *
   * Throws when JSON is malformed or canonical Rust validation rejects empty
   * sides, non-finite values, invalid weights or period length, or a sector
   * present on either side has `|net sector weight| <= 1e-6 * gross absolute
   * sector weight`. Spread-basis provenance cannot be validated from numeric
   * JSON alone.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param portfolioJson - Canonical JSON array of `FiPositionSnapshot` objects describing the portfolio side on the quote-reproducing Z-spread basis; weights must sum to 1.
   * @param benchmarkJson - Canonical JSON array of `FiPositionSnapshot` objects describing the benchmark side on the quote-reproducing Z-spread basis; weights must sum to 1.
   * @param configJson - Canonical JSON `FiAttributionConfig`; `period_years` is its only field, is required (no default), and unknown keys are rejected.
   * @throws Error - Throws a JavaScript exception if any JSON input is malformed; either side is empty; a value is non-finite; weights do not sum to one; `periodYears` is not finite and positive; a sector has a zero or near-zero net weight relative to gross weight; or the result cannot be converted to a JavaScript value.
   */
  campisiAttribution(
    portfolioJson: string,
    benchmarkJson: string,
    configJson: string
  ): Record<string, unknown>;
  /**
   * Carino-link already-computed single-period Campisi results.
   *
   * Binds Rust `campisi_carino_link`. Each period carries its own
   * already-applied `period_years`, so periods of *different* lengths (e.g.
   * act/365 calendar months) link correctly here; prefer this entry point
   * whenever the periods are not all the same length. Returns a structured
   * `FiCarinoLinkedResult` object.
   *
   * Throws if no periods are supplied, sector ordering differs, a consumed
   * top-level return/effect, per-sector linked effect, or sector `total_active`
   * is non-finite, `active_return` disagrees with the portfolio-minus-benchmark
   * return, a sector `total_active` disagrees with its five effects, sector
   * effects do not reconcile to their declared top-level totals, the five
   * totals do not reconcile to `active_return` within the overflow-safe
   * scaled-L1 tolerance, a reconciliation residual is non-finite, or a return
   * is outside the Carino domain.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param periodsJson - Canonical JSON array of `FiAttributionResult` objects in chronological order, as returned by `campisiAttribution`.
   * @throws Error - Throws a JavaScript exception if `periodsJson` is malformed, the sequence is empty or changes sector ordering, a consumed value or reconciliation is non-finite or inconsistent, a return is at most `-1`, or the linked result cannot be converted to a JavaScript value.
   */
  campisiCarinoLink(periodsJson: string): Record<string, unknown>;
  /**
   * Compute per-period Campisi attributions from snapshots and Carino-link them.
   *
   * Binds Rust `campisi_carino_link_from_snapshots`. One shared config — hence
   * one shared `period_years` — is applied to every period, so this entry point
   * is only correct for equal-length periods; use `campisiCarinoLink` for
   * unequal periods. Returns a structured `FiCarinoLinkedResult` object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param periodsJson - Canonical JSON array of `FiPeriodInput` objects, each holding `portfolio` and `benchmark` arrays of `FiPositionSnapshot`.
   * @param configJson - Canonical JSON `FiAttributionConfig` applied to every period; `period_years` is its only field and is required (no default).
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, any period fails Campisi attribution validation, the computed periods fail Carino linking validation, or the result cannot be converted to a JavaScript value.
   */
  campisiCarinoLinkFromSnapshots(periodsJson: string, configJson: string): Record<string, unknown>;
  /**
   * Reconcile the five Campisi effect totals against the active return.
   *
   * Binds the Rust method `FiAttributionResult::reconciliation_check`. The
   * decomposition reconciles by construction (selection is the residual), so
   * this is a floating-point sanity gate rather than a model check; without it
   * callers must re-sum the five totals by hand. Returns a structured
   * `FiReconciliationReport` object with `total_residual`, `is_reconciled`
   * and `tolerance`.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param resultJson - Canonical JSON `FiAttributionResult` as returned by `campisiAttribution` (`JSON.stringify` its structured result); unknown fields are rejected.
   * @param tolerance - Absolute reconciliation tolerance in return units; `1e-10` suits return-space values.
   * @throws Error - Throws a JavaScript exception if `resultJson` is malformed or does not match `FiAttributionResult`, or if the reconciliation report cannot be converted to a JavaScript value.
   */
  campisiReconciliationCheck(resultJson: string, tolerance: number): Record<string, unknown>;
  /**
   * Build a duration-cell base-return table from a reference universe.
   *
   * Binds Rust `cell_returns_from_reference` (Dynkin, Hyman & Vankudre 1998,
   * Appendix B): buckets `referenceJson` into fixed-width duration cells and
   * averages each cell's member total returns, interpolating interior gaps
   * and flat-extrapolating leading/trailing gaps. Returns a structured
   * `DurationCellTable` object; `JSON.stringify` it to chain into
   * `excessReturns`.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param referenceJson - Canonical JSON array of `ReferenceReturn` objects (`duration`, `total_return`, both decimals with duration in years); must be non-empty.
   * @param baseLabel - Label identifying the resulting curve (e.g. `"UST"`), carried through to the output's `base_label` for policy visibility.
   * @param configJson - Canonical JSON `CellConfig`; `width` is its only field (cell width in years, finite and positive) and is required, with no default.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, the reference universe is empty or contains an invalid duration or return, the cell width is not finite and positive, labels collide, the grid exceeds its safety bound, or the result cannot be converted to a JavaScript value.
   */
  cellReturnsFromReference(
    referenceJson: string,
    baseLabel: string,
    configJson: string
  ): Record<string, unknown>;
  /**
   * Build a duration-cell base-return table from start/end discount curves.
   *
   * Binds Rust `cell_returns_from_curves`: each cell's base return is the
   * holding-period return of a hypothetical zero-coupon position bought at
   * the cell midpoint off `start` and revalued off `end` after
   * `horizonYears` have elapsed. Every resulting cell is observed, unlike
   * the reference-universe path in `cellReturnsFromReference`. Returns a
   * structured `DurationCellTable` object; `JSON.stringify` it to chain into
   * `excessReturns`.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param start - Discount curve observed at the start of the holding period.
   * @param end - Discount curve observed `horizonYears` later, at period end.
   * @param horizonYears - Length of the holding period, in years; must be finite and positive.
   * @param maxDuration - Upper bound of the duration grid, in years; must be finite and strictly greater than `horizonYears`.
   * @param baseLabel - Label identifying the base curve (e.g. `"UST"`, `"USD-SOFR"`), stamped into the result purely for policy visibility.
   * @param configJson - Canonical JSON `CellConfig`; `width` is its only field and is required, with no default.
   * @throws Error - Throws a JavaScript exception if `configJson` is malformed; the width, horizon, or maximum duration is invalid; a cell matures within the holding period; the grid is too large or has duplicate labels; a required discount factor is not finite and positive; or the result cannot be converted to a JavaScript value.
   */
  cellReturnsFromCurves(
    start: DiscountCurve,
    end: DiscountCurve,
    horizonYears: number,
    maxDuration: number,
    baseLabel: string,
    configJson: string
  ): Record<string, unknown>;
  /**
   * Compute duration-matched credit excess returns against a base-return table.
   *
   * Binds Rust `excess_returns` (Dynkin, Hyman & Vankudre 1998, Appendix B):
   * each position's `duration` is matched to its duration cell in
   * `tableJson` and the position's excess return is `total_return -
   * cell.base_return`, the credit-specific component of performance
   * isolated from the general level/shape move of the base curve. Returns a
   * structured `ExcessReturnResult` object with per-position and
   * portfolio-level totals.
   *
   * Throws if the table is empty or has empty/duplicate cell labels,
   * non-finite/negative/zero-width, non-ascending, or overlapping cells; any
   * position is invalid or falls in no cell (including a valid gap); or
   * position weights do not sum to one.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param positionsJson - Canonical JSON array of `ExcessReturnPosition` objects (`id`, `weight`, `duration`, `total_return`); weights must sum to 1.
   * @param tableJson - Canonical JSON `DurationCellTable`; `JSON.stringify` the structured table returned by `cellReturnsFromReference` or `cellReturnsFromCurves`.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, the cell table is invalid, a position is invalid or falls in no cell, position weights do not sum to one, or the result cannot be converted to a JavaScript value.
   */
  excessReturns(positionsJson: string, tableJson: string): Record<string, unknown>;
  /**
   * Compute a single-period hierarchical duration-cell x sector grid attribution.
   *
   * Binds Rust `grid_attribution` (Dynkin, Hyman & Vankudre 1998, Appendix
   * A): decomposes active return into a per-cell curve (positioning)
   * effect, a within-cell sector allocation effect, and a
   * security-selection residual per (cell, sector). Returns a structured
   * `GridAttributionResult` object (`JSON.stringify` it to chain into
   * `gridCarinoLink`) whose `total_curve`, `total_sector` and
   * `total_selection` sum to `active_return` to floating-point precision
   * for well-conditioned inputs; among accepted inputs, the reconciliation
   * residual grows the closer any bucket's net weight sits to the
   * near-zero-net-weight rejection boundary (see the Rust module docs for
   * measured magnitudes).
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param portfolioJson - Canonical JSON array of `GridPosition` objects (`cell`, `sector`, `weight`, `total_return`) for the portfolio side; weights must sum to 1.
   * @param benchmarkJson - Canonical JSON array of `GridPosition` objects for the benchmark side; same weight-sum requirement.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, a weight or return is non-finite, either side's weights do not sum to one, a cell or cell-sector bucket has a zero or near-zero net weight relative to gross weight, or the result cannot be converted to a JavaScript value.
   */
  gridAttribution(portfolioJson: string, benchmarkJson: string): Record<string, unknown>;
  /**
   * Carino-link multi-period hierarchical grid attribution results.
   *
   * Binds Rust `grid_carino_link` (Carino 1999): applies Carino smoothing to
   * a chronological sequence of single-period `gridAttribution` results so
   * the three top-level effects (`linked_curve`, `linked_sector`,
   * `linked_selection`) sum exactly to the geometrically compounded active
   * return. Only the three top-level effects are linked; per-cell /
   * per-(cell, sector) multi-period linking is out of scope. Returns a
   * structured `GridCarinoLinkedResult` object.
   *
   * Throws if no periods are supplied; a consumed return or top-level effect
   * is non-finite; `active_return` disagrees with the portfolio-minus-
   * benchmark return; the three effect totals do not reconcile to
   * `active_return` within the overflow-safe scaled-L1 tolerance; a return-
   * identity or reconciliation residual is non-finite; or a return is outside
   * the Carino domain.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param periodsJson - Canonical JSON array of `GridAttributionResult` objects, in chronological order; `JSON.stringify` the structured results returned by `gridAttribution`.
   * @throws Error - Throws a JavaScript exception if `periodsJson` is malformed, the sequence is empty, a consumed value is non-finite or inconsistent, a return is at most `-1`, or the linked result cannot be converted to a JavaScript value.
   */
  gridCarinoLink(periodsJson: string): Record<string, unknown>;
  /**
   * Compute Jeet-Partani (2023) factor-Brinson unified attribution.
   *
   * Binds Rust `factor_brinson_attribution`: generalizes classical
   * Brinson-Fachler allocation/selection to continuous factor exposures by
   * replacing the sector partition with a factor-exposure matrix and a
   * caller-supplied benchmark factor-return vector. Returns a structured
   * `FactorBrinsonResult` object with `allocation`, `selection`, and their
   * per-factor / per-asset breakdowns.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param inputJson - Canonical JSON `FactorBrinsonInput` with `asset_ids`, `asset_returns`, `exposures` (row-major n_assets x n_factors), `factor_names`, `portfolio_weights` and `benchmark_weights`; each weight vector must sum to 1.
   * @param factorReturns - Caller-supplied benchmark factor returns `f_b` as a `number[]` or `Float64Array`, length `input.factor_names`; the `Float64Array` returned by `analytics.constrainedLeastSquares` can be passed directly.
   * @throws Error - Throws a JavaScript exception if `inputJson` is malformed; the asset or factor sets are empty; dimensions disagree; a value is non-finite; either weight vector does not sum to one; benchmark factor completeness is outside tolerance; or the result cannot be converted to a JavaScript value.
   */
  factorBrinsonAttribution(inputJson: string, factorReturns: NumericArray): Record<string, unknown>;
  /**
   * Compute a Modified-Dietz TWRR sub-period return from period JSON.
   * @returns Sub-period time-weighted return as a decimal.
   * @param periodJson - Single-period result JSON.
   * @throws Error - Throws a JavaScript exception if `periodJson` is malformed, does not match the expected period schema, or the return is undefined (non-positive adjusted denominator, out-of-range cashflow weight, non-finite inputs).
   */
  twrrModifiedDietz(periodJson: string): number;
  /**
   * Geometrically link TWRR sub-period returns from returns JSON.
   * @returns Linked time-weighted return object, including the annualized rate over `horizonYears`.
   * @param returnsJson - Numeric return-series JSON.
   * @param horizonYears - Return-linking horizon measured in years for annualization.
   * @throws Error - Throws a JavaScript exception if `returnsJson` is malformed, the return series is invalid (non-finite sub-period return or return at most -1, non-positive compounded growth factor), or the linked result cannot be converted to a JavaScript value.
   */
  twrrLinked(returnsJson: string, horizonYears: number): Record<string, unknown>;
  /**
   * Compute money-weighted return via XIRR from dated cashflow JSON.
   * @returns Annualized money-weighted return as a decimal.
   * @param cashflowsJson - Dated cashflow JSON.
   * @throws Error - Throws a JavaScript exception if `cashflowsJson` is malformed, contains an invalid date or insufficient cash flows for XIRR, or the numerical root cannot be found.
   */
  mwrXirr(cashflowsJson: string): number;
  /**
   * Build a runtime portfolio from a JSON spec, validate, and round-trip.
   *
   * Wire/validator surface: deserializes the spec, constructs the portfolio
   * with live instruments, validates structural invariants, then
   * re-serializes the canonical JSON **string** for confirmation or
   * re-ingest.
   * @returns Canonical portfolio JSON after construction and validation.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @throws Error - Throws a JavaScript exception if `specJson` is malformed or violates the portfolio schema, a position has an invalid quantity or instrument specification, portfolio validation fails, or the round-trip form cannot be serialized.
   */
  buildPortfolioFromSpecJson(specJson: string): string;
  /**
   * Extract the total portfolio value from a JSON result.
   * @returns Total portfolio market value in the result's reporting currency.
   * @param resultJson - Result JSON produced by a prior call.
   * @throws Error - Throws a JavaScript exception if `resultJson` is malformed or does not match the `PortfolioResult` schema.
   */
  portfolioResultTotalValue(resultJson: string): number;
  /**
   * Extract a specific metric from a portfolio result JSON.
   *
   * Returns `undefined` (via `Option`) if the metric was not produced.
   * @returns The named metric value, or `undefined` when that metric was not produced.
   * @param resultJson - Result JSON produced by a prior call.
   * @param metricId - Stable metric identifier used to select the required domain object.
   * @throws Error - Throws a JavaScript exception if `resultJson` is malformed or does not match the `PortfolioResult` schema. An absent `metricId` returns `undefined`.
   */
  portfolioResultGetMetric(resultJson: string, metricId: string): number | undefined;
  /**
   * Aggregate portfolio metrics from a valuation JSON.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param valuationJson - Portfolio or instrument valuation JSON.
   * @param baseCurrency - ISO-4217 base currency in which aggregate portfolio values are reported.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, `baseCurrency` or `asOf` is invalid, valuation currency or date metadata is inconsistent, a required FX conversion is unavailable or invalid, or the metrics cannot be converted to a JavaScript value.
   */
  aggregateMetrics(
    valuationJson: string,
    baseCurrency: string,
    marketJson: string,
    asOf: string
  ): Record<string, unknown>;
  /**
   * Value a portfolio from its spec and market context.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param strictRisk - Optional; when omitted or `undefined`, defaults to `true` (fail closed when a requested risk metric fails to compute), matching Rust `PortfolioValuationOptions`. Pass `false` only for an intentional PV-preserving fallback.
   * @param metrics - Optional risk-metric ids to offer every position. Omit for the standard set (PV plus `dv01`; pricer-specific metrics such as `theta` or `cs01` must be listed explicitly); an empty array performs PV-only valuation. Names are validated strictly against the standard `MetricId` set — an unknown name throws. The list is a menu, not a per-position request: one list is chosen for a book of mixed instrument types, so each position is asked for exactly the entries its own instrument type has a calculator for, and the rest appear on that position's `inapplicable_metrics`. Narrowing covers structural inapplicability only; `strictRisk` still governs a metric an instrument type supports but fails to compute. `priceInstrument` keeps the opposite contract and throws on a metric its instrument cannot produce. Mirrors the Python `metrics=` keyword.
   * @throws Error - Throws a JavaScript exception if the portfolio or market JSON is malformed, a requested metric name is unknown, portfolio construction or valuation fails, strict risk calculation cannot produce a requested metric, a required FX conversion is unavailable, or the valuation cannot be converted to a JavaScript value.
   */
  valuePortfolio(
    specJson: string,
    marketJson: string,
    strictRisk?: boolean,
    metrics?: string[]
  ): Record<string, unknown>;
  /**
   * Value an already-built [`Portfolio`] handle. Skips the per-call
   * `PortfolioSpec` parse + `Portfolio::from_spec` rebuild that
   * [`value_portfolio`] performs; use this when sweeping market scenarios
   * against a fixed portfolio.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param strictRisk - Optional; when omitted or `undefined`, defaults to `true` (fail closed when a requested risk metric fails to compute), matching Rust `PortfolioValuationOptions`. Pass `false` only for an intentional PV-preserving fallback.
   * @param metrics - Optional risk-metric ids to offer every position. Omit for the standard set (PV plus `dv01`; pricer-specific metrics such as `theta` or `cs01` must be listed explicitly); an empty array performs PV-only valuation. Names are validated strictly against the standard `MetricId` set — an unknown name throws instead of silently degrading to PV-only valuation. The list is a menu, not a per-position request: one list is chosen for a book of mixed instrument types, so each position is asked for exactly the entries its own instrument type has a calculator for, and the rest appear on that position's `inapplicable_metrics`. Narrowing covers structural inapplicability only; `strictRisk` still governs a metric an instrument type supports but fails to compute. `priceInstrument` keeps the opposite contract and throws on a metric its instrument cannot produce. Mirrors the Python `metrics=` keyword.
   * @throws Error - Throws a JavaScript exception if `marketJson` is malformed, a requested metric name is unknown, portfolio valuation fails, strict risk calculation cannot produce a requested metric, a required FX conversion is unavailable, or the valuation cannot be converted to a JavaScript value.
   */
  valuePortfolioBuilt(
    portfolio: Portfolio,
    marketJson: string,
    strictRisk?: boolean,
    metrics?: string[]
  ): Record<string, unknown>;
  /**
   * Aggregate the full classified cashflow ladder for a portfolio.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param allowPartial - Optional; when omitted or `undefined`, defaults to `false` (fail closed if any position fails schedule construction). Pass `true` to keep a partial ladder with issues on the result.
   * @throws Error - Throws a JavaScript exception if the portfolio or market JSON is malformed, portfolio construction fails, any position fails schedule construction while `allowPartial` is not `true`, monetary cash-flow aggregation overflows, or the aggregate cannot be converted to a JavaScript value.
   */
  aggregateFullCashflows(
    specJson: string,
    marketJson: string,
    allowPartial?: boolean
  ): Record<string, unknown>;
  /**
   * Aggregate the full classified cashflow ladder for an already-built
   * [`Portfolio`] handle.
   *
   * Skips the per-call `PortfolioSpec` parse + `Portfolio::from_spec` rebuild.
   * For batched or chained workflows (repeated cashflow builds across market
   * scenarios on the same portfolio), this is the cheap path.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param allowPartial - Optional; when omitted or `undefined`, defaults to `false` (fail closed if any position fails schedule construction). Pass `true` to keep a partial ladder with issues on the result.
   * @throws Error - Throws a JavaScript exception if `marketJson` is malformed, any position fails schedule construction while `allowPartial` is not `true`, monetary cash-flow aggregation overflows, or the aggregate cannot be converted to a JavaScript value.
   */
  aggregateFullCashflowsBuilt(
    portfolio: Portfolio,
    marketJson: string,
    allowPartial?: boolean
  ): Record<string, unknown>;
  /**
   * Apply a scenario to a portfolio and revalue.
   *
   * Returns a JS object with structured `valuation` and `report` values.
   * @returns Revalued result object and scenario application report.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param scenarioJson - Scenario specification JSON.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @throws Error - Throws a JavaScript exception if the portfolio, scenario, or market JSON is malformed; portfolio construction, scenario application, or revaluation fails; or the structured result cannot be converted to a JavaScript value.
   */
  applyScenarioAndRevalue(
    specJson: string,
    scenarioJson: string,
    marketJson: string
  ): ScenarioRevalueResult;
  /**
   * Apply a scenario to an already-built [`Portfolio`] handle and revalue.
   * Returns a JS object with structured `valuation` and `report` values.
   * @returns Revalued result object and scenario application report.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param scenarioJson - Scenario specification JSON.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @throws Error - Throws a JavaScript exception if the scenario or market JSON is malformed, scenario application or portfolio revaluation fails, or the structured result cannot be converted to a JavaScript value.
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
   * @returns Scenario-attributable P&L ladder and application report.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param scenarioJson - Canonical JSON payload representing the scenario whose profit-and-loss impact is measured.
   * @param marketJson - Canonical market-context JSON supplying the unshocked curves, quotes, and FX data used for the base leg.
   * @throws Error - Throws a JavaScript exception if the portfolio, scenario, or market JSON is malformed; portfolio construction, scenario application, or either valuation fails; valuation currencies are inconsistent; or the structured result cannot be converted to JavaScript.
   */
  scenarioPnl(specJson: string, scenarioJson: string, marketJson: string): ScenarioPnlResult;
  /**
   * Compute the profit and loss attributable to a scenario for an
   * already-built [`Portfolio`] handle.
   *
   * Values the portfolio against the unshocked market and against the
   * scenario-shocked market, and returns a JS object with structured `pnl`
   * (base-currency `total` plus `by_position`) and `report` values. Positions
   * added or removed by the scenario are zero-filled against the missing side,
   * so the drill-down always sums to the total.
   * @returns Scenario-attributable P&L ladder and application report.
   * @param portfolio - Built portfolio object whose positions and weights are used by the calculation.
   * @param scenarioJson - Canonical JSON payload representing the scenario whose profit-and-loss impact is measured.
   * @param marketJson - Canonical market-context JSON supplying the unshocked curves, quotes, and FX data used for the base leg.
   * @throws Error - Throws a JavaScript exception if the scenario or market JSON is malformed, scenario application or either valuation fails, valuation currencies are inconsistent, or the structured result cannot be converted to JavaScript.
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
   * constraints + options) and a `MarketContext` JSON, and returns a
   * structured `PortfolioOptimizationResult` object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @throws Error - Throws a JavaScript exception if either JSON input is malformed, the portfolio, objective, constraints, weighting, or missing-metric policy is invalid, a required market-dependent valuation fails, the solver cannot produce a result, or the result cannot be converted to a JavaScript value.
   */
  optimizePortfolio(specJson: string, marketJson: string): Record<string, unknown>;
  /**
   * Replay a portfolio through dated market snapshots.
   *
   * Accepts a portfolio spec, an array of dated market snapshots, and a
   * replay configuration. Returns a structured `ReplayResult` object.
   * @returns Returns a plain structured JavaScript object; `JSON.stringify` it for a canonical JSON string.
   * @param specJson - Canonical portfolio specification JSON defining positions, quantities, and base currency.
   * @param snapshotsJson - Market-snapshot JSON array.
   * @param configJson - Configuration JSON for this call.
   * @throws Error - Throws a JavaScript exception if any JSON input is malformed; the portfolio, replay configuration, or snapshot dates and ordering are invalid; valuation, attribution, or currency conversion fails; best-effort replay retains no step; or the result cannot be converted to a JavaScript value.
   */
  replayPortfolio(
    specJson: string,
    snapshotsJson: string,
    configJson: string
  ): Record<string, unknown>;
  /**
   * Compute first-order factor sensitivities and return the matrix.
   *
   * Accepts a JSON array of positions, a JSON array of `FactorDefinition`,
   * a `MarketContext` JSON, an ISO 8601 date, and an optional `BumpSizeConfig`
   * JSON.  Returns a structured object with `position_ids`, `factor_ids`, and
   * a row-major `data` matrix; `JSON.stringify` it to chain into
   * `decomposeFactorRisk`.
   * @returns Returns a structured `SensitivityMatrixResult` object.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param baseCurrency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @throws Error - Throws a JavaScript exception if `asOf` is not a valid ISO date; any JSON input is malformed; a factor definition or bump configuration is invalid or unsupported; bumping or repricing fails; or the sensitivity matrix cannot be converted to a JavaScript value.
   */
  computeFactorSensitivities(
    positionsJson: string,
    factorsJson: string,
    marketJson: string,
    asOf: string,
    baseCurrency: string,
    bumpConfigJson?: string
  ): SensitivityMatrixResult;
  /**
   * Compute first-order factor sensitivities using a pre-parsed [`Market`].
   *
   * Avoids reparsing market JSON for repeated factor analytics calls.
   * @returns Returns a structured `SensitivityMatrixResult` object.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param market - Market context or JSON payload supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param baseCurrency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @throws Error - Throws a JavaScript exception if `asOf` is not a valid ISO date; a position, factor, or bump-config JSON input is malformed; a factor definition is invalid or unsupported; bumping or repricing fails; or the sensitivity matrix cannot be converted to a JavaScript value.
   */
  computeFactorSensitivitiesWithMarket(
    positionsJson: string,
    factorsJson: string,
    market: Market,
    asOf: string,
    baseCurrency: string,
    bumpConfigJson?: string
  ): SensitivityMatrixResult;
  /**
   * Compute scenario P&L profiles via full repricing.
   *
   * Same position/factor/market inputs as `computeFactorSensitivities`, plus
   * an optional `n_scenario_points` integer. Returns a structured array with
   * one `{ factor_id, shifts, position_pnls }` entry per shocked factor.
   * @returns Returns a structured `FactorPnlProfile` array.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param baseCurrency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @param nScenarioPoints - Positive number of evenly spaced bump levels in each P-and-L profile.
   * @throws Error - Throws a JavaScript exception if `asOf` is not a valid ISO date; any JSON input is malformed; a factor, bump configuration, or scenario-point count is invalid or unsupported; bumping or repricing fails; or the profiles cannot be converted to a JavaScript value.
   */
  computePnlProfiles(
    positionsJson: string,
    factorsJson: string,
    marketJson: string,
    asOf: string,
    baseCurrency: string,
    bumpConfigJson?: string,
    nScenarioPoints?: number
  ): FactorPnlProfile[];
  /**
   * Compute scenario P&L profiles using a pre-parsed [`Market`].
   * @returns Returns a structured `FactorPnlProfile` array.
   * @param positionsJson - Canonical portfolio-positions JSON to bump and revalue.
   * @param factorsJson - Canonical factor-definition JSON identifying the market factors to shock.
   * @param market - Market context or JSON payload supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param baseCurrency - ISO reporting currency for all returned monetary exposures; missing FX throws an error.
   * @param bumpConfigJson - Canonical bump-configuration JSON defining factor shock sizes and conventions.
   * @param nScenarioPoints - Positive number of evenly spaced bump levels in each P-and-L profile.
   * @throws Error - Throws a JavaScript exception if `asOf` is not a valid ISO date; a position, factor, or bump-config JSON input is malformed; a factor or scenario-point count is invalid or unsupported; bumping or repricing fails; or the profiles cannot be converted to a JavaScript value.
   */
  computePnlProfilesWithMarket(
    positionsJson: string,
    factorsJson: string,
    market: Market,
    asOf: string,
    baseCurrency: string,
    bumpConfigJson?: string,
    nScenarioPoints?: number
  ): FactorPnlProfile[];
  /**
   * Decompose portfolio risk into factor and position contributions.
   *
   * Uses the parametric (covariance-based) Euler decomposition.  Accepts
   * a JSON sensitivity matrix (same schema as the output of
   * `computeFactorSensitivities`), a `FactorCovarianceMatrix` JSON, and an
   * optional `RiskMeasure` JSON.
   *
   * Returns a structured object with `total_risk`, `measure`, `residual_risk`,
   * `factor_contributions` (array), `position_factor_contributions` (array),
   * and `position_residual_contributions` (array; empty for the parametric
   * decomposer).
   *
   * `measure` uses the canonical serde form (`"variance"`, `"volatility"`, or
   * an object for `var` / `expected_shortfall`); the Python binding's
   * `measure` getter reports the same snake_case tag.
   * @returns Returns a structured `FactorRiskDecomposition` object.
   * @param sensitivitiesJson - Canonical factor-sensitivity result JSON; `JSON.stringify` the structured matrix returned by `computeFactorSensitivities`.
   * @param covarianceJson - Factor covariance-matrix JSON aligned with the supplied sensitivities.
   * @param riskMeasureJson - Risk-measure configuration JSON selecting the decomposition metric.
   * @throws Error - Throws a JavaScript exception if any JSON input is malformed; sensitivity dimensions or factor axes disagree; the covariance matrix or risk measure is invalid; decomposition produces invalid variance or another non-finite value; or the result cannot be converted to a JavaScript value.
   */
  decomposeFactorRisk(
    sensitivitiesJson: string,
    covarianceJson: string,
    riskMeasureJson?: string
  ): FactorRiskDecomposition;
}

/**
 * Namespaced TypeScript entry point for portfolio APIs.
 */
export declare const portfolio: PortfolioNamespace;

// --- scenarios -------------------------------------------------------------

/**
 * Non-fatal warning raised while applying a scenario.
 */
export interface ScenarioWarning {
  /**
   * Warning category raised while applying the scenario, such as a skipped or clamped effect.
   */
  kind: string;
  [key: string]: unknown;
}

/**
 * Authoritative manifest of the state changed by applied scenario effects.
 */
export interface ScenarioChangeManifest {
  /**
   * Concrete market-data targets changed by applied effects.
   */
  market_targets: unknown[];
  /**
   * Zero-based indices of portfolio instruments mutated in place.
   */
  changed_instrument_indices: number[];
  /**
   * Whether the effective valuation date changed.
   */
  as_of_changed: boolean;
  /**
   * Whether instruments were inserted, removed, or reordered.
   */
  portfolio_shape_changed: boolean;
  /**
   * Whether callers must conservatively treat every dependency as dirty.
   */
  all_dirty: boolean;
}

/**
 * Audit stamp describing the numeric mode, rounding context, and FX policy
 * under which a result was produced.
 */
export interface ResultsMeta {
  /**
   * Numeric engine mode used to produce the results.
   */
  numeric_mode: string;
  /**
   * Rounding context snapshot applied at IO boundaries.
   */
  rounding: Record<string, unknown>;
  /**
   * FX policy applied by the computing layer, when one was applied.
   */
  fx_policy_applied?: string | null;
  /**
   * Whether the producing computation ran in parallel (omitted when serial).
   */
  parallel?: boolean;
  /**
   * ISO-8601 timestamp when the result was computed.
   */
  timestamp?: string;
  /**
   * Finstack Quant library version used to produce the result.
   */
  version?: string;
  [key: string]: unknown;
}

/**
 * Per-instrument carry decomposition returned by a `time_roll_forward`
 * scenario operation.
 */
export interface RollForwardReport {
  [key: string]: unknown;
}

/**
 * Mutated market and optional model after applying a scenario.
 *
 * Mirrors the Rust `ApplicationEnvelope`: the mutated contexts cross the
 * boundary as objects, not JSON strings.
 */
export interface ScenarioApplyResult {
  /**
   * Mutated market context, as an object.
   */
  market: Record<string, unknown>;
  /**
   * Shocked canonical instrument envelopes in input order; absent when no inventory was supplied.
   */
  instruments?: Record<string, unknown>[];
  /**
   * Mutated financial model, as an object. Absent when no model was supplied.
   */
  model?: Record<string, unknown>;
  /**
   * Count of effects successfully applied. One operation can produce zero,
   * one, or many effects; inspect `changes` and `warnings` for coverage.
   */
  operations_applied: number;
  /**
   * Count of caller-supplied operations before template or hierarchy expansion.
   */
  user_operations: number;
  /**
   * Count of operations after template expansion and hierarchy resolution.
   */
  expanded_operations: number;
  /**
   * Authoritative manifest of the state changed by applied effects.
   */
  changes: ScenarioChangeManifest;
  /**
   * Non-fatal warnings produced while applying the scenario.
   */
  warnings: ScenarioWarning[];
  /**
   * Audit stamp (numeric mode, rounding context, FX policy). Omitted when absent.
   */
  meta?: ResultsMeta;
  /**
   * Roll-forward report, present only when the scenario contained a
   * `time_roll_forward` operation.
   */
  time_roll?: RollForwardReport;
}

/**
 * Mutated market after applying a market-only scenario.
 *
 * The same `ApplicationEnvelope` shape as `ScenarioApplyResult` minus `model`.
 */
export interface ScenarioApplyMarketResult {
  /**
   * Mutated market context, as an object.
   */
  market: Record<string, unknown>;
  /**
   * Shocked canonical instrument envelopes in input order; absent when no inventory was supplied.
   */
  instruments?: Record<string, unknown>[];
  /**
   * Count of effects successfully applied. One operation can produce zero,
   * one, or many effects; inspect `changes` and `warnings` for coverage.
   */
  operations_applied: number;
  /**
   * Count of caller-supplied operations before template or hierarchy expansion.
   */
  user_operations: number;
  /**
   * Count of operations after template expansion and hierarchy resolution.
   */
  expanded_operations: number;
  /**
   * Authoritative manifest of the state changed by applied effects.
   */
  changes: ScenarioChangeManifest;
  /**
   * Non-fatal warnings produced while applying the scenario to the market.
   */
  warnings: ScenarioWarning[];
  /**
   * Audit stamp (numeric mode, rounding context, FX policy). Omitted when absent.
   */
  meta?: ResultsMeta;
  /**
   * Roll-forward report, present only when the scenario contained a
   * `time_roll_forward` operation.
   */
  time_roll?: RollForwardReport;
}

/**
 * A structured scenario operation using the canonical Rust `kind` discriminator.
 */
export type ScenarioOperation = Record<string, unknown> & { kind: string };

/**
 * Validated scenario specification consumed by the scenario engine.
 */
export interface ScenarioSpec {
  /**
   * Stable scenario identifier.
   */
  id: string;
  /**
   * Optional human-readable name.
   */
  name?: string;
  /**
   * Optional human-readable description.
   */
  description?: string;
  /**
   * Ordered scenario operations.
   */
  operations: ScenarioOperation[];
  /**
   * Composition priority; lower values execute first.
   */
  priority: number;
  /**
   * Hierarchy conflict policy.
   */
  resolution_mode: 'most_specific_wins' | 'cumulative';
  /**
   * Optional ParCDS delivery. Omitted when left at the default
   * `"solve_to_par"`. `"first_order_shift"` applies delta hazard = delta spread / (1 - recovery) and reports an approximation warning.
   */
  hazard_bump_mode?: 'solve_to_par' | 'first_order_shift';
}

/**
 * Discovery metadata for one built-in historical scenario template.
 */
export interface TemplateMetadata {
  /**
   * Stable template identifier.
   */
  id: string;
  /**
   * Human-readable template name.
   */
  name: string;
  /**
   * Historical event and modeled-effects description.
   */
  description: string;
  /**
   * Primary historical event date in ISO-8601 form.
   */
  event_date: string;
  /**
   * Canonical asset-class labels affected by the scenario.
   */
  asset_classes: Array<'rates' | 'credit' | 'equity' | 'fx' | 'volatility' | 'commodity'>;
  /**
   * Freeform discovery tags.
   */
  tags: string[];
  /**
   * Scenario severity classification.
   */
  severity: 'mild' | 'moderate' | 'severe';
  /**
   * Component identifiers in deterministic build order.
   */
  components: string[];
}

/**
 * Namespaced TypeScript entry points for scenarios calculations and types.
 * @example
 * ```typescript
 * import init, { scenarios } from "finstack-quant-wasm";
 * await init();
 * console.log(scenarios.listBuiltinTemplates());
 * ```
 */
export interface ScenariosNamespace {
  /**
   * Parse and validate a scenario specification from JSON.
   *
   * Returns the validated scenario as a plain JavaScript object.
   * @returns Validated structured scenario specification.
   * @param jsonStr - Canonical JSON string to validate and re-serialize.
   * @throws Error - Rejects malformed or schema-incompatible `json_str`, a blank scenario ID, multiple time-roll operations, invalid operation identifiers or numeric fields, variant-specific operation violations, or serialization failure.
   */
  parseScenarioSpec(jsonStr: string): ScenarioSpec;
  /**
   * Compose multiple structured scenario specs into a single scenario.
   *
   * Specs are merged in priority order (lower number runs first).
   * @returns Structured composed scenario specification.
   * @param specs - Validated ScenarioSpec objects to compose in priority order.
   * @throws Error - Rejects malformed structured specs, input specs with mixed `hazard_bump_mode` values, composition that contains more than one time-roll operation, or failure to convert the composed specification.
   */
  composeScenarios(specs: ScenarioSpec[]): ScenarioSpec;
  /**
   * Validate a scenario specification JSON without executing it.
   *
   * Returns `undefined` when the spec is valid, throws on error. This mirrors
   * the Python `validate_scenario_spec` API, which returns `None` — an invalid
   * spec raises rather than returning a falsy value, so
   * `if (validateScenarioSpec(s))` is not a validity check.
   * @returns nothing; failure is reported by throwing.
   * @param jsonStr - Canonical JSON string to validate and re-serialize.
   * @throws Error - Rejects malformed or schema-incompatible `json_str`, a blank scenario ID, multiple time-roll operations, invalid operation identifiers or numeric fields, or variant-specific operation violations.
   */
  validateScenarioSpec(jsonStr: string): void;
  /**
   * List all built-in template identifiers.
   *
   * Returns a JSON array of template ID strings.
   * @returns Built-in scenario template identifiers.
   * @throws Error - Rejects if the embedded template registry cannot be parsed and validated, or if its template identifiers cannot be serialized to JavaScript.
   */
  listBuiltinTemplates(): string[];
  /**
   * Get typed metadata for all built-in templates.
   * @returns Metadata objects in deterministic registry order.
   * @throws Error - Rejects if the embedded template registry cannot be parsed and validated, or if its metadata cannot be serialized to JSON.
   */
  listBuiltinTemplateMetadata(): TemplateMetadata[];
  /**
   * Build a scenario spec from a built-in template.
   *
   * Returns a structured `ScenarioSpec`.
   * @returns Validated scenario specification from the built-in template.
   * @param templateId - Identifier of a built-in scenario template in the embedded registry.
   * @throws Error - Rejects a failure to load the embedded registry, an unknown `template_id`, a template whose resolved scenario fails validation, or failure to serialize the scenario.
   */
  buildFromTemplate(templateId: string): ScenarioSpec;
  /**
   * List component IDs for a built-in composite template.
   *
   * Returns a JS array of component ID strings.
   * @returns Component identifiers of the selected composite template.
   * @param templateId - Identifier of a built-in scenario template in the embedded registry.
   * @throws Error - Rejects a failure to load the embedded registry, an unknown `template_id`, or component identifiers that cannot be serialized to JavaScript.
   */
  listTemplateComponents(templateId: string): string[];
  /**
   * Build a specific component from a built-in composite template.
   * @returns Validated structured scenario specification for the selected component.
   * @param templateId - Identifier of a built-in scenario template in the embedded registry.
   * @param componentId - Identifier of a component within the selected composite template.
   * @throws Error - Rejects a failure to load the embedded registry, an unknown `template_id` or `component_id`, a component scenario that fails validation, or failure to serialize the scenario.
   */
  buildTemplateComponent(templateId: string, componentId: string): ScenarioSpec;
  /**
   * Build a scenario spec from fields.
   * @returns Validated structured scenario specification from the supplied fields.
   * @param id - Scenario identifier stored on the constructed spec.
   * @param operations - Structured scenario operation specifications in execution order.
   * @param name - Optional human-readable scenario name.
   * @param description - Optional human-readable description of the scenario purpose.
   * @param priority - Optional execution priority; lower values run earlier during composition. Omit for the Rust serde default (`0`), matching the Python `priority=0` keyword default.
   * @param resolutionMode - Optional hierarchy conflict policy: `"most_specific_wins"` (default) or `"cumulative"`.
   * @param hazardBumpMode - Optional ParCDS delivery: `"solve_to_par"` (default) rebootstraps par quotes; `"first_order_shift"` applies delta hazard = delta spread / (1 - recovery) and reports an approximation warning.
   * @throws Error - Rejects malformed or schema-incompatible `operations`, an unsupported `resolution_mode` or `hazard_bump_mode`, a blank scenario ID, multiple time-roll operations, invalid operation identifiers or numeric fields, variant-specific operation violations, or failure to serialize the scenario.
   */
  buildScenarioSpec(
    id: string,
    operations: ScenarioOperation[],
    name?: string,
    description?: string,
    priority?: number,
    resolutionMode?: 'most_specific_wins' | 'cumulative',
    hazardBumpMode?: 'solve_to_par' | 'first_order_shift'
  ): ScenarioSpec;
  /**
   * Apply a scenario to a market context and financial model.
   *
   * Returns a JavaScript object with `market` and `model` (the mutated
   * contexts as objects, not JSON strings), `operations_applied`,
   * `user_operations`, `expanded_operations`, `changes` (a
   * `ScenarioChangeManifest`), `warnings`, `meta` (a `ResultsMeta` audit stamp
   * carrying the numeric mode, rounding context, and FX policy; omitted when
   * absent), and `time_roll` (a `RollForwardReport`, only present when the
   * scenario contained a `time_roll_forward` operation).
   *
   * Optional instrument envelopes are copied and returned in `instruments`, in
   * input order. Instrument-scoped operations require an inventory. No holiday
   * calendar is supplied; business-day rolls use weekends-only adjustment.
   * @returns Mutated market and optional model after applying the scenario.
   * @param scenarioJson - JSON-serialized ScenarioSpec to validate and apply.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param modelJson - JSON-serialized FinancialModelSpec that scenario operations may mutate.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param instrumentsJson - Optional JSON array of canonical instrument envelopes; required for instrument shocks and returned as shocked copies in input order.
   * @throws Error - Rejects malformed scenario, market, or model JSON, an invalid ISO `as_of` date, an invalid scenario operation, missing market objects or hierarchy context, statement-model execution failures, failure to encode the mutated contexts, or failure to serialize the application envelope to JavaScript.
   */
  applyScenario(
    scenarioJson: string,
    marketJson: string,
    modelJson: string,
    asOf: string,
    instrumentsJson?: string
  ): ScenarioApplyResult;
  /**
   * Apply a scenario to a market context only (no model mutations).
   *
   * Returns the same envelope shape as [`apply_scenario`] minus `model`;
   * the same inventory and calendar rules apply.
   * @returns Mutated market after applying the scenario.
   * @param scenarioJson - JSON-serialized ScenarioSpec to validate and apply.
   * @param marketJson - Canonical market-context JSON supplying curves, quotes, and FX data.
   * @param asOf - ISO-8601 valuation date used to resolve date-dependent market data.
   * @param instrumentsJson - Optional JSON array of canonical instrument envelopes; required for instrument shocks and returned as shocked copies in input order.
   * @throws Error - Rejects malformed scenario or market JSON, an invalid ISO `as_of` date, an invalid scenario operation, missing market objects or hierarchy context, failure to encode the mutated market, or failure to serialize the application envelope to JavaScript.
   */
  applyScenarioToMarket(
    scenarioJson: string,
    marketJson: string,
    asOf: string,
    instrumentsJson?: string
  ): ScenarioApplyMarketResult;
  /**
   * Compute horizon total return under a scenario.
   *
   * Applies a scenario specification to project an instrument forward, then
   * decomposes the resulting P&L using factor-based attribution.
   *
   * @param instrumentJson - Canonical `finstack_quant.instrument/1` envelope.
   * @param marketJson - JSON-serialized `MarketContext`.
   * @param asOf - Valuation date (ISO 8601).
   * @param scenarioJson - JSON-serialized `ScenarioSpec`.
   * @param method - Attribution method: "parallel", "waterfall", "metrics_based", "taylor".
   * @param configJson - Optional FinstackConfig JSON for horizon analysis; omit to use defaults.
   * @param calendarId - Optional holiday calendar (e.g. "nyse", "target") used to business-day adjust `time_roll_forward` targets under `business_days` mode. Omit for a weekends-only calendar; unknown identifiers throw.
   * @returns The `HorizonResult` as a structured JavaScript object, matching the Python binding's typed `HorizonResult`.
   * @throws Error - Rejects malformed instrument, market, scenario, or configuration JSON; an invalid ISO `as_of` date; an unsupported attribution `method`; an unknown `calendar_id`; invalid, unsupported, or unresolved scenario operations; missing market data; pricing or attribution failures; or failure to serialize the horizon result to JavaScript.
   */
  computeHorizonReturn(
    instrumentJson: string,
    marketJson: string,
    asOf: string,
    scenarioJson: string,
    method?: string,
    configJson?: string,
    calendarId?: string
  ): Record<string, unknown>;
}

/**
 * Namespaced TypeScript entry point for scenarios APIs.
 */
export declare const scenarios: ScenariosNamespace;
