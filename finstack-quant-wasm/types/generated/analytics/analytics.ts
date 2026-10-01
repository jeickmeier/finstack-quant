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
 * - **`Act365L`** (ICMA Rule 251.1(i)(c)): the denominator depends on the
 *   coupon frequency and the enclosing `coupon_period` supplied via
 *   [`DayCountContext`]; both are required. Annual: 366 if February 29 falls
 *   in `(coupon_start, coupon_end]` (exclusive of start, inclusive of end),
 *   else 365. Non-annual: 366 if the next coupon date falls in a leap year,
 *   else 365. Partial accrual keeps the enclosing coupon's denominator.
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
 *   first/last coupons use
 *   [`act_act_isma_year_fraction_with_reference_period`].
 * - **`ActActAfb`** (QuantLib `ActualActual::AFB`): walk whole years
 *   **backwards from `end`** until the candidate is before `start`; each
 *   accepted year-step adds `1.0`. A year-step that lands on 28 February of a
 *   leap year is bumped to 29 February. The residual fraction is
 *   `days(start, residual_end) / den`, where `den` is 366 if 29 February lies
 *   in `[start, residual_end)`, else 365. No [`DayCountContext`] is required.
 * - **`Bus252`**: requires `calendar` in [`DayCountContext`]. It iterates each
 *   calendar day in the range to check business-day status, giving O(n) cost
 *   in the number of calendar days between the dates (about 11,000 iterations
 *   for a 30Y instrument).
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
 * Parses the snake_case wire values via [`std::str::FromStr`]
 * (for example, `"quarterly"` or `"semi_annual"`). The parser also accepts
 * the `"semiannual"` spelling and the pandas offset aliases `D`/`B`
 * (daily), `W` (weekly), `M`/`ME` (monthly), `Q`/`QE` (quarterly) and
 * `A`/`Y`/`YE` (annual); serde (de)serialization stays strict snake_case.
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
         * Annualized risk-free rate in decimal form (e.g. `0.02` for 2%).
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
   * Estimated beta coefficient, or [`f64::NAN`] when it is not estimable
   * together with the confidence interval.
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
   * Standard error of the beta estimate, or [`f64::NAN`] when undefined.
   */
  std_err: NonFiniteF64Wire;
  [k: string]: unknown;
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
   * Computed metric values, one per completed rolling window. An undefined
   * window (e.g. a zero-volatility Sharpe) is `NaN`; JSON carries it as the
   * `"nan"` sentinel so the series round-trips.
   */
  values: NonFiniteF64Wire[];
}
/**
 * Drawdown episode with start, valley, optional recovery, and max drawdown.
 */
export interface DrawdownEpisode {
  /**
   * Calendar days from start to end (or last observation).
   */
  duration_days: number;
  /**
   * Date when wealth recovered to the prior peak (None if still in drawdown).
   */
  end?: DateWire | null;
  /**
   * Maximum drawdown depth (negative fraction, e.g. −0.25 for a 25% loss).
   */
  max_drawdown: number;
  /**
   * Near-recovery threshold: the drawdown level at which 99% of the
   * peak-to-trough loss has been recovered (i.e. `max_drawdown * 0.01`,
   * a value slightly below zero). Useful for identifying "almost recovered"
   * drawdowns where the series is within 1% of the prior peak.
   */
  near_recovery_threshold: number;
  /**
   * Date when the drawdown began (peak date).
   */
  start: DateWire;
  /**
   * `true` when the series began already in drawdown (no observed prior
   * peak), so `start` is the first observation rather than a true peak.
   * Survival / duration analytics should treat such episodes as
   * left-censored.
   */
  truncated_at_start?: boolean;
  /**
   * Date of the maximum drawdown depth.
   */
  valley: DateWire;
  [k: string]: unknown;
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
   * R-squared of the regression, or [`f64::NAN`] when undefined.
   */
  r_squared: NonFiniteF64Wire;
  [k: string]: unknown;
}
/**
 * Lookback returns for each period horizon.
 */
export interface LookbackReturns {
  /**
   * Fiscal-year-to-date compounded return per ticker.
   */
  fytd: number[];
  /**
   * Month-to-date compounded return per ticker.
   */
  mtd: number[];
  /**
   * Quarter-to-date compounded return per ticker.
   */
  qtd: number[];
  /**
   * Ticker names aligned with every per-ticker vector below.
   */
  ticker_names: string[];
  /**
   * Year-to-date compounded return per ticker.
   */
  ytd: number[];
  [k: string]: unknown;
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
  adjusted_r_squared: NonFiniteF64Wire;
  /**
   * Annualized OLS intercept of the (possibly rf-adjusted) dependent series.
   */
  alpha: number;
  /**
   * Regression coefficients, one per factor.
   */
  betas: number[];
  /**
   * Fraction of variance explained; NaN for a constant dependent series.
   */
  r_squared: NonFiniteF64Wire;
  /**
   * Annualized residual volatility.
   */
  residual_vol: number;
  [k: string]: unknown;
}
/**
 * Central performance analytics engine.
 */
export interface Performance {
  benchmark_idx: number;
  end_idx: number;
  frequency: PeriodKind;
  price_dates: DateWire[];
  return_spans: TickerSpan[];
  returns: number[][];
  start_idx: number;
  ticker_names: string[];
}
export interface TickerSpan {
  end: number;
  start: number;
  [k: string]: unknown;
}
/**
 * Period-level aggregate statistics.
 */
export interface PeriodStats {
  /**
   * Mean of negative-return periods.
   */
  avg_loss: number;
  /**
   * Mean return across all periods.
   */
  avg_return: number;
  /**
   * Mean of positive-return periods.
   */
  avg_win: number;
  /**
   * Best single-period return.
   */
  best: number;
  /**
   * Longest streak of negative-return periods.
   */
  consecutive_losses: number;
  /**
   * Longest streak of positive-return periods.
   */
  consecutive_wins: number;
  /**
   * CPC index: `profit_factor × win_rate × payoff_ratio`.
   *
   * Not to be confused with the Common Sense Ratio
   * (`profit_factor × tail_ratio`), which is a different metric.
   */
  cpc_ratio: NonFiniteF64Wire;
  /**
   * Binary Kelly fraction: `p_win − p_loss / payoff_ratio`, with both
   * probabilities conditional on nonzero returns. Zero-return periods
   * do not affect this payoff approximation; `win_rate` uses all periods.
   */
  kelly_criterion: NonFiniteF64Wire;
  /**
   * `avg_win / |avg_loss|`. `0.0` when there are no wins; `+∞` when wins
   * exist but there are no losses.
   */
  payoff_ratio: NonFiniteF64Wire;
  /**
   * Sum of wins / sum of |losses| (gross profit / gross loss).
   * `0.0` when there are no wins; `+∞` when wins exist but there are no
   * losses.
   */
  profit_factor: NonFiniteF64Wire;
  /**
   * Fraction of periods with positive returns.
   */
  win_rate: number;
  /**
   * Worst single-period return.
   */
  worst: number;
  [k: string]: unknown;
}
/**
 * One calendar bucket of a periodic-return series.
 */
export interface PeriodicReturn {
  /**
   * Date of the last observation in the bucket (the period end).
   */
  date: DateWire;
  /**
   * Compounded simple return over the bucket as a decimal (`0.01` is 1%).
   */
  value: number;
}
/**
 * Rolling greeks output.
 */
export interface RollingGreeks {
  /**
   * Rolling alpha values.
   */
  alphas: number[];
  /**
   * Rolling beta values.
   */
  betas: number[];
  /**
   * End dates for each rolling window.
   */
  dates: DateWire[];
  [k: string]: unknown;
}
