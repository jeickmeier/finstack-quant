// Generated from the finstack-quant-analytics JSON schemas by scripts/generate-contract-types.mjs. Do not edit.

/**
 * An `f64` in the [`non_finite_f64`] wire form: a JSON number when finite,
 * otherwise one of the strings `"inf"`, `"-inf"` or `"nan"`.
 *
 * Contract types store plain `f64` fields with
 * `#[serde(with = "finstack_quant_core::wire::non_finite_f64")]` and name this
 * type in `#[schemars(with = ...)]`, so the generated schema describes the
 * sentinel strings the serializer actually writes.
 */
export type NonFiniteF64Wire = number | NonFiniteSentinel;
/**
 * Sentinel string the [`non_finite_f64`] adapter writes for a non-finite `f64`.
 */
export type NonFiniteSentinel = "inf" | "-inf" | "nan";
/**
 * Day-count convention for CAGR annualization over explicit calendar dates.
 */
export type CagrDayCount =
  | "act365_25"
  | {
      day_count: DayCount;
    };
/**
 * Supported day-count conventions with industry-standard definitions.
 *
 * Each variant implements a specific day count convention as defined by
 * ISDA, ICMA, or local market conventions. The conventions determine how
 * interest accrues between payment dates.
 *
 * The variant docs are deliberately short: they are published verbatim as the
 * `description` of each wire value in the `common/1/day_count` JSON Schema,
 * which every instrument contract inlines. Each one carries the convention
 * name, its defining rule and its standards citation. Formulas, market usage
 * and worked examples live in the sections below.
 *
 * # Standards References
 *
 * Implementations follow:
 * - **ISDA**: 2006 ISDA Definitions, Section 4.16.
 *   `docs/REFERENCES.md#isda-2006-definitions`
 * - **ICMA**: ICMA Rule Book, Rule 251. `docs/REFERENCES.md#icma-rule-book`
 * - **ISMA** (1999). "Recommendations for Accrued Interest Calculations."
 * - **SIA/PSA**: Standard Securities Calculation Methods (SIA Standard Formulas)
 * - **ISO**: ISO 20022 Day Count Fraction Codes
 *
 * # Formulas
 *
 * The 30/360 family shares one day-count formula and differs only in how the
 * day-of-month values are adjusted:
 *
 * ```text
 * Days = 360(Y₂ - Y₁) + 30(M₂ - M₁) + (D₂' - D₁')
 * year_fraction = Days / 360
 *
 * Thirty360 (SIA/PSA):
 *   D₁' = 30   if D₁ is 31 or last day of February
 *   D₂' = 30   if D₂ is 31 and D₁' = 30
 *   D₂' = 30   if D₂ is last day of Feb and D₁ is last day of Feb
 *
 * ThirtyE360 (ISDA §4.16(g)):
 *   D₁' = min(D₁, 30)
 *   D₂' = min(D₂, 30)
 *
 * ThirtyE360Isda (ISDA §4.16(h)):
 *   D₁' = 30   if D₁ is the last day of its month (incl. end of February)
 *   D₂' = 30   if D₂ is 31, or if D₂ is the last day of February and the
 *              period does not end on the termination (maturity) date
 *
 * Thirty360It (QuantLib `Thirty360::Italian`):
 *   D₁' = 30   if D₁ is 31 or (month is February and D₁ > 27)
 *   D₂' = 30   if D₂ is 31 or (month is February and D₂ > 27)
 * ```
 *
 * - **`Thirty360`, SIA/PSA versus ISDA**: this implementation follows the
 *   SIA/PSA convention, which includes a February end-of-month rule. ISDA 2006
 *   §4.16(f) specifies a slightly different set of adjustments that omit it.
 *   Both are commonly called "30/360 US", but they can produce different day
 *   counts for periods that start or end on the last day of February.
 * - **`ThirtyE360Isda` termination-date exception**: ISDA §4.16(h) keeps D₂
 *   unadjusted when the period ends on the termination date and that date is
 *   the last day of February. Set
 *   [`DayCountContext::end_is_termination_date`] for the final period to
 *   maturity; ordinary coupon periods leave it false.
 * - **`Thirty360It`** is distinct from US SIA (February EOM only when both
 *   ends are February EOM) and from 30E/360 (no February-after-27 rule).
 *
 * The actual-day conventions:
 *
 * - **`Act365L`** (ISDA, A009): requires the enclosing calculation period
 *   `coupon_period` in [`DayCountContext`]; no frequency is needed. The
 *   denominator is 366 if that period's end date is in a leap year, else
 *   365, for every coupon frequency. Partial accrual keeps the enclosing
 *   coupon's denominator.
 *   Accrual dates outside that coupon are rejected; sum separate coupon
 *   slices for calculations spanning multiple coupon periods. This is **not**
 *   ACT/ACT AFB, which uses a sub-period splitting algorithm; use
 *   [`DayCount::ActActAfb`] for AFB / Actual/Actual Euro.
 * - **`Nl365`**: counts the actual calendar days in `(start, end]` and removes
 *   every February 29 that falls in the period, so a full leap year still
 *   yields exactly 1.0.
 * - **`ActAct`** (ISDA): split the period at calendar-year boundaries, take
 *   (days in segment) / (days in that year) for each segment, and sum.
 * - **`ActActIsma`** (ICMA): determine quasi-coupon periods from the payment
 *   frequency, take (actual days) / (actual days in coupon period) for each,
 *   and sum. Requires `frequency` in [`DayCountContext`]; for irregular
 *   first/last coupons also supply the enclosing regular `coupon_period`.
 * - **`ActActAfb`** (QuantLib `ActualActual::AFB`): walk whole years
 *   **backwards from `end`** until the candidate is before `start`; each
 *   accepted year-step adds `1.0`. A year-step that lands on 28 February of a
 *   leap year is bumped to 29 February. The residual fraction is
 *   `days(start, residual_end) / den`, where `den` is 366 if 29 February lies
 *   in `[start, residual_end)`, else 365. No [`DayCountContext`] is required.
 * - **`Bus252`**: requires `calendar` in [`DayCountContext`] and counts its
 *   business days in `[start, end)` through
 *   [`HolidayCalendar::count_business_days`](crate::dates::HolidayCalendar::count_business_days).
 *   Built-in calendars answer from a cached per-year prefix count; custom
 *   calendars scan each calendar day unless they override the count. A count
 *   outside `0..=elapsed days` is an error.
 * - **`OneOne`**: inflation fixed legs compound across annual periods using
 *   this convention. Empty intervals return zero and reversed intervals are
 *   rejected, as for all day counts in this API.
 *
 * # Usage
 *
 * | Convention | Standard for |
 * | --- | --- |
 * | `Act360` | USD and EUR money markets, short-term rate derivatives (SOFR, €STR), FX swaps and forwards |
 * | `Act365F` | GBP money markets (SONIA), cable (GBP/USD) FX, some Commonwealth bond markets |
 * | `Act365L` | GBP floating-rate notes, some European bond markets |
 * | `Thirty360` | US corporate bonds, US municipal bonds, US agency debt |
 * | `ThirtyE360` | Eurobonds, international bonds, some interest rate swaps |
 * | `Nl365` | Some Canadian money-market and mortgage instruments, legacy systems that ignore leap days |
 * | `ActAct` | US Treasury bonds, USD and EUR swap fixed legs, government bonds in many markets |
 * | `ActActIsma` | International bonds with regular coupons, Eurobonds, ICMA-governed securities |
 * | `Bus252` | BRL-denominated instruments (ANBIMA), some equity derivatives and variance swaps |
 */
export type DayCount =
  | "one_one"
  | "act_360"
  | "act_365f"
  | "act_365l"
  | "30_360"
  | "30e_360"
  | "30e_360_isda"
  | "30_360_it"
  | "nl_365"
  | "act_act"
  | "act_act_isma"
  | "act_act_afb"
  | "bus_252";
/**
 * Finite JSON number in the closed interval `[0, 1]`.
 */
export type ClosedUnitIntervalF64Wire = number;
/**
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
/**
 * The metric a rolling [`DatedSeries`] carries.
 *
 * Serialized as its lowercase name (`"volatility"`, `"sortino"`,
 * `"sharpe"`, `"return"`), which hosts also use as the value-column label.
 */
export type RollingMetric = "volatility" | "sortino" | "sharpe" | "return";
/**
 * A finite numeric representation or the undefined-result sentinel `"nan"`.
 *
 * Use `f64` for an unrestricted JSON number, or a validated numeric wire type
 * such as [`super::ClosedUnitIntervalF64Wire`] to retain its range constraints.
 * Unlike [`NonFiniteF64Wire`], its JSON contract excludes infinity sentinel strings.
 */
export type FiniteOrNanWireForClosedUnitIntervalF64Wire = ClosedUnitIntervalF64Wire | NanSentinel;
/**
 * The undefined-result token accepted by [`FiniteOrNanWire`].
 */
export type NanSentinel = "nan";
/**
 * A finite numeric representation or the undefined-result sentinel `"nan"`.
 *
 * Use `f64` for an unrestricted JSON number, or a validated numeric wire type
 * such as [`super::ClosedUnitIntervalF64Wire`] to retain its range constraints.
 * Unlike [`NonFiniteF64Wire`], its JSON contract excludes infinity sentinel strings.
 */
export type FiniteOrNanWireForNonNegativeF64Wire = NonNegativeF64Wire | NanSentinel;
/**
 * Finite JSON number greater than or equal to zero.
 */
export type NonNegativeF64Wire = number;
/**
 * A finite numeric representation or the undefined-result sentinel `"nan"`.
 *
 * Use `f64` for an unrestricted JSON number, or a validated numeric wire type
 * such as [`super::ClosedUnitIntervalF64Wire`] to retain its range constraints.
 * Unlike [`NonFiniteF64Wire`], its JSON contract excludes infinity sentinel strings.
 */
export type FiniteOrNanWireFordouble = number | NanSentinel;
/**
 * Whether a panel's series are prices or simple returns.
 */
export type PanelSeriesKind = "prices" | "returns";
/**
 * Period frequency type.
 *
 * Defines the frequency of periodic schedules (cashflow rolls, return-series
 * resampling, statement reporting). Each variant carries an implied
 * "periods-per-year" used by [`PeriodKind::periods_per_year`] and by
 * downstream annualization helpers in `finstack-quant-analytics`.
 *
 * `Daily` follows the trading-day convention (252 per year), not the
 * calendar-day convention (365 per year). Use `Weekly` if you need
 * calendar-week granularity.
 *
 * Parses only the canonical snake_case wire values via [`std::str::FromStr`]:
 * `daily`, `weekly`, `monthly`, `quarterly`, `semi_annual`, and `annual`.
 * Aliases, whitespace padding and case variants are rejected.
 */
export type PeriodKind = "daily" | "quarterly" | "monthly" | "weekly" | "semi_annual" | "annual";
/**
 * How the dependent return series is interpreted in a multi-factor regression.
 */
export type ReturnKind =
  | "excess"
  | {
      total: {
        /**
         * Finite annual-effective decimal cash rate above `-1`, including
         * explicit zero (for example, `0.02` for 2% effective per year).
         */
        risk_free_rate: number;
        [k: string]: unknown;
      };
    };

/**
 * OLS beta result with standard error and 95% confidence interval.
 */
export interface BetaResult {
  /**
   * Estimated beta coefficient, or [`f64::NAN`] when fewer than two pairs or a constant benchmark makes it unidentified.
   */
  beta: NonFiniteF64Wire;
  /**
   * Lower bound of the 95% confidence interval, or [`f64::NAN`] when
   * undefined.
   */
  ci_lower: NonFiniteF64Wire;
  /**
   * Upper bound of the 95% confidence interval, or [`f64::NAN`] when
   * undefined.
   */
  ci_upper: NonFiniteF64Wire;
  /**
   * Two-sided confidence level of `ci_lower`/`ci_upper` as a decimal
   * probability (`0.95` = 95%). The interval is
   * `beta ± t_{n_obs − 2, (1 + confidence_level) / 2} × std_err`.
   */
  confidence_level: number;
  /**
   * Number of paired observations the regression used: the exact equal-length position-paired sample. Reported even when the
   * estimates are [`f64::NAN`]. Required in the serialized result.
   */
  n_obs: number;
  /**
   * Standard error of the beta estimate, or [`f64::NAN`] when undefined.
   */
  std_err: NonFiniteF64Wire;
}
/**
 * Common-sample Pearson correlation with ticker and sample provenance.
 */
export interface CorrelationResult {
  /**
   * Last included return-ending date.
   */
  end_date: DateWire;
  /**
   * Symmetric positive-semidefinite Pearson matrix, with exact unit diagonal.
   */
  matrix: number[][];
  /**
   * Number of shared return observations used for every entry.
   */
  n_obs: number;
  /**
   * First included return-ending date, excluding the initial wealth date.
   */
  start_date: DateWire;
  /**
   * Ordered unique ticker labels for both matrix axes.
   */
  tickers: string[];
}
/**
 * A dated time-series column: scalar values aligned with window-end dates.
 */
export interface DatedSeries {
  /**
   * Window-end dates aligned 1:1 with `values`.
   */
  dates: DateWire[];
  /**
   * Metric carried by `values`; hosts use its name as the value-column label.
   */
  value_column: RollingMetric;
  /**
   * Computed metric values, one per completed rolling window. Undefined
   * estimates (singleton sample risk or a zero-over-zero ratio) are `NaN`;
   * genuine zero-risk ratios can be signed infinity. JSON preserves each
   * nonfinite value with the corresponding string sentinel.
   */
  values: NonFiniteF64Wire[];
}
/**
 * Drawdown episode with start, valley, optional recovery, and max drawdown.
 */
export interface DrawdownEpisode {
  /**
   * Calendar days from peak to recovery, or last observation when open.
   */
  duration_days: number;
  /**
   * First observed recovery to zero drawdown; absent for an open episode.
   */
  end?: DateWire | null;
  /**
   * Signed decimal peak-to-valley loss in the closed interval `[-1, 0)`.
   */
  max_drawdown: number;
  /**
   * Actual valuation date of the peak immediately preceding the decline.
   */
  start: DateWire;
  /**
   * First actual observation attaining the deepest loss in the episode.
   */
  valley: DateWire;
}
/**
 * Configuration for fiscal year periods.
 *
 * Always holds a start month in `1..=12` and a start day in `1..=31`;
 * deserialization applies the same validation as [`FiscalConfig::new`].
 */
export interface FiscalConfig {
  start_day: number;
  start_month: number;
}
/**
 * Greeks (alpha, beta, R-squared, adjusted R-squared) from a single-factor regression.
 */
export interface GreeksResult {
  /**
   * Adjusted R-squared of the regression, or [`f64::NAN`] when undefined.
   */
  adjusted_r_squared: NonFiniteF64Wire;
  /**
   * Annualized Jensen alpha, or [`f64::NAN`] when the regression slope is
   * undefined.
   */
  alpha: NonFiniteF64Wire;
  /**
   * Beta (slope) of portfolio vs benchmark, or [`f64::NAN`] when undefined.
   */
  beta: NonFiniteF64Wire;
  /**
   * Number of paired observations the regression used: the exact equal-length position-paired sample. Reported even when the
   * estimates are [`f64::NAN`]. Required in the serialized result.
   */
  n_obs: number;
  /**
   * R-squared of the regression, or [`f64::NAN`] when undefined.
   */
  r_squared: NonFiniteF64Wire;
}
/**
 * Lookback returns for each period horizon.
 */
export interface LookbackReturns {
  /**
   * Explicit fiscal partition used for fiscal-year-to-date selection.
   */
  fiscal_config: FiscalConfig;
  /**
   * Fiscal-year-to-date compounded return per ticker.
   */
  fytd: NonFiniteF64Wire[];
  /**
   * Observation count and actual holding boundaries for each fiscal-year-to-date value.
   */
  fytd_samples: LookbackSample[];
  /**
   * Month-to-date compounded return per ticker.
   */
  mtd: NonFiniteF64Wire[];
  /**
   * Observation count and actual holding boundaries for each month-to-date value.
   */
  mtd_samples: LookbackSample[];
  /**
   * Quarter-to-date compounded return per ticker.
   */
  qtd: NonFiniteF64Wire[];
  /**
   * Observation count and actual holding boundaries for each quarter-to-date value.
   */
  qtd_samples: LookbackSample[];
  /**
   * Date on which every lookback window ended: the requested date, else the
   * last active date of the panel.
   */
  ref_date: DateWire;
  /**
   * Ticker names aligned with every per-ticker vector below.
   */
  ticker_names: string[];
  /**
   * Year-to-date compounded return per ticker.
   */
  ytd: NonFiniteF64Wire[];
  /**
   * Observation count and actual holding boundaries for each year-to-date value.
   */
  ytd_samples: LookbackSample[];
}
/**
 * Compact actual holding coverage for one selected lookback sample.
 */
export interface LookbackSample {
  /**
   * Actual final selected return endpoint; absent when empty.
   */
  holding_end?: DateWire | null;
  /**
   * Actual valuation immediately before the first selected return; absent when empty.
   */
  holding_start?: DateWire | null;
  /**
   * Number of original return intervals selected, rather than calendar buckets.
   */
  n_obs: number;
}
/**
 * Result of a multi-factor regression.
 */
export interface MultiFactorResult {
  /**
   * Adjusted R-squared; NaN when R-squared or residual degrees of freedom are undefined.
   *
   * ```text
   * adj_R² = 1 − (1 − R²) × (n − 1) / (n − k − 1)
   * ```
   */
  adjusted_r_squared: FiniteOrNanWireFordouble;
  /**
   * Annualized OLS intercept of the (possibly rf-adjusted) dependent series.
   */
  alpha: number;
  /**
   * Regression coefficients, one per factor.
   *
   * @minItems 1
   */
  betas: [number, ...number[]];
  /**
   * Number of observations the regression used: the common length of the
   * dependent series and every factor series. Residual degrees of freedom
   * are `n_obs − betas.len() − 1`. Required in the serialized result.
   */
  n_obs: number;
  /**
   * Fraction of variance explained; NaN for a constant dependent series.
   */
  r_squared: FiniteOrNanWireForClosedUnitIntervalF64Wire;
  /**
   * Annualized residual-df-adjusted volatility; NaN for a saturated fit.
   */
  residual_vol: FiniteOrNanWireForNonNegativeF64Wire;
}
/**
 * Central performance analytics engine.
 */
export interface Performance {
  benchmark_idx?: number | null;
  end_idx: number;
  frequency?: PeriodKind | null;
  price_dates: DateWire[];
  return_spans: TickerSpan[];
  returns: number[][];
  start_idx: number;
  ticker_names: string[];
}
export interface TickerSpan {
  end: number;
  start: number;
}
/**
 * Dated price or return panel, one series per ticker, that a Performance is built from.
 */
export interface PerformancePanel {
  benchmark_ticker?: string | null;
  dates: DateWire[];
  frequency?: PeriodKind | null;
  kind: PanelSeriesKind;
  series: (number | null)[][];
  ticker_names: string[];
}
/**
 * Period-level aggregate statistics.
 */
export interface PeriodStats {
  /**
   * Conditional negative-return mean, retaining its negative sign; undefined when absent.
   */
  avg_loss: NonFiniteF64Wire;
  /**
   * Arithmetic mean over all occupied bucket returns; undefined when empty.
   */
  avg_return: NonFiniteF64Wire;
  /**
   * Conditional positive-return mean; undefined when no positive bucket exists.
   */
  avg_win: NonFiniteF64Wire;
  /**
   * Largest bucket return, or undefined for no buckets.
   */
  best: NonFiniteF64Wire;
  /**
   * Longest strictly negative run among successive occupied buckets; observed zero interrupts it.
   */
  consecutive_losses: number;
  /**
   * Longest strictly positive run among successive occupied buckets; missing buckets do not interrupt it.
   */
  consecutive_wins: number;
  /**
   * Cardinal Profitability Construct adaptation `profit_factor * win_rate * payoff_ratio`; requires both groups.
   */
  cpc_index: NonFiniteF64Wire;
  /**
   * Signed loss-normalized binary risk fraction `p - q/payoff`, with decisive-bucket probabilities; requires both groups.
   */
  kelly_risk_fraction: NonFiniteF64Wire;
  /**
   * Number of strictly negative bucket returns.
   */
  losses: number;
  /**
   * Number of occupied aggregation buckets, including neutral returns.
   */
  n_obs: number;
  /**
   * Conditional mean gain divided by mean loss magnitude; undefined unless both groups exist.
   */
  payoff_ratio: NonFiniteF64Wire;
  /**
   * Sum of gains divided by loss magnitudes; positive infinity for gains without losses.
   */
  profit_factor: NonFiniteF64Wire;
  /**
   * Positive bucket count divided by all occupied buckets; undefined when empty.
   */
  win_rate: NonFiniteF64Wire;
  /**
   * Number of strictly positive bucket returns.
   */
  wins: number;
  /**
   * Smallest bucket return, or undefined for no buckets.
   */
  worst: NonFiniteF64Wire;
}
/**
 * One calendar bucket of a periodic-return series.
 */
export interface PeriodicReturn {
  /**
   * Last actual observed return endpoint in this bucket.
   */
  date: DateWire;
  /**
   * Finite compounded decimal simple return, at least `-1`.
   */
  value: number;
}
/**
 * Rolling greeks output.
 */
export interface RollingGreeks {
  /**
   * Annual arithmetic Jensen alpha; undefined windows serialize as `"nan"`.
   */
  alphas: FiniteOrNanWireFordouble[];
  /**
   * Dimensionless OLS beta; unavailability matches alpha at every date.
   */
  betas: FiniteOrNanWireFordouble[];
  /**
   * Strictly increasing return-ending dates, one per completed window.
   */
  dates: DateWire[];
}
