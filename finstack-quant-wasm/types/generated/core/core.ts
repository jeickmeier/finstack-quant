// Generated from the finstack-quant-core JSON schemas by scripts/generate-contract-types.mjs. Do not edit.

/**
 * Business day adjustment conventions per ISDA standards.
 *
 * Defines how dates are adjusted when they fall on non-business days
 * (weekends or holidays). Used throughout fixed income and derivatives
 * markets for determining payment dates, fixing dates, and maturity dates.
 *
 * # Standards References
 *
 * - **ISDA**: 2006 ISDA Definitions, Section 4.12
 * - **FpML**: BusinessDayConventionEnum
 * - **ISO 20022**: Business Day Convention codes
 *
 * # Default
 *
 * `BusinessDayConvention::default()` is `ModifiedFollowing`, the ISDA 2006
 * Definitions Section 4.12(c) convention used for swap and bond period dates.
 * Host bindings use it whenever a caller omits the convention.
 */
export type BusinessDayConvention =
  "unadjusted" | "following" | "modified_following" | "preceding" | "modified_preceding" | "nearest";
/**
 * ISO 4217 currency enumeration
 */
export type Currency =
  | "AED"
  | "AFN"
  | "ALL"
  | "AMD"
  | "ANG"
  | "AOA"
  | "ARS"
  | "AUD"
  | "AWG"
  | "AZN"
  | "BAM"
  | "BBD"
  | "BDT"
  | "BGN"
  | "BHD"
  | "BIF"
  | "BMD"
  | "BND"
  | "BOB"
  | "BRL"
  | "BSD"
  | "BTN"
  | "BWP"
  | "BYN"
  | "BZD"
  | "CAD"
  | "CDF"
  | "CHF"
  | "CLF"
  | "CLP"
  | "CNY"
  | "COP"
  | "CRC"
  | "CUC"
  | "CUP"
  | "CVE"
  | "CZK"
  | "DJF"
  | "DKK"
  | "DOP"
  | "DZD"
  | "EGP"
  | "ERN"
  | "ETB"
  | "EUR"
  | "FJD"
  | "FKP"
  | "GBP"
  | "GEL"
  | "GHS"
  | "GIP"
  | "GMD"
  | "GNF"
  | "GTQ"
  | "GYD"
  | "HKD"
  | "HNL"
  | "HRK"
  | "HTG"
  | "HUF"
  | "IDR"
  | "ILS"
  | "INR"
  | "IQD"
  | "IRR"
  | "ISK"
  | "JMD"
  | "JOD"
  | "JPY"
  | "KES"
  | "KGS"
  | "KHR"
  | "KMF"
  | "KPW"
  | "KRW"
  | "KWD"
  | "KYD"
  | "KZT"
  | "LAK"
  | "LBP"
  | "LKR"
  | "LRD"
  | "LSL"
  | "LYD"
  | "MAD"
  | "MDL"
  | "MGA"
  | "MKD"
  | "MMK"
  | "MNT"
  | "MOP"
  | "MRU"
  | "MUR"
  | "MVR"
  | "MWK"
  | "MXN"
  | "MYR"
  | "MZN"
  | "NAD"
  | "NGN"
  | "NIO"
  | "NOK"
  | "NPR"
  | "NZD"
  | "OMR"
  | "PAB"
  | "PEN"
  | "PGK"
  | "PHP"
  | "PKR"
  | "PLN"
  | "PYG"
  | "QAR"
  | "RON"
  | "RSD"
  | "RUB"
  | "RWF"
  | "SAR"
  | "SBD"
  | "SCR"
  | "SDG"
  | "SEK"
  | "SGD"
  | "SHP"
  | "SLE"
  | "SLL"
  | "SOS"
  | "SRD"
  | "SSP"
  | "STN"
  | "SYP"
  | "SZL"
  | "THB"
  | "TJS"
  | "TMT"
  | "TND"
  | "TOP"
  | "TRY"
  | "TTD"
  | "TWD"
  | "TZS"
  | "UAH"
  | "UGX"
  | "USD"
  | "UYU"
  | "UZS"
  | "VED"
  | "VES"
  | "VND"
  | "VUV"
  | "WST"
  | "XAF"
  | "XCD"
  | "XOF"
  | "XPF"
  | "YER"
  | "ZAR"
  | "ZMW"
  | "ZWL";
/**
 * Serializable state representation for any curve type.
 *
 * Produced when persisting market data snapshots through serde.
 */
export type CurveState =
  | {
      /**
       * Whether non-monotonic DFs are allowed (dangerous override)
       */
      allow_non_monotonic: boolean;
      /**
       * Base date
       */
      base: DateWire;
      /**
       * OIS cut-off (business days) the curve was calibrated under, if any.
       */
      calibration_ois_cutoff_days?: number | null;
      /**
       * Day count convention for discount time basis
       */
      day_count: DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Opaque FX policy stamp; see [`DiscountCurve::fx_policy`].
       */
      fx_policy?: string | null;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Minimum forward rate floor (if set)
       */
      min_forward_rate?: number | null;
      /**
       * Minimum tenor for forward rate calculations
       */
      min_forward_tenor: number;
      /**
       * Exact typed calibration replay recipe.
       */
      rate_calibration?: RateCalibrationRecipe | null;
      /**
       * Canonical source interpolation and accumulated continuous transformations.
       */
      transform?: DiscountCurveTransform | null;
      type: "discount";
    }
  | {
      /**
       * Base date
       */
      base: DateWire;
      /**
       * Day count convention
       */
      day_count: DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
       */
      fx_policy?: string | null;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Optional contractual reset/end-date boundaries.
       *
       * `None` selects fixed numeric-tenor discount-factor stepping.
       */
      projection_grid?: number[] | null;
      /**
       * Exact typed calibration replay recipe.
       */
      rate_calibration?: RateCalibrationRecipe | null;
      /**
       * Reset lag in business days
       */
      reset_lag: number;
      /**
       * Index tenor in years
       */
      tenor: number;
      /**
       * Canonical source interpolation and cumulative continuous transformations.
       */
      transform?: ForwardCurveTransform | null;
      type: "forward";
    }
  | {
      /**
       * Base date
       */
      base: DateWire;
      /**
       * Currency
       */
      currency?: Currency | null;
      /**
       * Day count convention
       */
      day_count: DayCount;
      /**
       * Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
       */
      fx_policy?: string | null;
      /**
       * Exact calibration replay inputs.
       */
      hazard_calibration?: HazardCalibrationRecipe | null;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Optional issuer
       */
      issuer?: string | null;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Par interpolation method
       */
      par_interp?: ParInterp;
      /**
       * Par spread points for reporting
       */
      par_points: [unknown, unknown][];
      /**
       * Recovery rate
       */
      recovery_rate: number;
      /**
       * Seniority
       */
      seniority?: Seniority | null;
      type: "hazard";
    }
  | {
      /**
       * Base CPI level at t=0
       */
      base_cpi: number;
      /**
       * Base date
       */
      base_date: DateWire;
      /**
       * Day count convention
       */
      day_count?: DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Indexation lag in months
       */
      indexation_lag_months?: number;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      type: "inflation";
    }
  | {
      correlations: number[];
      detachment_points: number[];
      id: Id;
      type: "base_correlation";
    }
  | {
      /**
       * Base date
       */
      base: DateWire;
      /**
       * Day count convention
       */
      day_count: DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Spot index level (volatility index curves)
       */
      spot_level?: number | null;
      /**
       * Spot price (signed price curves)
       */
      spot_price?: number | null;
      type: "price";
    }
  | {
      /**
       * Base date
       */
      base: DateWire;
      /**
       * Day count convention
       */
      day_count: DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Spot index level (volatility index curves)
       */
      spot_level?: number | null;
      /**
       * Spot price (signed price curves)
       */
      spot_price?: number | null;
      type: "vol_index";
    }
  | {
      /**
       * Base date.
       */
      base: DateWire;
      /**
       * Day count convention.
       */
      day_count: DayCount;
      /**
       * Extrapolation policy.
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier.
       */
      id: string;
      /**
       * Interpolation style.
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve.
       */
      knot_points: [unknown, unknown][];
      type: "basis_spread";
    }
  | {
      /**
       * Base date.
       */
      base_date: DateWire;
      /**
       * Day count convention.
       */
      day_count: DayCount;
      /**
       * Curve identifier.
       */
      id: string;
      /**
       * Nelson-Siegel model parameters.
       */
      model: NelsonSiegelModel;
      type: "parametric";
    };
/**
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
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
 * Extrapolation policy for evaluation outside the knot range.
 *
 * All [`InterpolationStrategy`](super::traits::InterpolationStrategy)
 * implementations honour this policy at both ends of the knot range.
 * Choose based on what behaviour the downstream pricer expects:
 *
 * - `FlatZero` (default): hold the boundary value constant. Safe for
 *   discount factors and survival probabilities.
 * - `FlatForward`: extend the boundary slope. Useful for forward-rate
 *   curves where extrapolated rates should track the local term-structure
 *   slope.
 * - `Nan`: refuse to extrapolate; return `NaN` and require the caller to
 *   detect and handle out-of-range queries explicitly. Recommended for
 *   production risk systems.
 *
 * `#[non_exhaustive]` so that future policies (e.g. linear-zero,
 * asymptotic) can be added without breaking downstream code.
 */
export type ExtrapolationPolicy = "flat_zero" | "flat_forward" | "none";
/**
 * Enum of supported interpolation styles. The default is `Linear`.
 */
export type InterpStyle = "linear" | "log_linear" | "monotone_convex" | "cubic_hermite" | "piecewise_quadratic_forward";
/**
 * Numerical method used to calibrate a rate curve.
 */
export type RateCalibrationMethod =
  | "bootstrap"
  | {
      global_solve: {
        /**
         * Whether the original solve requested its specialized Jacobian.
         */
        use_analytical_jacobian: boolean;
        [k: string]: unknown;
      };
    };
/**
 * OIS floating-leg compounding convention used during calibration.
 */
export type RateCalibrationOisCompounding =
  | "simple"
  | {
      compounded_in_arrears: {
        /**
         * Business-day lookback applied to rate observations.
         */
        lookback_days: number;
        [k: string]: unknown;
      };
    }
  | {
      compounded_with_observation_shift: {
        /**
         * Business days by which observations and accrual weights are shifted.
         */
        shift_days: number;
        [k: string]: unknown;
      };
    }
  | {
      compounded_with_rate_cutoff: {
        /**
         * Number of business days in the rate-cutoff window.
         */
        cutoff_days: number;
        [k: string]: unknown;
      };
    };
/**
 * Lossless rate quote representation used by calibration replay.
 */
export type RateCalibrationQuote =
  | {
      deposit: {
        /**
         * Referenced rate index.
         */
        index_id: Id;
        /**
         * Relative-tenor or absolute-date pillar.
         */
        pillar: RateCalibrationPillar;
        /**
         * Quoted deposit rate.
         */
        rate: number;
      };
    }
  | {
      fra: {
        /**
         * FRA end pillar.
         */
        end: RateCalibrationPillar;
        /**
         * Referenced rate index.
         */
        index_id: Id;
        /**
         * Quoted FRA rate.
         */
        rate: number;
        /**
         * FRA start pillar.
         */
        start: RateCalibrationPillar;
      };
    }
  | {
      futures: {
        /**
         * Convention-registry identifier for the futures contract.
         */
        contract: RateCalibrationFutureContractId;
        /**
         * Optional pre-computed convexity adjustment.
         */
        convexity_adjustment?: number | null;
        /**
         * Futures expiry date.
         */
        expiry: DateWire;
        /**
         * Quoted futures price.
         */
        price: number;
      };
    }
  | {
      swap: {
        /**
         * Referenced floating-rate index.
         */
        index_id: Id;
        /**
         * Relative-tenor or absolute-date maturity pillar.
         */
        pillar: RateCalibrationPillar;
        /**
         * Quoted fixed rate.
         */
        rate: number;
        /**
         * Optional floating-leg spread.
         */
        spread_decimal?: number | null;
      };
    }
  | {
      basis: {
        /**
         * Referenced projection-rate index.
         */
        index_id: Id;
        /**
         * Relative-tenor or absolute-date maturity pillar.
         */
        pillar: RateCalibrationPillar;
        /**
         * Quoted basis spread in decimal form.
         */
        spread_decimal: number;
      };
    };
/**
 * A phantom-typed identifier that prevents mixing different kinds of IDs.
 *
 * This type wraps a string identifier with a phantom type parameter to ensure
 * type safety at compile time. Different `Id<T>` types with different `T`
 * cannot be compared or mixed accidentally, preventing entire classes of bugs.
 *
 * # Type Parameters
 *
 * * `T` - Phantom type tag that distinguishes this ID from IDs with different tags
 *
 * # Invariants
 *
 * - Storage uses `Arc<str>` for efficient cloning
 * - The phantom marker has zero size and runtime cost
 * - Two `Id<T>` values are equal if their string values are equal
 * - IDs with different type tags (`Id<A>` vs `Id<B>`) cannot be compared
 *
 * # Thread Safety
 *
 * `Id<T>` is `Send + Sync` as it wraps an `Arc<str>`. Multiple threads can
 * safely share and clone IDs with minimal synchronization overhead.
 */
export type Id = string;
/**
 * Typed maturity specification retained from an original market quote.
 */
export type RateCalibrationPillar =
  | {
      tenor: Tenor;
    }
  | {
      date: DateWire;
    };
/**
 * Unit of a tenor period.
 */
export type TenorUnit = "days" | "weeks" | "months" | "years";
/**
 * Serialized identifier for an interest-rate futures convention.
 */
export type RateCalibrationFutureContractId = string;
/**
 * Role of a curve and its linked rate-curve identifier during calibration.
 */
export type RateCalibrationCurveRole =
  | {
      discount: {
        /**
         * Projection curve identifier used by the calibration instruments.
         */
        projection_curve_id: Id;
        [k: string]: unknown;
      };
    }
  | {
      projection: {
        /**
         * Discount curve identifier used by the calibration instruments.
         */
        discount_curve_id: Id;
        [k: string]: unknown;
      };
    };
/**
 * Interpolation method for reporting par spreads stored on the curve.
 *
 * Applies only to *par-spread* readouts (the spreads quoted at calibration
 * pillars), not to the underlying hazard rates. Hazard interpolation always
 * follows piecewise-constant survival. Use `LogLinear` when spreads span
 * multiple decades (e.g. high-yield issuers) so interpolation stays in
 * log-space; otherwise the default `Linear` is fine.
 */
export type ParInterp = "linear" | "log_linear";
/**
 * Seniority level for credit exposures.
 *
 * Used to tag a hazard curve with the seniority of the issuer's debt
 * observed by the curve. Drives the recovery-rate prior in default
 * modelling and selects the right LGD prior in
 * `finstack_quant_models::credit::lgd::seniority`.
 *
 * Order is **not** total — `SeniorSecured` is strictly senior to `Senior`,
 * `Subordinated`, and `Junior`, but the relative ordering of `Subordinated`
 * vs. `Junior` is jurisdiction-dependent. Do not rely on `Ord` semantics.
 */
export type Seniority = "senior_secured" | "senior" | "subordinated" | "junior";
/**
 * Nelson-Siegel model parameters.
 *
 * Stores either the 4-parameter NS or 6-parameter NSS specification.
 */
export type NelsonSiegelModel =
  | {
      /**
       * Long-term rate level.
       */
      beta0: number;
      /**
       * Short-term component.
       */
      beta1: number;
      /**
       * Medium-term hump.
       */
      beta2: number;
      /**
       * Decay factor (must be > 0).
       */
      tau: number;
      variant: "ns";
      [k: string]: unknown;
    }
  | {
      /**
       * Long-term rate level.
       */
      beta0: number;
      /**
       * Short-term component.
       */
      beta1: number;
      /**
       * Medium-term hump.
       */
      beta2: number;
      /**
       * Second hump.
       */
      beta3: number;
      /**
       * First decay factor (must be > 0).
       */
      tau1: number;
      /**
       * Second decay factor (must be > 0, ≠ τ₁).
       */
      tau2: number;
      variant: "nss";
      [k: string]: unknown;
    };
/**
 * Exact decimal encoded only as a JSON string.
 */
export type DecimalWire = string;
/**
 * Type of dividend event.
 *
 * Distinguishes between cash payments, stock distributions, and continuous
 * yield approximations used in different pricing models.
 *
 * # Variants
 *
 * - **Cash**: Actual dividend payment (quarterly, semi-annual, etc.)
 * - **Stock**: Share distribution (less common, complicates option pricing)
 * - **Yield**: Continuous approximation for analytical models
 */
export type DividendKind =
  | {
      cash: Money;
    }
  | {
      yield: number;
    }
  | {
      stock: {
        /**
         * Stock distribution ratio; 0.05 corresponds to a 5% stock dividend.
         */
        ratio: number;
        [k: string]: unknown;
      };
    };
/**
 * Standard FX conversion strategies used to hint FX providers.
 *
 * The policy tells a provider *how* the rate will be applied so it can decide
 * between spot, forward, or averaged sources.
 */
export type FxConversionPolicy = "cashflow_date" | "period_end" | "period_average";
/**
 * Interpolation method for CPI/RPI values between monthly observations.
 *
 * Determines how index values are computed for dates between published
 * monthly levels. Different markets use different conventions.
 *
 * # Market Conventions
 *
 * - **US TIPS**: Linear interpolation (daily pro-rata)
 * - **UK Index-Linked Gilts**: Linear interpolation with 3-month lag
 * - **Euro inflation bonds**: Varies by issuer (typically linear)
 */
export type InflationInterpolation = "step" | "linear";
/**
 * Contractual observation lag for inflation index reference dates.
 *
 * Inflation indices are published with a delay (typically 2-4 weeks). Securities
 * using these indices incorporate an observation lag to ensure the reference
 * index is published by settlement. This lag does not specify the actual
 * publication date; use [`InflationIndex::with_publication_dates`] for that.
 *
 * # Standard Lags by Market
 *
 * - **US TIPS**: 3-month lag (reference index from 3 months prior)
 * - **UK Index-Linked Gilts**: 3-month lag (8-month for older issues)
 * - **French OATi**: 3-month lag
 * - **German index-linked**: 3-month lag
 *
 * # Rationale
 *
 * The lag ensures:
 * 1. Reference index is published before settlement
 * 2. Index value is known at coupon payment date
 * 3. No estimation or forecasting required for payment calculation
 */
export type InflationLag =
  | {
      months: number;
    }
  | {
      days: number;
    }
  | "none";
/**
 * Single market observable that doesn't require a full curve.
 *
 * Represents point-in-time market data like spot prices, spreads, or unitless
 * parameters. Stored in [`MarketContext`](crate::market_data::context::MarketContext)
 * alongside curves for simple lookups.
 *
 * # Use Cases
 *
 * - **Spot prices**: Equity spots, commodity prices, FX spots
 * - **Recovery rates**: Credit recovery assumptions (unitless)
 * - **Correlation parameters**: Equity-FX correlation, basis correlations
 * - **Spreads**: Credit spreads, basis spreads (unitless or monetary)
 * - **Multipliers**: Beta, vega notionals, adjustment factors
 */
export type MarketScalar =
  | {
      unitless: number;
    }
  | {
      price: Money;
    };
/**
 * Numeric schema revision for contracts whose sole supported revision is v1.
 */
export type SchemaVersion = number;
/**
 * Interpolation strategy for [`ScalarTimeSeries`].
 */
export type SeriesInterpolation = "step" | "linear";
/**
 * Interpolation contract for vol surfaces.
 */
export type VolInterpolationMode = "vol" | "total_variance";
/**
 * Quoting convention of the volatilities stored on a [`VolSurface`].
 *
 * The same `vol_surface_id` channel is read by consumers with very different
 * expectations: rates calibrations typically read normal (Bachelier, absolute)
 * vols on an `expiry × tenor` ATM matrix, while equity/FX/swaption smile
 * consumers read Black (lognormal, relative) vols on `expiry × strike`. The
 * stored numbers are an order of magnitude apart (e.g. 0.008 normal vs 0.20
 * Black), so misreading one as the other silently mis-prices. Tagging the
 * quote type lets consumers enforce their convention via
 * [`VolSurface::require_quote_type`].
 */
export type VolQuoteType = "black_lognormal" | "shifted_black_lognormal" | "normal";
/**
 * Semantic meaning of the secondary axis on a [`VolSurface`].
 *
 * Most option surfaces are defined on `expiry × strike`, but some calibration
 * workflows materialize ATM matrices on `expiry × tenor`. Keeping the axis type
 * explicit prevents consumers from accidentally interpreting tenor buckets as
 * strikes.
 */
export type VolSurfaceAxis = "strike" | "tenor";
/**
 * Identifier for a Gregorian period like `2025Q1` or a fiscal period like
 * `FY2025W53`.
 */
export type PeriodId = string;
/**
 * Warning generated during schedule construction.
 *
 * Warnings indicate non-fatal issues that occurred during schedule generation.
 * Unlike errors, these allow the schedule to be created but signal that
 * something unexpected happened that callers should be aware of.
 *
 * # Use Cases
 *
 * - **Graceful fallback**: When [`ScheduleErrorPolicy::GracefulEmpty`] is set and an error
 *   would normally occur, the builder returns an empty schedule with a warning
 *   describing the original error.
 */
export type ScheduleWarning =
  | {
      graceful_fallback: {
        /**
         * Human-readable description of the error that was suppressed.
         */
        error_message: string;
      };
    }
  | {
      missing_calendar_id: {
        /**
         * The calendar identifier that could not be resolved.
         */
        calendar_id: string;
      };
    };
/**
 * Explicit policy for how schedule construction should respond to recoverable issues.
 */
export type ScheduleErrorPolicy = "strict" | "missing_calendar_warning" | "graceful_empty";
/**
 * Stub period handling when start/end dates don't align with payment frequency.
 *
 * Controls how schedules are generated when the start and end dates don't
 * divide evenly by the payment frequency, resulting in an irregular period
 * (stub) at the beginning or end of the schedule.
 *
 * # Variants
 *
 * - **`None`**: No stub allowed (default). Generates regular periods from
 *   start to end and returns an error
 *   ([`InputError::NonIntegerScheduleTenor`]) when the dates don't divide
 *   evenly by the frequency. Use a stub variant for misaligned schedules.
 *
 * [`InputError::NonIntegerScheduleTenor`]: crate::error::InputError::NonIntegerScheduleTenor
 * - **`ShortFront`**: Short stub period at the start. Schedule is built
 *   backward from the end date, creating a short first period.
 * - **`ShortBack`**: Short stub period at the end. Schedule is built forward
 *   from the start date, creating a short final period.
 * - **`LongFront`**: Long stub period at the start. Combines the first two
 *   periods into a single longer period.
 * - **`LongBack`**: Long stub period at the end. Combines the last two periods
 *   into a single longer period.
 *
 * # Financial Context
 *
 * Stub conventions are important for:
 * - Interest accrual calculations (short/long first coupons)
 * - Cash flow present value computations
 * - Matching market conventions for specific instruments
 *
 * # See Also
 *
 * - [`ScheduleBuilder::stub_rule`] to configure stub behavior
 */
export type StubKind = "none" | "short_front" | "short_back" | "long_front" | "long_back";
/**
 * Column storage variants supported by the table envelope.
 */
export type TableColumnData =
  | {
      type: "string";
      values: string[];
      [k: string]: unknown;
    }
  | {
      type: "nullable_string";
      values: (string | null)[];
      [k: string]: unknown;
    }
  | {
      type: "float64";
      values: number[];
      [k: string]: unknown;
    }
  | {
      type: "nullable_float64";
      values: (number | null)[];
      [k: string]: unknown;
    }
  | {
      type: "u_int32";
      values: number[];
      [k: string]: unknown;
    }
  | {
      type: "nullable_u_int32";
      values: (number | null)[];
      [k: string]: unknown;
    }
  | {
      type: "int64";
      values: number[];
      [k: string]: unknown;
    }
  | {
      type: "nullable_int64";
      values: (number | null)[];
      [k: string]: unknown;
    };
/**
 * Optional semantic hint for a column.
 */
export type TableColumnRole = "dimension" | "index" | "measure" | "attribute";

/**
 * Serializable state for credit index data.
 *
 * Instead of serializing `Arc<Curve>` directly, we store curve IDs that
 * reference curves present in the `MarketContextState`.
 */
export interface CreditIndexState {
  /**
   * ID of the base correlation curve (must exist in context curves)
   */
  base_correlation_curve_id: string;
  /**
   * Unique identifier for this credit index
   */
  id: string;
  /**
   * ID of the index hazard curve (must exist in context curves)
   */
  index_credit_curve_id: string;
  /**
   * Optional map of issuer ID → hazard curve ID
   */
  issuer_credit_curve_ids?: {
    [k: string]: string;
  } | null;
  /**
   * Optional map of issuer ID → recovery rate
   */
  issuer_recovery_rates?: {
    [k: string]: number;
  } | null;
  /**
   * Optional map of issuer ID → weight
   */
  issuer_weights?: {
    [k: string]: number;
  } | null;
  /**
   * Number of constituents
   */
  num_constituents: number;
  /**
   * Recovery rate
   */
  recovery_rate: number;
}
export interface CurveAdjustmentSegment {
  /**
   * Function slope on this segment.
   */
  slope: number;
  /**
   * Segment origin in the source curve's time coordinates.
   */
  start: number;
  /**
   * Right-hand function value at the segment origin.
   */
  value: number;
}
/**
 * Typed conventions required to replay a rate-curve calibration.
 */
export interface RateCalibrationRecipe {
  /**
   * Currency of the calibrated curve.
   */
  currency: Currency;
  /**
   * Day count used for the curve's time axis.
   */
  curve_day_count: DayCount;
  /**
   * Numerical calibration method.
   */
  method: RateCalibrationMethod;
  /**
   * Optional OIS floating-leg compounding override.
   */
  ois_compounding?: RateCalibrationOisCompounding | null;
  /**
   * Complete typed quote set required for exact replay.
   */
  quotes: RateCalibrationQuote[];
  /**
   * Discount/projection role and linked curve identifier.
   */
  role: RateCalibrationCurveRole;
}
/**
 * A parsed tenor representing a time period.
 *
 * Tenors are commonly used in financial markets to specify maturities,
 * payment frequencies, and rate fixing periods.
 */
export interface Tenor {
  /**
   * Number of `unit` periods in the tenor. Must be at least 1; `0` is
   * rejected because a zero-length period makes schedule generation loop.
   */
  count: number;
  /**
   * Calendar unit the count is expressed in, such as days, weeks, months,
   * or years.
   */
  unit: TenorUnit;
}
/**
 * Source interpolation and a single accumulated transformation, never a chain
 * of nested curves. The adjustment is stored as its piecewise-linear derivative
 * so evaluating beyond a completed triangular shock does not subtract large
 * quadratic polynomials.
 */
export interface DiscountCurveTransform {
  /**
   * Cumulative derivative of the additive log-discount adjustment.
   */
  adjustment: PiecewiseLinearAdjustment;
  /**
   * Current origin in the source interpolation's year-fraction coordinates.
   */
  offset: number;
  /**
   * Original, untransformed interpolation pillars.
   */
  source_points: [unknown, unknown][];
}
/**
 * A flat function representation: repeated shocks merge on a breakpoint union
 * rather than forming a recursively nested transformation history.
 */
export interface PiecewiseLinearAdjustment {
  /**
   * Constant value before the first breakpoint.
   */
  initial_value: number;
  /**
   * Sorted, merged linear segments.
   */
  segments: CurveAdjustmentSegment[];
}
export interface ForwardCurveTransform {
  /**
   * Cumulative additive rate adjustment in source time coordinates.
   */
  adjustment: PiecewiseLinearAdjustment;
  /**
   * Current curve origin in the source curve's year-fraction coordinates.
   */
  offset: number;
  /**
   * Accumulated parallel multiplicative factor on the source interpolation.
   */
  scale: number;
  /**
   * Original interpolation pillars, independent of current curve samples.
   */
  source_points: [unknown, unknown][];
}
/**
 * Exact valuation-layer inputs required to replay a hazard-curve calibration.
 *
 * The core market-data crate stores these payloads without interpreting them;
 * the calibration crate deserializes them back into its canonical
 * `HazardCurveParams`, typed CDS quotes, and `CalibrationConfig`. Keeping the
 * complete serde payloads avoids replacing date pillars with rounded tenors or
 * silently substituting current defaults for the original solver policy.
 */
export interface HazardCalibrationRecipe {
  /**
   * Exact serialized `CalibrationConfig`, including solver and validation policy.
   */
  calibration_config: {
    [k: string]: unknown;
  };
  /**
   * Original par-spread or upfront inputs used to calibrate the curve.
   */
  calibration_inputs: HazardCalibrationInput[];
  /**
   * Exact serialized `HazardCurveParams` used for the original solve.
   */
  hazard_params: {
    [k: string]: unknown;
  };
  /**
   * Par-spread-only inputs used for quote-space spread risk.
   */
  spread_risk_inputs: HazardCalibrationInput[];
  [k: string]: unknown;
}
/**
 * One atomic quote binding retained for hazard calibration replay.
 */
export interface HazardCalibrationInput {
  /**
   * Contractual pillar date resolved from the quote and CDS conventions.
   */
  pillar_date: DateWire;
  /**
   * Frozen year-fraction pillar time used by the original solve.
   */
  pillar_time: number;
  /**
   * Exact serialized typed CDS quote.
   */
  quote: {
    [k: string]: unknown;
  };
}
/**
 * A dated dividend event.
 */
export interface DividendEvent {
  /**
   * Ex-dividend date.
   */
  date: DateWire;
  /**
   * Event kind.
   */
  kind: DividendKind;
  [k: string]: unknown;
}
/**
 * Currency-tagged monetary amount with safe arithmetic.
 *
 * Values retain decimal precision independently of ISO 4217 display precision.
 *
 * When you need configurable rounding during ingestion, use
 * [`Money::new_with_config`].
 */
export interface Money {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: DecimalWire;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency: Currency;
}
/**
 * Dividend schedule for an equity or ETF underlying.
 *
 * Contains a time-ordered sequence of dividend events used for pricing equity
 * derivatives and calculating total return. The schedule can be referenced by
 * multiple instruments via its [`CurveId`] in the market context.
 *
 * # Usage in Pricing
 *
 * - **Discrete dividends**: Subtract PV of future dividends from spot for option pricing
 * - **Ex-dividend adjustments**: Reduce forward price by dividend amount
 * - **Dividend futures**: Sum dividends in contract period
 * - **Total return**: Include dividend reinvestment in performance
 */
export interface DividendSchedule {
  /**
   * Quote currency for cash dividends (optional metadata).
   */
  currency?: Currency | null;
  /**
   * Sorted events by date (ascending).
   */
  events: DividendEvent[];
  /**
   * Unique identifier of this schedule in the market context.
   */
  id: Id;
  /**
   * Optional display symbol/ticker for convenience.
   */
  underlying?: string | null;
}
/**
 * Configuration for [`FxMatrix`](crate::money::fx::FxMatrix) behaviour.
 *
 * Controls triangulation and caching.
 */
export interface FxConfig {
  /**
   * Maximum number of provider-observed quotes retained in the LRU cache.
   */
  cache_capacity?: number;
  /**
   * Whether to enable automatic triangulation for missing rates.
   *
   * When enabled, the matrix will attempt `from -> pivot -> to` and will not
   * search multi-hop paths or alternative pivots.
   */
  enable_triangulation?: boolean;
  /**
   * Pivot currency for triangulation fallback (typically USD).
   *
   * Current limitation: triangulation uses this single configured pivot only.
   * If a market convention requires a different routing currency for a given
   * pair, callers must seed that direct quote explicitly instead of relying on
   * automatic cross construction.
   */
  pivot_currency?: Currency;
}
/**
 * Delta-quoted FX volatility surface.
 *
 * Stores market-standard FX vol quotes (ATM DNS, 25-delta risk-reversal,
 * 25-delta butterfly) across multiple expiries. Models-layer functions
 * perform delta conversion and volatility evaluation.
 */
export interface FxDeltaVolSurface {
  /**
   * ATM delta-neutral straddle vols per expiry.
   */
  atm_vols: number[];
  /**
   * Optional 10-delta butterfly per expiry.
   */
  bf_10d?: number[] | null;
  /**
   * 25-delta butterfly per expiry.
   */
  bf_25d: number[];
  /**
   * Expiry times in years.
   */
  expiries: number[];
  /**
   * Surface identifier.
   */
  id: Id;
  /**
   * Optional 10-delta risk reversal per expiry.
   */
  rr_10d?: number[] | null;
  /**
   * 25-delta risk reversal per expiry.
   */
  rr_25d: number[];
}
/**
 * Serializable state of an FxMatrix.
 * Contains the configuration and cached quotes that can be persisted and restored.
 * Serialization fails with an ordinary serializer error if any captured
 * explicit or provider rate is non-finite or non-positive, including rates
 * that overflow after a mutable underlying provider changes under a shock.
 */
export interface FxMatrixState {
  /**
   * Matrix configuration, including pivot and cache capacity.
   */
  config: FxConfig;
  /**
   * Pinned, date/policy-scoped quotes as `(from, to, on, policy, rate)`.
   *
   * Required: a snapshot that omits it would silently re-derive those
   * dates from the provider instead of restoring the pinned fixings.
   */
  pinned_quotes: [unknown, unknown, unknown, unknown, unknown][];
  /**
   * Captured date/policy-scoped provider quotes. These override captured
   * pair-global provider quotes for their scope while remaining below
   * explicit matrix quotes in either direction. Required even when empty.
   */
  provider_pinned_quotes: [unknown, unknown, unknown, unknown, unknown][];
  /**
   * Captured provider quotes, below explicit global and date/policy-pinned
   * quotes in lookup priority. Market-context restoration uses these to
   * rebuild a quote-only provider; arbitrary live provider behavior is not
   * serialized. Required even when the provider has no snapshot quotes.
   */
  provider_quotes: [unknown, unknown, unknown][];
  /**
   * Pair-global quotes as source, target, rate tuples.
   */
  quotes: [unknown, unknown, unknown][];
}
/**
 * A single node in the market data hierarchy tree.
 *
 * Nodes form a tree: each has a name, optional key-value tags for cross-cutting
 * queries, ordered children, and leaf `CurveId` references.
 */
export interface HierarchyNode {
  /**
   * Child nodes, keyed by their name. The key becomes the child's `name`
   * during deserialization. Insertion order is preserved.
   */
  children?: {
    [k: string]: HierarchyNode;
  };
  /**
   * Curves attached directly to this node. Serialized as `curves`.
   */
  curves?: Id[];
  /**
   * Free-form key/value labels on this node, used for selection and
   * reporting. Insertion order is preserved.
   */
  tags?: {
    [k: string]: string;
  };
}
/**
 * Inflation index time series with lagging and seasonality.
 *
 * Wraps historical CPI/RPI observations with market-standard conventions for
 * lag application and interpolation. Used for pricing TIPS, linkers, and
 * inflation derivatives.
 *
 * # Components
 *
 * - **Observations**: Index levels labelled by reference date/month
 * - **Interpolation**: Daily interpolation between monthly observations
 * - **Lag**: Contractual observation lag (typically 3 months for TIPS)
 * - **Publication dates**: Optional explicit availability dates by reference month
 * - **Seasonality**: Optional monthly adjustment factors
 *
 * # Interpolation Methods
 *
 * - **Step**: Last observation carried forward (conservative)
 * - **Linear**: Daily interpolation between months (TIPS standard)
 *
 * # Lag Application
 *
 * Reference index calculation applies lag before interpolation:
 * ```text
 * For settlement date T with 3-month lag:
 * 1. Reference date = T - 3 months
 * 2. Find bracketing CPI observations
 * 3. Interpolate linearly between them
 * ```
 *
 * # Thread Safety
 *
 * Immutable after construction; safe to share via `Arc<InflationIndex>`.
 *
 * # References
 *
 * - **TIPS Mechanics**:
 *   - US Treasury (2024). "TIPS In Depth." treasurydirect.gov. `docs/REFERENCES.md#deacon-derry-mirfendereski-2004`
 *   - Deacon, M., Derry, A., & Mirfendereski, D. (2004). *Inflation-Indexed Securities*
 *     (2nd ed.). Wiley Finance. Chapter 2 (Index-linked bond mechanics). `docs/REFERENCES.md#deacon-derry-mirfendereski-2004`
 *
 * - **Index Lagging**:
 *   - Kerkhof, J. (2005). "Inflation Derivatives Explained." *Journal of Derivatives
 *     Accounting*, 2(1), 1-19. `docs/REFERENCES.md#kerkhof-2005`
 *   - Hurd, M., & Relleen, J. (2006). "Estimating the Inflation Risk Premium."
 *     Bank of England Quarterly Bulletin, Q2 2006. `docs/REFERENCES.md#kerkhof-2005`
 */
export interface InflationIndex {
  /**
   * Currency
   */
  currency: Currency;
  /**
   * Unique identifier
   */
  id: string;
  /**
   * Interpolation method
   */
  interpolation: InflationInterpolation;
  /**
   * Lag policy
   */
  lag: InflationLag;
  /**
   * Observations as (date, value) pairs
   */
  observations: [unknown, unknown][];
  /**
   * Explicit monthly observation availability; no release dates are inferred.
   */
  publication_dates?: InflationPublicationWire[];
  /**
   * Optional seasonality factors
   *
   * @minItems 12
   * @maxItems 12
   */
  seasonality?: [number, number, number, number, number, number, number, number, number, number, number, number] | null;
}
export interface InflationPublicationWire {
  /**
   * Inclusive date on which the observation must be available.
   */
  publication_date: DateWire;
  /**
   * First calendar day of the reference month.
   */
  reference_month: DateWire;
}
/**
 * Canonical v1 persisted snapshot of a complete market-data context.
 */
export interface MarketContextState {
  /**
   * Collateral CSA mappings
   */
  collateral: {
    [k: string]: string;
  };
  /**
   * Credit index aggregates (references curves by ID)
   */
  credit_indices: CreditIndexState[];
  /**
   * All curves (discount, forward, hazard, inflation, base correlation)
   */
  curves: CurveState[];
  /**
   * Dividend schedules
   */
  dividends: DividendSchedule[];
  /**
   * FX matrix state (optional)
   */
  fx?: FxMatrixState | null;
  /**
   * FX delta-quoted volatility surfaces
   */
  fx_delta_vol_surfaces: FxDeltaVolSurface[];
  /**
   * Market data hierarchy snapshot, or `null` when no hierarchy is configured.
   *
   * The key is mandatory but its value is nullable: `deserialize_with` makes
   * serde demand the key, and the struct-level `required` extension keeps it
   * mandatory in the schema without stripping the `null` branch.
   */
  hierarchy: MarketDataHierarchy | null;
  /**
   * Inflation indices
   */
  inflation_indices: InflationIndex[];
  /**
   * Market scalars and prices
   */
  prices: {
    [k: string]: MarketScalar;
  };
  /**
   * Required schema version. Only version `1` is accepted.
   */
  schema_version: SchemaVersion;
  /**
   * Generic time series
   */
  series: ScalarTimeSeries[];
  /**
   * Volatility surfaces
   */
  surfaces: VolSurface[];
  /**
   * SABR volatility cubes
   */
  vol_cubes: VolCube[];
}
/**
 * The top-level market data hierarchy containing root nodes.
 *
 * Each root represents a major asset class or category (e.g., "Rates", "Credit",
 * "FX", "Equity", "Volatility"). The hierarchy is fully serializable and can be
 * loaded from JSON configuration files.
 */
export interface MarketDataHierarchy {
  /**
   * Top-level nodes, keyed by their name. The key becomes each node's
   * `name` during deserialization. Insertion order is preserved.
   */
  roots: {
    [k: string]: HierarchyNode;
  };
  [k: string]: unknown;
}
/**
 * Date-indexed time series with flexible interpolation.
 *
 * Provides lightweight storage for historical or forecast data with step or
 * linear interpolation. Unlike full term structures, this is optimized for
 * sparse, irregularly-spaced observations.
 *
 * # Storage
 *
 * Uses columnar format with parallel arrays:
 * - Dates stored as i32 (days since Unix epoch) for compact size
 * - Values stored as f64
 * - Binary search for O(log n) lookup
 *
 * # Interpolation
 *
 * - **Step**: Last observation carried forward (LOCF)
 * - **Linear**: Linear interpolation between observations
 *
 * # Use Cases
 *
 * - **Economic indicators**: GDP, unemployment rate, PMI
 * - **Credit metrics**: Historical credit spreads, CDS levels
 * - **Commodity fundamentals**: Inventory levels, production data
 * - **Any sparse time series**: Where full curve infrastructure is overkill
 */
export interface ScalarTimeSeries {
  /**
   * Optional currency
   */
  currency?: Currency | null;
  /**
   * Series identifier
   */
  id: string;
  /**
   * Interpolation method
   */
  interpolation: SeriesInterpolation;
  /**
   * Observations as (date, value) pairs
   */
  observations: [unknown, unknown][];
}
/**
 * Volatility surface defined on expiry × strike grid.
 *
 * Internally stores volatilities in row-major order as a boxed slice.
 */
export interface VolSurface {
  /**
   * Additive displacements in forward/strike units, one per expiry for shifted Black quotes.
   */
  displacements?: number[] | null;
  /**
   * Expiry times in years
   */
  expiries: number[];
  /**
   * Surface identifier
   */
  id: string;
  /**
   * Interpolation contract.
   */
  interpolation_mode: VolInterpolationMode;
  /**
   * Quote convention.
   */
  quote_type: VolQuoteType;
  /**
   * Semantic meaning of the secondary axis.
   */
  secondary_axis: VolSurfaceAxis;
  /**
   * Strike prices
   */
  strikes: number[];
  /**
   * Volatility values in row-major order
   */
  vols_row_major: number[];
}
/**
 * SABR volatility cube on an expiry x tenor grid.
 *
 * Each grid node stores a [`SabrParameterData`] and a forward rate. The
 * interpolation mode records how a models-layer evaluator should combine
 * nodes; core performs structural validation only.
 */
export interface VolCube {
  /**
   * Option expiries in years, strictly increasing. Indexes the first axis.
   */
  expiries: number[];
  /**
   * Row-major: forwards[expiry_idx * n_tenors + tenor_idx]
   */
  forwards: number[];
  /**
   * Identifier the cube is registered and looked up under.
   */
  id: string;
  /**
   * How volatilities are interpolated between the cube's grid points.
   */
  interpolation_mode: VolInterpolationMode;
  /**
   * Row-major: params[expiry_idx * n_tenors + tenor_idx]
   */
  params: SabrParameterData[];
  /**
   * Underlying swap tenors in years, strictly increasing. Indexes the
   * second axis.
   */
  tenors: number[];
}
/**
 * Data-only SABR parameters for one volatility-cube node.
 *
 * This type records calibrated market data. SABR evaluation, interpolation,
 * calibration, and convention conversion are owned by
 * `finstack-quant-models`.
 */
export interface SabrParameterData {
  alpha: number;
  beta: number;
  nu: number;
  rho: number;
  shift?: number | null;
}
/**
 * A concrete period with start/end dates and actual/forecast flag.
 */
export interface Period {
  /**
   * Exclusive end date.
   */
  end: string;
  /**
   * Identifier of this period.
   */
  id: PeriodId;
  /**
   * True when this period is part of the "actuals" subset.
   */
  is_actual: boolean;
  /**
   * Inclusive start date.
   */
  start: string;
}
/**
 * Ordered reporting periods with their calendar bounds and actual/forecast flags.
 */
export interface PeriodPlan {
  /**
   * Ordered periods produced by the parser.
   */
  periods: Period[];
  [k: string]: unknown;
}
/**
 * Rating level for credit rating scales.
 */
export interface RatingLevel {
  /**
   * Minimum score threshold for this rating.
   */
  min_score: number;
  /**
   * Rating name, for example `AAA` or `Aaa`.
   */
  name: string;
  /**
   * Numeric score on a 0-100 scale.
   */
  score: number;
}
/**
 * Generated accrual, payment and fixing dates with any construction warnings.
 */
export interface Schedule {
  /**
   * Unadjusted accrual grid (period start plus each period end).
   *
   * These dates are never business-day adjusted. Payment-date adjustment,
   * payment lag, and fixing lag live on [`Self::payment_dates`] and
   * [`Self::fixing_dates`].
   */
  dates: DateWire[];
  /**
   * Fixing dates for each accrual period.
   *
   * Empty when no fixing lag is configured; otherwise the same length as
   * [`Self::payment_dates`].
   */
  fixing_dates?: DateWire[];
  /**
   * Payment date for each accrual period (one per period end).
   *
   * Length is `dates.len().saturating_sub(1)`. Duplicate payment dates are
   * retained so the series stays 1:1 with period ends.
   */
  payment_dates?: DateWire[];
  /**
   * Warnings generated during schedule construction.
   *
   * Non-empty when graceful fallback mode suppressed an error or when
   * other non-fatal issues occurred during generation.
   */
  warnings?: ScheduleWarning[];
  [k: string]: unknown;
}
/**
 * Persisted inputs of a date schedule: range, frequency, stub, adjustment and lags.
 */
export interface ScheduleSpec {
  business_day_convention?: BusinessDayConvention | null;
  calendar_id?: string | null;
  cds_imm_mode: boolean;
  end: DateWire;
  end_of_month: boolean;
  error_policy: ScheduleErrorPolicy;
  fixing_lag_business_days?: number | null;
  frequency: Tenor;
  imm_mode?: boolean;
  payment_lag_days?: number;
  start: DateWire;
  stub: StubKind;
}
/**
 * Named scorecard rating scale: rating levels ordered best to worst with score thresholds.
 */
export interface ScorecardScale {
  /**
   * Human-readable description.
   */
  description?: string | null;
  /**
   * Ordered list of rating levels from best to worst.
   */
  ratings: RatingLevel[];
  /**
   * Scale name, for example `S&P` or `Moody's`.
   */
  scale_name: string;
}
/**
 * A single named column in a [`TableEnvelope`].
 */
export interface TableColumn {
  /**
   * Column values.
   */
  data: TableColumnData;
  /**
   * Optional per-column metadata.
   */
  metadata?: {
    [k: string]: unknown;
  };
  /**
   * Column name.
   */
  name: string;
  /**
   * Optional semantic hint for bindings and consumers.
   */
  role?: TableColumnRole | null;
  [k: string]: unknown;
}
/**
 * Column-oriented table with a shared row count and typed column storage.
 */
export interface TableEnvelope {
  /**
   * Ordered set of columns.
   */
  columns: TableColumn[];
  /**
   * Table-level metadata for downstream bindings and documentation.
   */
  metadata?: {
    [k: string]: unknown;
  };
  /**
   * Number of rows in the table.
   */
  row_count: number;
  [k: string]: unknown;
}
/**
 * Tenor-by-strike volatility quotes at a fixed option expiry, preserving quote convention and displacement.
 */
export interface VolCubeExpirySlice {
  displacements?: number[] | null;
  expiry: number;
  id: string;
  quote_type: VolQuoteType;
  strikes: number[];
  tenors: number[];
  vols_row_major: number[];
}
