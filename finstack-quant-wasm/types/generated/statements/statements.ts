// Generated from the finstack-quant-statements JSON schemas by scripts/generate-contract-types.mjs. Do not edit.

/**
 * Generic accrual method usable across instruments.
 *
 * This mirrors the semantics of bond accrual methods but is defined at the
 * cashflow layer so it can be reused by any instrument that exposes a
 * `CashFlowSchedule`.
 */
export type AccrualMethod = "linear" | "compounded";
/**
 * How a self-referential cap base (where `base_node == target_node`) is
 * resolved each period.
 */
export type CapBaseMode = "reported" | "progressive";
/**
 * Defines how an adjustment value is derived.
 */
export type AdjustmentValue =
  | {
      /**
       * Map of period_id -> amount
       */
      amounts: {
        [k: string]: number;
      };
      type: "fixed";
      [k: string]: unknown;
    }
  | {
      /**
       * Node ID to reference (e.g., "revenue")
       */
      node_id: string;
      /**
       * Percentage to apply (e.g., 0.05 for 5%)
       */
      percentage: number;
      type: "percentage_of_node";
      [k: string]: unknown;
    };
/**
 * Amortization specification for principal over time.
 *
 * Describes how principal amortizes or is exchanged during the life of the contract.
 * Used by instruments (e.g., bonds) and cashflow legs for consistent behavior.
 */
export type AmortizationSpec =
  | "none"
  | {
      linear_to: {
        /**
         * Target remaining principal at the end of the amortization schedule.
         */
        final_notional: Money;
      };
    }
  | {
      step_remaining: {
        /**
         * Ordered list of `(date, remaining_principal_after_date)`.
         */
        schedule: [unknown, unknown][];
      };
    }
  | {
      percent_of_original_per_period: {
        /**
         * Fraction of original notional paid per period (e.g., 0.05 = 5%).
         */
        pct: number;
      };
    }
  | {
      percent_of_remaining_per_period: {
        /**
         * Fraction of the outstanding principal paid per period (e.g.,
         * 0.025 = 2.5%), in `[0, 1]`.
         */
        pct: number;
      };
    }
  | {
      linear_between: {
        /**
         * Amortization end (full repayment), on or before maturity.
         */
        end: DateWire;
        /**
         * Amortization start; installments fall on payment dates strictly
         * after it.
         */
        start: DateWire;
      };
    }
  | {
      custom_principal: {
        /**
         * List of `(date, principal_amount)` exchanges; amounts are absolute cashflows.
         */
        items: [unknown, unknown][];
      };
    };
/**
 * Exact decimal encoded only as a JSON string.
 */
export type DecimalWire = string;
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
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
/**
 * Value that can be currency-aware or unitless.
 *
 * Used for node values that can represent:
 * - **Amount**: Currency-aware monetary values (e.g., USD 1,000,000)
 * - **Scalar**: Unitless values (e.g., ratios, percentages, counts)
 */
export type AmountOrScalar = Money | number;
/**
 * Anti-dilution protection applied to conversion terms.
 *
 * When dilutive events occur (stock splits, below-market issuances, special
 * dividends), the conversion ratio is adjusted to protect bondholders from
 * value erosion.
 *
 * # Industry Practice
 *
 * Most convertible bonds use **Weighted Average** anti-dilution, which is
 * less protective but more issuer-friendly. **Full Ratchet** is mainly seen
 * in private placements and venture-style convertibles.
 */
export type AntiDilutionPolicy = "none" | "full_ratchet" | "weighted_average";
/**
 * Asset dynamics specification for the Merton model.
 *
 * Controls the stochastic process assumed for the firm's asset value.
 */
export type AssetDynamics =
  | "geometric_brownian"
  | {
      jump_diffusion: {
        /**
         * Poisson jump arrival intensity (jumps per year).
         */
        jump_intensity: number;
        /**
         * Mean log-jump size.
         */
        jump_mean: number;
        /**
         * Volatility of log-jump size.
         */
        jump_vol: number;
        [k: string]: unknown;
      };
    }
  | {
      credit_grades: {
        /**
         * Log-normal barrier volatility `λ`: the standard deviation of the
         * natural log of the default barrier (Finger et al. 2002,
         * "CreditGrades Technical Document"). Despite the field name, this
         * is *not* a generic uncertainty scalar — it is the lognormal
         * dispersion of the global recovery rate, entering the survival
         * formula as `a_t² = σ²t + λ²` and the barrier shift `exp(λ²)`.
         */
        barrier_uncertainty: number;
        /**
         * Mean recovery rate at default.
         */
        mean_recovery: number;
        [k: string]: unknown;
      };
    };
/**
 * Barrier-crossing detection policy for first-passage default simulation.
 *
 * `Discrete` only checks the barrier at grid points (fast but biased for
 * coarse time steps). `BrownianBridge` uses a Brownian-bridge crossing
 * probability between grid points to approximate continuous monitoring.
 */
export type BarrierCrossing = "discrete" | "brownian_bridge";
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
 * # Examples
 *
 * ```rust
 * use finstack_quant_core::types::{CurveId, InstrumentId};
 *
 * // Create IDs with different type tags
 * let curve = CurveId::from("USD-SOFR");
 * let bond = InstrumentId::from("ISIN:US912828XG60");
 *
 * // Can compare IDs of the same type
 * assert_eq!(curve, CurveId::from("USD-SOFR"));
 * assert_ne!(curve, CurveId::from("EUR-ESTR"));
 *
 * // Cannot compare IDs of different types (compile error):
 * // let _ = curve == bond;  // Error: mismatched types
 * ```
 *
 * # Thread Safety
 *
 * `Id<T>` is `Send + Sync` as it wraps an `Arc<str>`. Multiple threads can
 * safely share and clone IDs with minimal synchronization overhead.
 */
export type Id = string;
/**
 * Thin facade over canonical builder coupon specs for bond cashflows.
 *
 * Wraps `FixedCouponSpec` and `FloatingCouponSpec` from the cashflow builder,
 * providing convenience constructors with sensible defaults for common bond use cases.
 * This ensures parity with all builder features (floors/caps, BDC, calendars, PIK, etc.)
 * while keeping the bond API simple.
 */
export type CashflowSpec =
  | {
      fixed: FixedCouponSpec;
    }
  | {
      floating: FloatingCouponSpec;
    }
  | {
      step_up: StepUpCouponSpec;
    }
  | {
      amortizing: {
        /**
         * Base cashflow specification (fixed or floating).
         */
        base: CashflowSpec;
        /**
         * Amortization schedule.
         */
        schedule: AmortizationSpec;
        [k: string]: unknown;
      };
    };
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
 * # Examples
 *
 * ```rust
 * use finstack_quant_core::dates::{adjust, BusinessDayConvention, Date};
 * use finstack_quant_core::dates::calendar::TARGET2;
 * use time::Month;
 *
 * // Saturday, January 4, 2025
 * let weekend = Date::from_calendar_date(2025, Month::January, 4).expect("Valid date");
 *
 * // Following: moves to next Monday (Jan 6)
 * let adj = adjust(weekend, BusinessDayConvention::Following, &TARGET2)?;
 * assert_eq!(adj.day(), 6);
 *
 * // Preceding: moves to previous Friday (Jan 3)
 * let adj = adjust(weekend, BusinessDayConvention::Preceding, &TARGET2)?;
 * assert_eq!(adj.day(), 3);
 * # Ok::<(), finstack_quant_core::Error>(())
 * ```
 */
export type BusinessDayConvention =
  "unadjusted" | "following" | "modified_following" | "preceding" | "modified_preceding" | "nearest";
/**
 * Coupon cashflow type for fixed/floating coupons.
 *
 * - `Cash`: 100% paid in cash.
 * - `PIK`: 100% capitalized into principal.
 * - `Split { cash_fraction, pik_fraction }`: decimal shares (summing to 1) of
 *   the coupon amount paid in cash and capitalized.
 */
export type CouponType =
  | "cash"
  | "pik"
  | {
      split: {
        /**
         * Fraction of the coupon paid in cash, expressed as a decimal share in
         * `[0, 1]`.
         */
        cash_fraction: DecimalWire;
        /**
         * Fraction of the coupon capitalized as PIK, expressed as a decimal
         * share in `[0, 1]`.
         */
        pik_fraction: DecimalWire;
      };
    };
/**
 * Supported day-count conventions with industry-standard definitions.
 *
 * Each variant implements a specific day count convention as defined by
 * ISDA, ICMA, or local market conventions. The conventions determine how
 * interest accrues between payment dates.
 *
 * # Standards References
 *
 * Implementations follow:
 * - **ISDA**: 2006 ISDA Definitions, Section 4.16
 * - **ICMA**: ICMA Rule Book, Rule 251
 * - **ISO**: ISO 20022 Day Count Fraction Codes
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
 * use time::Month;
 *
 * let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
 * let end = Date::from_calendar_date(2025, Month::July, 1).expect("Valid date");
 *
 * // Actual/360 - money market convention
 * let yf_360 = DayCount::Act360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
 *
 * // 30/360 - bond convention
 * let yf_30360 = DayCount::Thirty360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
 *
 * assert!(yf_360 > yf_30360); // Act/360 has larger denominator
 * ```
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
 * Unit of a tenor period.
 */
export type TenorUnit = "days" | "weeks" | "months" | "years";
/**
 * Roll-date rule applied when generating schedule anchors.
 *
 * Selects the core `ScheduleBuilder` date-generation mode. The IMM modes
 * force a quarterly grid: the schedule frequency and stub rule configured on
 * [`ScheduleParams`] are overridden with quarterly / short-back, matching
 * `ScheduleBuilder::imm` and `ScheduleBuilder::cds_imm`.
 *
 * # Variants
 *
 * - **`None`**: plain tenor stepping from the schedule boundaries (default).
 * - **`Imm`**: quarterly third Wednesdays of Mar/Jun/Sep/Dec (CME IMM dates
 *   for rate, currency, and equity index futures).
 * - **`CdsImm`**: 20th of Mar/Jun/Sep/Dec (post-Big-Bang standard CDS roll
 *   dates). When the start date is not itself a roll date, the first period
 *   accrues from the roll date immediately **preceding** the start (standard
 *   front accrual per the ISDA Big Bang Protocol, April 2009).
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_cashflows::builder::specs::RollRule;
 *
 * let rule = RollRule::default();
 * assert_eq!(rule, RollRule::None);
 * ```
 *
 * # References
 *
 * - `docs/REFERENCES.md#isda-cds-standard-model`
 * - CME IMM date rules (third Wednesday of the contract month)
 */
export type RollRule = "none" | "imm" | "cds_imm";
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
 * # Examples
 *
 * ```rust
 * use finstack_quant_core::dates::{ScheduleBuilder, Tenor, StubKind};
 * use time::{Date, Month};
 *
 * let start = Date::from_calendar_date(2025, Month::January, 10)?;
 * let end = Date::from_calendar_date(2025, Month::December, 15)?;
 *
 * // Short stub at front
 * let sched = ScheduleBuilder::new(start, end)?
 *     .frequency(Tenor::quarterly())
 *     .stub_rule(StubKind::ShortFront)
 *     .build()?;
 * # Ok::<(), Box<dyn std::error::Error>>(())
 * ```
 *
 * # See Also
 *
 * - [`ScheduleBuilder::stub_rule`] to configure stub behavior
 */
export type StubKind = "none" | "short_front" | "short_back" | "long_front" | "long_back";
/**
 * How the fixings of one accrual period combine into the period rate.
 *
 * One canonical enum for every floating leg and coupon: swaps, basis and
 * cross-currency legs, TRS financing, IR futures, FRN / loan / structured
 * credit coupons.
 *
 * | Variant | Period rate | Typical use |
 * |---------|-------------|-------------|
 * | `simple` | One term fixing / forward over the period | EURIBOR, Term SOFR, legacy IBOR |
 * | `simple_average` | `(Σ rᵢ·dᵢ) / D` of daily overnight fixings | Averaged overnight loans, Fed-Funds futures |
 * | `compounded_in_arrears` | `[∏(1 + rᵢ·dᵢ/B) − 1]·B/D` | SOFR / SONIA / €STR / TONA OIS (lookback 0) |
 * | `compounded_with_observation_shift` | as above, observations and weights shifted | ISDA 2021 observation shift |
 * | `compounded_with_rate_cutoff` | as above, last fixings frozen | ARRC lockout / SWPM "Rate Cut-Off Days" |
 *
 * Day counts are business days and must be non-negative.
 *
 * # References
 *
 * - ISDA 2021 Definitions, compounded RFR conventions `docs/REFERENCES.md#isda-2021-definitions`
 * - ARRC (2020). "SOFR: A User's Guide." `docs/REFERENCES.md#arrc-sofr-users-guide`
 * - BoE SONIA conventions `docs/REFERENCES.md#boe-sonia-key-features`
 *
 * # Examples
 *
 * ```
 * use finstack_quant_cashflows::builder::FloatingLegCompounding;
 *
 * assert_eq!(FloatingLegCompounding::default(), FloatingLegCompounding::Simple);
 * assert_eq!(
 *     FloatingLegCompounding::sofr(),
 *     FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
 * );
 * ```
 */
export type FloatingLegCompounding =
  | "simple"
  | "simple_average"
  | {
      compounded_in_arrears: {
        /**
         * Business days by which observation dates move backward while the
         * day-count weights stay on the original accrual dates ("lookback
         * without observation shift", ISDA 2021 / ARRC). `0` is plain
         * in-arrears, the cleared-OIS convention.
         */
        lookback_days: number;
      };
    }
  | {
      compounded_with_observation_shift: {
        /**
         * Business days to shift both observation dates and weights.
         */
        shift_days: number;
      };
    }
  | {
      compounded_with_rate_cutoff: {
        /**
         * Business days before period end over which the rate is frozen.
         */
        cutoff_days: number;
      };
    };
/**
 * Policy for handling floating rate projection failures.
 *
 * Controls what happens when a forward curve lookup fails during
 * cashflow emission. The default (`Error`) surfaces failures explicitly;
 * the other variants are explicit opt-in degradation modes for callers
 * that intentionally want a projected schedule without a forward curve.
 *
 * # References
 *
 * - `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
 * - `docs/REFERENCES.md#hull-options-futures`
 */
export type FloatingRateFallback =
  | "error"
  | "spread_only"
  | {
      fixed_rate: DecimalWire;
    };
/**
 * Where overnight index floors/caps are applied for daily-compounded rates.
 */
export type OvernightIndexConstraintApplication = "daily" | "period";
/**
 * Enumeration of cash-flow kinds for classification and ordering.
 *
 * Used to distinguish between different types of cashflows for
 * proper sequencing, risk calculation, and accounting treatment.
 *
 * # Sign Convention
 *
 * The enum itself is **view agnostic**: individual instruments are
 * responsible for mapping these kinds into a holder or issuer view.
 * By convention in this crate:
 *
 * | Kind | Holder View (Long) | Issuer View (Short) |
 * |------|-------------------|---------------------|
 * | Interest (Fixed/Float) | Positive (receive) | Negative (pay) |
 * | Notional (initial) | Negative (pay) | Positive (receive) |
 * | Notional (final) | Positive (receive) | Negative (pay) |
 * | Amortization | Positive (receive) | Negative (pay) |
 * | PIK | Increases notional | Increases liability |
 * | Fee | Negative (pay) | Positive (receive) |
 *
 * When constructing cashflow schedules, instruments should apply the appropriate
 * sign based on the economic perspective being represented.
 *
 * # Cashflow Categories
 *
 * Variants are grouped by category:
 * - **Interest**: `Fixed`, `FloatReset`, `Stub`
 * - **Inflation**: `InflationCoupon`
 * - **Fees**: `Fee`, `CommitmentFee`, `UsageFee`, `FacilityFee`
 * - **Principal**: `Notional`, `PIK`, `Amortization`, `PrePayment`
 * - **Revolving**: `RevolvingDraw`, `RevolvingRepayment`
 * - **Credit Events**: `DefaultedNotional`, `Recovery`
 * - **Margin/Collateral**: `InitialMarginPost`, `VariationMarginPay`, etc.
 */
export type CFKind =
  | "fixed"
  | "float_reset"
  | "inflation_coupon"
  | "fee"
  | "commitment_fee"
  | "usage_fee"
  | "facility_fee"
  | "lc_fee"
  | "fronting_fee"
  | "notional"
  | "pik"
  | "amortization"
  | "pre_payment"
  | "revolving_draw"
  | "revolving_repayment"
  | "defaulted_notional"
  | "recovery"
  | "accrued_on_default"
  | "stub"
  | "initial_margin_post"
  | "initial_margin_return"
  | "variation_margin_receive"
  | "variation_margin_pay"
  | "margin_interest"
  | "collateral_substitution_in"
  | "collateral_substitution_out";
/**
 * Meaning of the emitted schedule relative to pricing and waterfall policy.
 */
export type CashflowRepresentation = "contractual" | "projected" | "placeholder" | "no_residual";
/**
 * Merton Monte Carlo configuration stored on the bond for registry-based pricing.
 *
 * This is a wrapper around
 * [`crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig`]
 * that allows the pricer registry to access the MC configuration from
 * [`InstrumentPricingOverrides`].
 */
export type MertonMcOverride = MertonMcConfig;
/**
 * Which structural parameter to calibrate in the MC engine.
 */
export type CalibrationParameter = "debt_barrier" | "asset_vol";
/**
 * Quote input for the bond quote engine.
 *
 * All spreads are expressed in **decimal** (`0.01 = 100bp`).
 */
export type BondQuoteInput =
  | {
      clean_price_pct: number;
    }
  | {
      dirty_price_currency: number;
    }
  | {
      ytm: number;
    }
  | {
      ytw: number;
    }
  | {
      z_spread: number;
    }
  | {
      discount_margin: number;
    }
  | {
      oas: number;
    }
  | {
      asw_market: number;
    }
  | {
      i_spread: number;
    }
  | {
      japanese_simple_yield: number;
    };
/**
 * Recovery model specification.
 */
export type RecoveryModel =
  | "constant"
  | "inverse_linear"
  | {
      inverse_power: {
        /**
         * Power exponent (`alpha`).
         */
        exponent: number;
        [k: string]: unknown;
      };
    }
  | {
      floored_inverse: {
        /**
         * Minimum recovery rate floor.
         */
        floor: number;
        [k: string]: unknown;
      };
    }
  | {
      linear_decline: {
        /**
         * Minimum recovery rate floor.
         */
        floor: number;
        /**
         * Sensitivity of recovery to leverage increase (`beta`).
         */
        sensitivity: number;
        [k: string]: unknown;
      };
    };
/**
 * Map from leverage to hazard rate.
 */
export type LeverageHazardMap =
  | {
      power_law: {
        /**
         * Power-law exponent (`beta`).
         */
        exponent: number;
        [k: string]: unknown;
      };
    }
  | {
      exponential: {
        /**
         * Exponential sensitivity (`beta`).
         */
        sensitivity: number;
        [k: string]: unknown;
      };
    }
  | {
      tabular: {
        /**
         * Corresponding hazard rates at each breakpoint.
         */
        hazard_points: number[];
        /**
         * Leverage breakpoints (must be sorted ascending).
         */
        leverage_points: number[];
        [k: string]: unknown;
      };
    };
/**
 * Barrier monitoring type for default determination.
 */
export type MertonBarrierType =
  | "terminal"
  | {
      first_passage: {
        /**
         * Growth rate of the default barrier over time.
         */
        barrier_growth_rate: number;
        [k: string]: unknown;
      };
    };
/**
 * Time-varying PIK schedule for the MC engine.
 *
 * Controls per-coupon PIK behavior, either uniformly or as a step
 * function over time.
 *
 * # Examples
 *
 * ```
 * use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::merton_mc::{PikMode, PikSchedule};
 *
 * // All coupons PIK
 * let uniform = PikSchedule::Uniform(PikMode::Pik);
 *
 * // PIK for first 2 years, then cash
 * let stepped = PikSchedule::Stepped(vec![(0.0, PikMode::Pik), (2.0, PikMode::Cash)]);
 *
 * // Toggle for 3 years, then mandatory cash
 * let toggle_window = PikSchedule::Stepped(vec![(0.0, PikMode::Toggle), (3.0, PikMode::Cash)]);
 * ```
 */
export type PikSchedule =
  | {
      uniform: PikMode;
    }
  | {
      stepped: [unknown, unknown][];
    };
/**
 * Per-coupon PIK behavior for the MC engine.
 *
 * Determines how each coupon payment is handled: paid in cash, accreted
 * to notional (PIK), split between cash and PIK, or decided dynamically
 * by a [`ToggleExerciseModel`].
 */
export type PikMode =
  | "cash"
  | "pik"
  | {
      split: {
        /**
         * Fraction paid in cash (e.g. 0.5 for 50%).
         */
        cash_fraction: number;
        /**
         * Fraction accreted to notional.
         */
        pik_fraction: number;
        [k: string]: unknown;
      };
    }
  | "toggle";
/**
 * Toggle exercise model for PIK/cash decision.
 */
export type ToggleExerciseModel =
  | {
      threshold: ThresholdToggle;
    }
  | {
      stochastic: StochasticToggle;
    }
  | {
      optimal_exercise: OptimalToggle;
    };
/**
 * Direction for threshold comparison.
 */
export type ThresholdDirection = "above" | "below";
/**
 * Which credit metric drives the toggle decision.
 */
export type CreditStateVariable = "hazard_rate" | "distance_to_default" | "leverage";
/**
 * Price/accrual convention used for OAS inversion targets.
 */
export type OasPriceBasis = "settlement_dirty" | "forward_accrued_clean";
/**
 * Compounding convention for interest rates.
 *
 * Used to specify how interest rates should be quoted or converted.
 * All variants produce mathematically equivalent discount factors when
 * applied consistently.
 *
 * # Relationship Between Conventions
 *
 * For a given discount factor DF at time t, the rates under different
 * conventions are related by:
 *
 * ```text
 * DF = e^(-r_cc × t)                    [Continuous]
 *    = (1 + r_ann)^(-t)                 [Annual]
 *    = (1 + r_per/n)^(-n×t)             [Periodic(n)]
 *    = 1 / (1 + r_simple × t)           [Simple]
 * ```
 *
 * # Ordering of Rates
 *
 * For positive rates and t > 0: `r_simple > r_annual > r_continuous`
 * (less frequent compounding requires a higher quoted rate for the same DF).
 */
export type Compounding =
  | "continuous"
  | "annual"
  | {
      periodic: number;
    }
  | "simple";
/**
 * AssetPool-granularity policy for the structured-credit default engine.
 *
 * Selects whether each scenario realizes defaults name-by-name (finite-pool
 * copula simulation) or applies the closed-form LHP conditional default
 * probability uniformly to the pool.
 */
export type PoolGranularity = "per_name" | "large_homogeneous";
/**
 * Short-rate lattice used by the rates-only bond tree.
 *
 * Selected by `instrument_pricing_overrides.model_config.tree_model`; when the
 * field is absent the bond tree uses Hull-White.
 */
export type ShortRateTreeModel = "hull_white" | "black_derman_toy";
/**
 * Policy for evaluating volatility surfaces outside their calibrated grid.
 *
 * Market-standard production systems typically make this choice explicit because
 * extrapolation can materially affect PV and greeks.
 *
 * # Market Standards
 *
 * - **Error**: Conservative approach for production systems; forces explicit handling.
 * - **Clamp**: Simple flat extrapolation; common for quick prototyping.
 * - **LinearInVariance**: Market-standard for equity/FX; preserves no-arbitrage conditions
 *   better than linear-in-vol by extrapolating in total variance space (σ²T).
 */
export type VolSurfaceExtrapolation = "error" | "clamp" | "linear_in_variance";
/**
 * Basis used for bond duration, convexity, and DV01-style risk metrics.
 */
export type BondRiskBasis = "bullet_discountable" | "callable_oas";
/**
 * Linear (first-order) or iterative (full-reprice root-find) solve mode.
 *
 * # Why the two modes disagree
 *
 * The gap between them is usually **not** dominated by convexity. `Linear`
 * divides by the sensitivity measured at `as_of`, whereas `Iterative`
 * reprices at the horizon date, where the instrument has less remaining time
 * and therefore a different sensitivity. On a 5Y bond over a 6M horizon the
 * two differ by several percent even though convexity over the ~9bp solved
 * shift contributes only a fraction of that. The gap grows with horizon
 * length, not just with curvature.
 *
 * `Iterative` is the more accurate answer where it is supported; `Linear` is
 * the fast approximation and the default.
 */
export type BreakevenMode = "linear" | "iterative";
/**
 * Which valuation parameter to solve the breakeven for.
 *
 * # Result units
 *
 * The breakeven metric is a bare `f64` whose unit depends on the target. Read
 * the per-variant docs before interpreting a value:
 *
 * | Target             | Sensitivity     | Result unit          |
 * |--------------------|-----------------|----------------------|
 * | `ZSpread`          | CS01            | basis points         |
 * | `Ytm`              | DV01            | basis points         |
 * | `Oas`              | CS01            | basis points         |
 * | `ImpliedVol`       | Vega            | vol points (1 = 1%)  |
 * | `BaseCorrelation`  | Correlation01   | correlation points   |
 */
export type BreakevenTarget = "z_spread" | "ytm" | "implied_vol" | "base_correlation" | "oas";
/**
 * Day basis used to convert annual analytic option theta into a per-day amount.
 *
 * Applied by the analytic-theta pricers (EquityOption, FxOption,
 * FxDigitalOption) through `metric_pricing_overrides.theta_day_basis`.
 */
export type ThetaDayBasis = "calendar_365" | "trading_252";
/**
 * Finite JSON number in the open interval `(0, 1)`.
 */
export type OpenUnitIntervalF64Wire = number;
/**
 * VaR calculation method.
 */
export type VarMethod = "full_revaluation" | "taylor_approximation";
/**
 * Issue price = invested capital `V0` (amount funded at issue; IRR initial outflow
 * and MOIC denominator).
 *
 * Defaults to [`IssuePrice::Par`] when not specified.
 */
export type IssuePrice =
  | "par"
  | {
      amount: Money;
    }
  | {
      pct_of_par: number;
    };
/**
 * Which return metric is guaranteed by the floor, and its target.
 *
 * Use [`ReturnFloorKind::Moic`] for money-multiple protection (common in
 * leveraged loans and private credit) or [`ReturnFloorKind::Xirr`] for
 * annualized IRR protection.
 */
export type ReturnFloorKind =
  | {
      moic: number;
    }
  | {
      xirr: number;
    };
/**
 * When the return-floor protection window applies.
 *
 * The window defines the interval over which the issuer can trigger a
 * floor-protected early redemption. Outside this window the floor is
 * inactive (the bond behaves as uncallable or follows its normal call
 * schedule). The floor binds only on early issuer-called redemptions; it
 * never applies at maturity.
 */
export type ProtectionWindow =
  | "full"
  | {
      from: DateWire;
    }
  | {
      between: {
        /**
         * Last date on which the floor-protected call window closes (inclusive).
         */
        end: DateWire;
        /**
         * First date on which the floor-protected call window opens.
         */
        start: DateWire;
        [k: string]: unknown;
      };
    };
/**
 * Exercise schedule convention for option models.
 */
export type ExerciseStyle = "european" | "american" | "bermudan";
/**
 * Whether a contractual spread is included in overnight daily compounding.
 *
 * ISDA-standard RFR coupons normally compound only the overnight index and add
 * any spread as simple interest after compounding. `Include` represents the
 * less common contract where the spread enters every daily compound factor.
 */
export type OvernightSpreadCompounding = "exclude" | "include";
/**
 * Type of interest rate option
 */
export type RateOptionType = "cap" | "floor" | "caplet" | "floorlet";
/**
 * Settlement type for options
 */
export type SettlementType = "physical" | "cash";
/**
 * Volatility convention for cap/floor pricing.
 *
 * The volatility type determines how the input volatility is interpreted
 * and which pricing model is used:
 *
 * # Lognormal (Black-Scholes)
 *
 * The standard market convention where volatility is expressed as a
 * proportion of the forward rate. Uses the Black (1976) formula.
 *
 * **Constraints**: Requires positive forward rates and strikes.
 *
 * # Normal (Bachelier)
 *
 * Volatility expressed in absolute rate terms (e.g., 50bp = 0.50%).
 * Uses the Bachelier model, which naturally handles negative rates.
 *
 * **Use case**: EUR/CHF markets with negative rates.
 *
 * # Market Convention Notes
 *
 * - **USD**: Historically lognormal, shifting to normal post-SOFR
 * - **EUR**: Predominantly normal since negative rates became common
 * - **GBP/JPY**: Mixed, check dealer quotes
 *
 * Always verify the vol convention with your data provider as using
 * the wrong type will produce materially incorrect prices.
 */
export type CapFloorVolType = "lognormal" | "shifted_lognormal" | "normal" | "auto";
/**
 * Cash claim category affected by a capital-structure warning.
 */
export type CapitalStructureClaimCategory = "fees" | "interest";
/**
 * Instruments supported by company financial statement capital structures.
 *
 * This intentionally smaller union keeps the financial statement schema
 * focused on debt and its common interest-rate hedges. The payloads are
 * converted to the canonical valuations registry only when a model is
 * evaluated.
 */
export type FinancialStatementInstrument =
  | {
      spec: Bond;
      type: "bond";
    }
  | {
      spec: ConvertibleBond;
      type: "convertible_bond";
    }
  | {
      spec: RevolvingCredit;
      type: "revolving_credit";
    }
  | {
      spec: TermLoan;
      type: "term_loan";
    }
  | {
      spec: InterestRateSwap;
      type: "interest_rate_swap";
    }
  | {
      spec: CapFloor;
      type: "cap_floor";
    }
  | {
      spec: Swaption;
      type: "swaption";
    };
/**
 * How dividends affect conversion terms (dividend protection).
 *
 * Dividend protection compensates the holder for dividends paid on the
 * underlying, which otherwise leak value from the conversion option (the
 * stock drifts at `r - q` under the risk-neutral measure). The variants
 * carry no threshold parameter, so protection is **full**: the entire
 * continuous dividend yield `q` is protected.
 *
 * # Pricing model (continuous-yield accretion)
 *
 * Under the continuous dividend yield `q` used by the tree pricer, each
 * protected dividend payment bumps the conversion ratio by the dividend
 * fraction; the continuous limit is exponential accretion from the
 * valuation date:
 *
 * ```text
 * AdjustRatio:  ratio(t)  = ratio_0 * exp(q * t)
 * AdjustPrice:  K_conv(t) = K_0 * exp(-q * t)
 * ```
 *
 * Since `ratio = notional / K_conv`, the price decay is mathematically the
 * same ratio accretion — the pricer implements both variants through the
 * single ratio-accretion mechanism. Accretion composes multiplicatively on
 * top of any anti-dilution adjustments from
 * [`ConvertibleBond::effective_conversion_ratio`].
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_valuations::instruments::fixed_income::convertible::{
 *     AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
 * };
 *
 * let conversion = ConversionSpec {
 *     ratio: Some(25.0),
 *     price: None,
 *     policy: ConversionPolicy::Voluntary,
 *     anti_dilution: AntiDilutionPolicy::None,
 *     dividend_adjustment: DividendAdjustment::AdjustRatio,
 *     dilution_events: Vec::new(),
 * };
 * assert!(conversion.dividend_adjustment.is_protected());
 * ```
 */
export type DividendAdjustment = "none" | "adjust_price" | "adjust_ratio";
/**
 * Defines how and when conversion can occur.
 */
export type ConversionPolicy =
  | "voluntary"
  | {
      mandatory_on: DateWire;
    }
  | {
      window: {
        /**
         * End.
         */
        end: DateWire;
        /**
         * Start.
         */
        start: DateWire;
        [k: string]: unknown;
      };
    }
  | {
      upon_event: ConversionEvent;
    }
  | {
      mandatory_variable: {
        /**
         * Date of mandatory conversion.
         */
        conversion_date: DateWire;
        /**
         * Lower conversion price (below this, holder receives max shares).
         */
        lower_conversion_price: number;
        /**
         * Upper conversion price (above this, holder receives min shares).
         */
        upper_conversion_price: number;
        [k: string]: unknown;
      };
    };
/**
 * Events that may trigger conversion.
 */
export type ConversionEvent =
  | "qualified_ipo"
  | "change_of_control"
  | {
      price_trigger: PriceTrigger;
    };
/**
 * Draw and repayment specification.
 *
 * Determines whether the facility uses a known (deterministic) schedule
 * or stochastic utilization for Monte Carlo pricing.
 */
export type DrawRepaySpec =
  | {
      deterministic: DrawRepayEvent[];
    }
  | {
      stochastic: StochasticUtilizationSpec;
    };
/**
 * Credit spread process specification.
 */
export type CreditSpreadProcessSpec =
  | {
      cir: {
        /**
         * Initial credit spread
         */
        initial: number;
        /**
         * Mean reversion speed (κ)
         */
        kappa: number;
        /**
         * Volatility (σ)
         */
        sigma: number;
        /**
         * Long-term mean (θ)
         */
        theta: number;
        [k: string]: unknown;
      };
    }
  | {
      constant: number;
    }
  | {
      market_anchored: {
        /**
         * Credit curve identifier in `MarketContext` used to anchor spreads.
         */
        credit_curve_id: Id;
        /**
         * Annualized CDS (index) option implied volatility for spreads.
         */
        implied_vol: number;
        /**
         * Mean reversion speed (κ) of the CIR process.
         */
        kappa: number;
        /**
         * Optional tenor in years; if None, uses facility maturity horizon.
         */
        tenor_years?: number | null;
        [k: string]: unknown;
      };
    };
/**
 * Upfront (arrangement or original-issue-discount) fee of a facility.
 *
 * Paid by the borrower to the lender on the issue date. Enters the
 * present value only while the commitment date lies after the valuation
 * date, and the effective-interest-rate metrics always.
 */
export type UpfrontFee =
  | {
      amount: Money;
    }
  | {
      fraction_of_commitment: number;
    };
/**
 * Contractual coupon of a loan facility or note: a fixed all-in rate or a
 * floating index plus spread.
 *
 * Shared by `TermLoan.rate`, `RevolvingCredit.rate`, `AssetBackedFacility.rate`
 * and structured-credit `Tranche.coupon`.
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_valuations::instruments::fixed_income::loan_terms::RateSpec;
 *
 * let fixed = RateSpec::Fixed { rate: 0.06 }; // 6% all-in
 * assert!(matches!(fixed, RateSpec::Fixed { .. }));
 * ```
 */
export type RateSpec =
  | {
      fixed: {
        /**
         * Annual rate as a decimal (`0.06` = 6%).
         */
        rate: number;
      };
    }
  | {
      floating: FloatingRateSpec;
    };
/**
 * Type of borrower call provision on a term loan.
 *
 * Institutional term loans use several types of call provisions:
 * - **Hard call**: Non-callable until the call date, then callable at the stated price.
 * - **Soft call**: Callable at any time, but subject to a premium (call protection).
 *   Typically applies for the first 6-24 months ("non-call period").
 * - **Make-whole**: Borrower must pay the present value of remaining cashflows
 *   discounted at a reference rate (typically a Treasury rate) plus a spread.
 *   This ensures the lender receives full economic value upon early prepayment.
 *
 * # Industry Practice
 *
 * Leveraged term loans typically have 6-12 months of soft call protection
 * (101% of par, sometimes called "soft call 101"), after which they become
 * callable at par. Make-whole provisions are more common in investment-grade
 * term loans and private placements.
 */
export type LoanCallType =
  | "hard"
  | "soft"
  | {
      make_whole: MakeWholeSpec;
    };
/**
 * Basis for calculating commitment fees on undrawn portions.
 *
 * Determines the denominator for commitment fee calculations on
 * revolving or delayed-draw facilities.
 */
export type CommitmentFeeBase = "undrawn" | "commitment_minus_outstanding";
/**
 * Original Issue Discount (OID) policy for term loan origination.
 *
 * OID represents the discount from par value at loan origination. The policy
 * determines how the discount is handled: withheld from proceeds or tracked separately.
 *
 * # Industry Practice
 *
 * OID is common in institutional term loans and private credit, particularly for:
 * - Leveraged buyout financing (LBO loans)
 * - Distressed refinancings
 * - High-yield institutional term loans
 *
 * Typical OID ranges from 1-5% (100-500 bp) of par value.
 *
 * # Accounting Treatment
 *
 * OID affects accounting under GAAP/IFRS:
 * - **Withheld**: Reduces initial cash proceeds, increases effective yield
 * - **Separate**: May be accounted as upfront fee or amortized discount
 *
 * For effective interest rate (EIR) amortization schedules, see [`OidEirSpec`].
 *
 * # Variants
 *
 * - `WithheldBp`: Discount in basis points of each funded draw, withheld
 *   from proceeds
 * - `WithheldAmount`: Fixed facility-level amount withheld from funded
 *   proceeds, pro-rated across draws by draw size
 * - `SeparateBp`: Discount in basis points of each draw, tracked
 *   separately and not withheld
 * - `SeparateAmount`: Fixed facility-level amount tracked separately,
 *   pro-rated across draws by draw size
 *
 * # Examples
 *
 * ```text
 * use finstack_quant_valuations::instruments::fixed_income::term_loan::spec::OidPolicy;
 * use finstack_quant_core::money::Money;
 * use finstack_quant_core::currency::Currency;
 *
 * // 2% OID withheld from proceeds
 * let oid = OidPolicy::WithheldBp(dec!(200));  // 200 bp = 2%
 *
 * // $50,000 fixed OID
 * let oid_fixed = OidPolicy::WithheldAmount(Money::from((50_000_i64, Currency::USD)));
 * ```
 */
export type OidPolicy =
  | {
      withheld_bp: DecimalWire;
    }
  | {
      withheld_amount: Money;
    }
  | {
      separate_bp: DecimalWire;
    }
  | {
      separate_amount: Money;
    };
/**
 * Method for calculating par rates in swaps
 */
export type ParRateMethod = "forward_based" | "discount_ratio";
/**
 * Clearing status for OTC derivatives.
 *
 * Determines whether a trade is cleared through a CCP or remains bilateral
 * under a CSA agreement.
 */
export type ClearingStatus =
  | "bilateral"
  | {
      cleared: {
        /**
         * CCP identifier (e.g., "LCH", "CME", "ICE", "JSCC")
         */
        ccp: string;
        [k: string]: unknown;
      };
    };
/**
 * Collateral asset classes per BCBS-IOSCO standards.
 *
 * Asset classes determine baseline haircuts and eligibility criteria.
 * The BCBS-IOSCO framework specifies minimum haircuts by asset class.
 *
 * # Reference
 *
 * BCBS-IOSCO "Margin requirements for non-centrally cleared derivatives" (2020)
 * Annex A: Standardized haircut schedule `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
 */
export type CollateralAssetClass =
  | "cash"
  | "government_bonds"
  | "agency_bonds"
  | "covered_bonds"
  | "corporate_bonds"
  | "equity"
  | "gold"
  | "mutual_funds";
/**
 * Initial margin calculation methodology.
 *
 * Different methodologies are used depending on regulatory requirements,
 * product type, and whether trades are cleared or bilateral.
 *
 * # BCBS-IOSCO Standards
 *
 * For bilateral (uncleared) OTC derivatives, either SIMM or the regulatory
 * schedule approach may be used. SIMM is the industry standard for large
 * dealers due to its risk-sensitivity.
 */
export type ImMethodology = "haircut" | "simm" | "schedule" | "internal_model" | "clearing_house";
/**
 * Margin call frequency.
 *
 * Determines how often margin calls are made and collateral is exchanged.
 * Industry standard for OTC derivatives is daily under BCBS-IOSCO rules.
 */
export type MarginTenor = "daily" | "weekly" | "monthly" | "on_demand";
/**
 * Explicit ISDA SIMM credit risk-class and bucket assignment.
 *
 * Corporate, sovereign, and index credit exposures belong to credit
 * qualifying, including high-yield sectors represented by SIMM buckets 7-12.
 * Credit non-qualifying is reserved for securitizations and other exposures
 * governed by the non-qualifying risk class.
 */
export type SimmCreditClassification =
  | {
      risk_class: "qualifying";
      /**
       * Sector bucket used for credit-qualifying delta aggregation.
       */
      sector: SimmCreditSector;
    }
  | {
      risk_class: "non_qualifying";
    };
/**
 * SIMM credit sector for bucket assignment.
 *
 * Maps reference entities to ISDA SIMM credit qualifying buckets.
 * See ISDA SIMM v2.6 Table 2.
 */
export type SimmCreditSector =
  | "sovereign"
  | "financial"
  | "basic_materials"
  | "consumer_goods"
  | "technology_media"
  | "health_care"
  | "high_yield_sovereign"
  | "high_yield_financial"
  | "high_yield_basic_materials"
  | "high_yield_consumer_goods"
  | "high_yield_technology_media"
  | "high_yield_health_care"
  | "residual";
/**
 * Direction for instrument legs (universal for IRS, CDS, variance swaps, etc.)
 *
 * For interest rate swaps: Pay = pay fixed/receive floating, Receive = receive fixed/pay floating
 * For credit default swaps: Pay = buy protection (pay premium), Receive = sell protection (receive premium)
 * For variance swaps: Pay = short variance, Receive = long variance
 */
export type PayReceive = "pay" | "receive";
/**
 * Cash settlement annuity method for cash-settled swaptions.
 *
 * The trade confirmation or ISDA settlement matrix determines the method.
 * Modern EUR cash-settled swaptions use collateralized cash price; legacy
 * trades may retain par-yield or ISDA par-par terms.
 */
export type CashSettlementMethod = "collateralized_cash_price" | "par_yield" | "isda_par_par" | "zero_coupon";
/**
 * Option payoff direction used by analytical and numerical model engines.
 */
export type OptionType = "call" | "put";
/**
 * Finite JSON number that is strictly greater than zero.
 *
 * This type is used by serde field adapters so runtime deserialization and
 * generated schemas enforce the same positive-number contract.
 */
export type PositiveF64Wire = number;
/**
 * Finite JSON number in the closed interval `[0, 1]`.
 */
export type ClosedUnitIntervalF64Wire = number;
/**
 * Finite JSON number greater than or equal to zero.
 */
export type NonNegativeF64Wire = number;
/**
 * Finite correlation coefficient in the closed interval `[-1, 1]`.
 */
export type CorrelationWire = number;
/**
 * Volatility convention (Black lognormal or Bachelier normal) used to price
 * a swaption. Wire values: `black`, `normal`.
 */
export type VolatilityModel = "black" | "normal";
/**
 * Standard FX conversion strategies used to hint FX providers.
 *
 * The policy tells a provider *how* the rate will be applied so it can decide
 * between spot, forward, or averaged sources.
 */
export type FxConversionPolicy = "cashflow_date" | "period_end" | "period_average";
/**
 * Payment priority levels in the waterfall.
 */
export type PaymentPriority =
  "fees" | "interest" | "amortization" | "mandatory_prepayment" | "voluntary_prepayment" | "sweep" | "equity";
/**
 * Typed reason for a capital-structure evaluation warning.
 */
export type CapitalStructureWarning =
  | {
      /**
       * Ratio used after applying the safety bound.
       */
      clamped_ratio: number;
      kind: "scale_clamped";
      /**
       * Unbounded ratio calculated from model and schedule balances.
       */
      raw_ratio: number;
      [k: string]: unknown;
    }
  | {
      /**
       * Original contractual payment date.
       */
      cashflow_date: DateWire;
      /**
       * Canonical cashflow classification that was excluded.
       */
      cashflow_kind: CFKind;
      kind: "cashflow_ignored";
      [k: string]: unknown;
    }
  | {
      /**
       * Negative claim amount in the waterfall currency.
       */
      amount: number;
      /**
       * Payment category containing the invalid negative claim.
       */
      category: CapitalStructureClaimCategory;
      /**
       * Instrument whose claim was neutralized.
       */
      instrument_id: string;
      kind: "negative_claim_neutralized";
      [k: string]: unknown;
    }
  | {
      /**
       * Negative amount that was floored, in the waterfall currency.
       */
      amount: number;
      kind: "negative_available_cash_floored";
      /**
       * Statement node supplying the available-cash amount.
       */
      node_id: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Unpaid amount in the waterfall currency.
       */
      amount: number;
      /**
       * Instrument carrying the unpaid claim forward.
       */
      instrument_id: string;
      kind: "interest_shortfall";
      [k: string]: unknown;
    }
  | {
      /**
       * Unpaid amount in the waterfall currency.
       */
      amount: number;
      /**
       * Instrument carrying the unpaid claim forward.
       */
      instrument_id: string;
      kind: "fee_shortfall";
      [k: string]: unknown;
    }
  | {
      /**
       * Unpaid amount in the waterfall currency.
       */
      amount: number;
      /**
       * Instrument carrying the unpaid claim forward.
       */
      instrument_id: string;
      kind: "principal_shortfall";
      [k: string]: unknown;
    };
/**
 * Category that groups related checks together.
 */
export type CheckCategory =
  | "accounting_identity"
  | "cross_statement_reconciliation"
  | "internal_consistency"
  | "credit_reasonableness"
  | "data_quality";
/**
 * Type-safe identifier for a node in a financial model.
 *
 * Serializes as a plain string and is interoperable with `&str` via
 * [`Borrow`] and [`AsRef`].
 */
export type NodeId = string;
/**
 * Severity level for a check finding, ordered from least to most severe.
 */
export type CheckSeverity = "info" | "warning" | "error";
/**
 * Warning emitted during evaluation.
 */
export type EvalWarning =
  | {
      division_by_zero: {
        /**
         * Identifier of the node that triggered the warning.
         */
        node_id: string;
        /**
         * Period in which the warning occurred.
         */
        period: string;
        [k: string]: unknown;
      };
    }
  | {
      nan_propagated: {
        /**
         * Identifier of the node that produced the NaN value.
         */
        node_id: string;
        /**
         * Period in which the warning occurred.
         */
        period: string;
        [k: string]: unknown;
      };
    }
  | {
      non_finite_value: {
        /**
         * Identifier of the node that produced the non-finite value.
         */
        node_id: string;
        /**
         * Period in which the warning occurred.
         */
        period: string;
        /**
         * The actual non-finite value, encoded as `"nan"`, `"inf"`, or `"-inf"`.
         */
        value: ResultNumber;
        [k: string]: unknown;
      };
    }
  | {
      capital_structure: {
        /**
         * Period in which the warning was raised.
         */
        period: string;
        /**
         * Typed reason and associated diagnostic values.
         */
        warning: CapitalStructureWarning;
        [k: string]: unknown;
      };
    }
  | {
      non_finite_skipped: {
        /**
         * Number of non-finite inputs dropped.
         */
        count: number;
        /**
         * Name of the aggregate function that dropped values.
         */
        function: string;
        /**
         * Identifier of the node whose aggregate dropped inputs.
         */
        node_id: string;
        /**
         * Period in which the drop occurred.
         */
        period: string;
        [k: string]: unknown;
      };
    };
export type ResultNumber = number | ("nan" | "inf" | "-inf");
/**
 * Available forecast methods.
 */
export type ForecastMethod =
  | "forward_fill"
  | "growth_pct"
  | "curve_pct"
  | "normal"
  | "log_normal"
  | "override"
  | "time_series"
  | "seasonal"
  | "fade_to_target"
  | "mean_reverting"
  | "bootstrap";
/**
 * Node computation type.
 *
 * Determines how a node's value is computed:
 * - **Value**: Only explicit values (actuals, assumptions)
 * - **Calculated**: Only formula-derived
 * - **Mixed**: Value OR Forecast OR Formula (precedence: Value > Forecast > Formula)
 */
export type NodeType = "value" | "calculated" | "mixed";
/**
 * Node value type classification.
 *
 * Determines whether a node represents monetary values (with a specific currency)
 * or scalar values (ratios, percentages, counts, etc.).
 */
export type NodeValueType =
  | {
      /**
       * Currency of the monetary value
       */
      currency: Currency;
      type: "monetary";
      [k: string]: unknown;
    }
  | {
      type: "scalar";
      [k: string]: unknown;
    };
/**
 * Identifier for a Gregorian period like `2025Q1` or a fiscal period like
 * `FY2025W53`.
 */
export type PeriodId = string;
/**
 * Numeric schema revision for contracts whose sole supported revision is v1.
 */
export type SchemaVersion = number;
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
 * Numeric mode used for evaluation.
 */
export type StatementNumericMode = "float64";

/**
 * Specification for a single adjustment (add-back or deduction).
 */
export interface Adjustment {
  /**
   * Optional cap on the adjustment amount
   */
  cap?: AdjustmentCap | null;
  /**
   * Category for grouping (optional)
   */
  category?: string | null;
  /**
   * Unique identifier for this adjustment
   */
  id: string;
  /**
   * Human-readable name (e.g., "Synergies", "Management Fees")
   */
  name: string;
  /**
   * How the adjustment value is calculated
   */
  value: AdjustmentValue;
}
/**
 * Defines a cap on an adjustment.
 */
export interface AdjustmentCap {
  /**
   * For self-referential caps (`base_node == target_node`), choose whether
   * to size the cap against the reported or the progressively adjusted
   * base. Defaults to `Reported`, the standard LBO / credit-agreement
   * convention. Ignored when `base_node` is `None` or points to a
   * different node.
   */
  base_mode?: CapBaseMode;
  /**
   * The base node to calculate the cap against (e.g., "EBITDA")
   * If None, the cap is a fixed absolute amount.
   */
  base_node?: string | null;
  /**
   * The percentage of the base node to cap at (e.g., 0.20 for 20%)
   * Or the absolute amount if base_node is None.
   */
  value: number;
}
/**
 * Currency-tagged monetary amount with safe arithmetic.
 *
 * Values retain decimal precision independently of ISO 4217 display precision.
 *
 * When you need configurable rounding during ingestion, use
 * [`Money::new_with_config`].
 *
 * # Examples
 * ```rust
 * use finstack_quant_core::money::Money;
 * use finstack_quant_core::currency::Currency;
 *
 * let notional = Money::from((1_000_000_i64, Currency::EUR));
 * assert_eq!(notional.currency(), Currency::EUR);
 * assert_eq!(notional.amount(), 1_000_000.0);
 * ```
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
 * User-defined tags and key-value metadata for instrument classification.
 */
export interface Attributes {
  /**
   * Structured metadata associated with the instrument.
   */
  meta?: {
    [k: string]: string;
  };
  /**
   * User-defined tags for categorization.
   */
  tags?: string[];
}
/**
 * Bond instrument with fixed, floating, or amortizing cashflows.
 *
 * Cashflow sign convention (holder view):
 * - All contractual cashflows **received by a long holder** (coupons,
 *   amortization, final redemption) are represented as **positive** amounts.
 * - Cash outflows for the holder (e.g., purchase price, funding, short
 *   positions) are represented as **negative** amounts and are handled at
 *   trade level rather than in the bond's contractual schedule.
 *
 * Supports call/put schedules, quoted prices for yield-to-maturity calculations,
 * and custom cashflow schedule overrides. Uses a clean `CashflowSpec` that wraps
 * the canonical builder coupon specs for maximum flexibility and parity.
 *
 * [`crate::instruments::Instrument::value`] is the **dirty NPV at `as_of`**,
 * not the market dirty quote at settlement. Quoted YTM, z-spread, and DM
 * remain settlement-anchored.
 */
export interface Bond {
  /**
   * Accrual method for interest calculation between coupon dates.
   *
   * Determines how accrued interest is calculated:
   * - `Linear` (default): Simple interest interpolation (most bonds)
   * - `Compounded`: Actuarial accrual per ICMA Rule 251 (some European bonds)
   *
   * For inflation-linked bonds (TIPS, UK Linkers), use the dedicated
   * `InflationLinkedBond` instrument which handles index-ratio accrual.
   */
  accrual_method?: AccrualMethod;
  /**
   * Attributes for scenario selection and tagging.
   */
  attributes: Attributes;
  /**
   * Optional call/put schedule (dates and redemption prices as % of par amount).
   */
  call_put?: CallPutSchedule | null;
  /**
   * Cashflow specification (fixed, floating, or amortizing).
   */
  cashflow_spec: CashflowSpec;
  /**
   * Optional credit curve identifier (default intensity). When present,
   * credit-rate pricing is enabled.
   */
  credit_curve_id?: Id | null;
  /**
   * Optional pre-built cashflow schedule. If provided, this will be used instead of
   * generating cashflows from the cashflow_spec.
   */
  custom_cashflows?: CashFlowSchedule | null;
  /**
   * Discount curve identifier for pricing.
   */
  discount_curve_id: Id;
  /**
   * Calendar identifier for ex-coupon day counting.
   */
  ex_coupon_calendar_id?: Id | null;
  /**
   * Number of ex-coupon days before coupon date.
   */
  ex_coupon_days?: number;
  /**
   * Unique identifier for the bond.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Issue date of the bond.
   */
  issue_date: DateWire;
  /**
   * Maturity date of the bond.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Principal amount of the bond.
   */
  notional: Money;
  /**
   * Optional repo (financing) discount curve for carry cost computation.
   */
  repo_curve_id?: Id | null;
  /**
   * Optional guaranteed minimum-return ("return floor") call protection.
   *
   * When present, the bond is treated as prepayable across the protection
   * window with redemption floored so the investor meets the target MOIC/XIRR.
   * Deterministic pricing lowers it into `call_put`; stochastic
   * rates-credit pricing evaluates it against each simulated distribution
   * and balance path.
   */
  return_floor?: ReturnFloorSpec | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Number of settlement days after trade date (e.g., 2 for T+2).
   */
  settlement_days?: number;
}
/**
 * Schedule of call and put options for a bond.
 *
 * Contains lists of call and put options that can be exercised during the bond's life.
 * Used for pricing callable/putable bonds and calculating yield-to-worst.
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
 * use finstack_quant_core::dates::Date;
 * use time::Month;
 *
 * let mut schedule = CallPutSchedule::default();
 * schedule.calls.push(CallPut {
 *     start: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
 *     end: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
 *     price_pct_of_par: 102.0,
 *     make_whole: None,
 * });
 * ```
 */
export interface CallPutSchedule {
  /**
   * Call options (issuer can redeem early).
   */
  calls: CallPut[];
  /**
   * Put options (holder can redeem early).
   */
  puts: CallPut[];
}
/**
 * Call or put option on a bond.
 *
 * Represents a single call or put option with an exercise period and redemption price.
 * Call options allow the issuer to redeem early; put options allow the holder to redeem early.
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_valuations::instruments::fixed_income::bond::CallPut;
 * use finstack_quant_core::dates::Date;
 * use time::Month;
 *
 * // Discrete call option: issuer can redeem at 102% of par on Jan 1, 2027
 * let call = CallPut {
 *     start: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
 *     end: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
 *     price_pct_of_par: 102.0,
 *     make_whole: None,
 * };
 * ```
 */
export interface CallPut {
  /**
   * Last date when the option can be exercised, inclusive.
   *
   * Use the same value as `start` for one-day/discrete exercise.
   */
  end: DateWire;
  /**
   * Optional make-whole call specification.
   *
   * When set, the call price is computed as:
   *   `max(clean_floor, PV of remaining cashflows - accrued) + accrued`
   *
   * Here `clean_floor` is notional times `price_pct_of_par / 100`; the
   * reference PV discounts remaining cashflows at the reference curve plus
   * spread, and accrued interest is paid exactly once.
   *
   * This ensures the holder is compensated at treasury + spread for early redemption.
   * Common in investment-grade corporate and convertible bonds.
   */
  make_whole?: MakeWholeSpec | null;
  /**
   * Clean redemption price as percentage of par amount (100 means par).
   * Accrued coupon interest at exercise is added to determine the cash payment.
   */
  price_pct_of_par: number;
  /**
   * First date when the option can be exercised.
   */
  start: DateWire;
}
/**
 * Make-whole call specification.
 *
 * Defines the reference curve and spread used to compute the make-whole redemption
 * price. The issuer pays the holder the greater of par and the present value of
 * remaining cashflows discounted at the reference rate plus a spread.
 *
 * # Industry Practice
 *
 * - Investment-grade corporates: typically Treasury + 25-50 bp
 * - High-yield: typically Treasury + 50-100 bp
 * - Convertibles: typically Treasury + 50 bp
 */
export interface MakeWholeSpec {
  /**
   * Reference curve identifier (e.g., "USD-TREASURY").
   */
  reference_curve_id: Id;
  /**
   * Spread over the reference curve in basis points (e.g., 50.0 = T+50bps).
   */
  spread_bp: number;
}
/**
 * Fixed-rate coupon specification.
 *
 * This type combines the coupon quote, payment behavior, and schedule
 * conventions required to emit a fixed-rate leg.
 */
export interface FixedCouponSpec {
  /**
   * Whether accrual-period boundaries are business-day adjusted with
   * `business_day_convention`.
   *
   * - `false` (default, bond convention): accrual periods run between the
   *   unadjusted schedule anchors; only payment dates roll.
   * - `true` (swap convention, ISDA 2006 §4.10 / ARRC SOFR conventions):
   *   both accrual-period boundaries are rolled with `business_day_convention` before year
   *   fractions and overnight observation windows are computed. The swap
   *   presets ([`Self::usd_sofr_swap`], [`Self::eur_estr_swap`],
   *   [`Self::gbp_sonia_swap`], [`Self::jpy_tona_swap`]) set this to
   *   `true`.
   *
   * Serialized only when `true`, so existing wire payloads are unchanged.
   */
  adjust_accrual_dates?: boolean;
  /**
   * Business-day convention applied when rolling **payment dates** onto
   * valid business days.
   *
   * Accrual boundaries are left unadjusted (bond/ICMA convention) unless
   * [`Self::adjust_accrual_dates`] is `true`, in which case the same
   * convention also rolls both accrual-period boundaries (swap/ISDA 2006
   * §4.10 convention).
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Holiday calendar identifier used together with `business_day_convention`.
   *
   * Use `"weekends_only"` when only Saturday/Sunday adjustment is needed.
   */
  calendar_id: Id;
  /**
   * Coupon settlement behavior: cash, PIK, or an explicit split of the
   * coupon amount.
   */
  coupon_type?: CouponType;
  /**
   * Day-count convention used to convert each generated accrual period into a
   * year fraction.
   */
  day_count: DayCount;
  /**
   * Whether end-of-month rolling should be preserved when generating the
   * schedule.
   */
  end_of_month?: boolean;
  /**
   * Accrual and payment frequency used to generate the schedule boundaries.
   */
  frequency: Tenor;
  /**
   * Payment lag in business days after the adjusted accrual end date.
   */
  payment_lag_days?: number;
  /**
   * Coupon rate as a decimal (e.g., 0.05 for 5%). Uses Decimal for exact representation.
   */
  rate: DecimalWire;
  /**
   * Roll-date rule for schedule anchors (standard IMM or CDS IMM grids).
   *
   * [`RollRule::None`] (default) keeps plain tenor stepping. The IMM modes
   * override `frequency`/`stub` with quarterly / short-back; see [`RollRule`].
   *
   * Serialized only when not `None`, so existing wire payloads are
   * unchanged.
   */
  roll_rule?: RollRule;
  /**
   * Stub-handling rule used when the start/end dates do not fit an exact
   * whole number of periods.
   */
  stub?: StubKind;
}
/**
 * A parsed tenor representing a time period.
 *
 * Tenors are commonly used in financial markets to specify maturities,
 * payment frequencies, and rate fixing periods.
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_core::dates::{Tenor, TenorUnit};
 * # fn main() -> finstack_quant_core::Result<()> {
 *
 * let tenor = Tenor::new(3, TenorUnit::Months).expect("valid tenor fixture");
 * assert_eq!(tenor.count(), 3);
 * assert_eq!(tenor.unit(), TenorUnit::Months);
 *
 * // Parse from string
 * let parsed = Tenor::parse("6M")?;
 * assert_eq!(parsed.count(), 6);
 * assert_eq!(parsed.unit(), TenorUnit::Months);
 * # Ok(())
 * # }
 * ```
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
 * Floating coupon specification (composes FloatingRateSpec).
 *
 * Used by the cashflow builder for instruments with floating rate coupons.
 * Embeds the canonical `FloatingRateSpec` for rate projection and adds
 * coupon-specific settings like payment frequency and PIK behavior.
 */
export interface FloatingCouponSpec {
  /**
   * Whether accrual-period boundaries are business-day adjusted with
   * `business_day_convention`.
   *
   * - `false` (default, bond convention): accrual periods run between the
   *   unadjusted schedule anchors; only payment dates roll.
   * - `true` (swap convention, ISDA 2006 §4.10 / ARRC SOFR conventions):
   *   both accrual-period boundaries are rolled with `business_day_convention` before year
   *   fractions and overnight observation windows are computed. The swap
   *   presets ([`Self::usd_sofr_swap`], [`Self::eur_estr_swap`],
   *   [`Self::gbp_sonia_swap`], [`Self::jpy_tona_swap`]) set this to
   *   `true`.
   *
   * Serialized only when `true`, so existing wire payloads are unchanged.
   */
  adjust_accrual_dates?: boolean;
  /**
   * Business-day convention applied when rolling **payment dates** onto
   * valid business days.
   *
   * Accrual boundaries are left unadjusted (bond/ICMA convention) unless
   * [`Self::adjust_accrual_dates`] is `true`, in which case the same
   * convention also rolls both accrual-period boundaries (swap/ISDA 2006
   * §4.10 convention).
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Holiday calendar identifier used together with `business_day_convention`.
   *
   * Use `"weekends_only"` when only Saturday/Sunday adjustment is needed.
   */
  calendar_id: Id;
  /**
   * Coupon type (Cash/PIK/Split).
   */
  coupon_type?: CouponType;
  /**
   * Day-count convention used to convert each generated accrual period into a
   * year fraction.
   */
  day_count: DayCount;
  /**
   * Whether end-of-month rolling should be preserved when generating the
   * schedule.
   */
  end_of_month?: boolean;
  /**
   * Accrual and payment frequency used to generate the schedule boundaries.
   */
  frequency: Tenor;
  /**
   * Payment lag in business days after the adjusted accrual end date.
   */
  payment_lag_days?: number;
  /**
   * Floating rate specification (contains index, spread, floor, cap, etc).
   */
  rate_spec: FloatingRateSpec;
  /**
   * Roll-date rule for schedule anchors (standard IMM or CDS IMM grids).
   *
   * [`RollRule::None`] (default) keeps plain tenor stepping. The IMM modes
   * override `frequency`/`stub` with quarterly / short-back; see [`RollRule`].
   *
   * Serialized only when not `None`, so existing wire payloads are
   * unchanged.
   */
  roll_rule?: RollRule;
  /**
   * Stub-handling rule used when the start/end dates do not fit an exact
   * whole number of periods.
   */
  stub?: StubKind;
}
/**
 * Canonical floating rate specification for all instruments.
 *
 * Used by bonds, swaps, credit facilities, and structured products.
 * All instruments should compose this type rather than defining their own
 * floating rate specifications.
 *
 * # Rate Calculation
 *
 * The all-in rate is computed as:
 * 1. Look up forward rate from `forward_curve_id` curve for the accrual period
 * 2. Apply `index_floor_bp` to index rate (if specified) - applied BEFORE adding spread
 * 3. Add `spread_bp` to get base rate
 * 4. Multiply by `gearing` (typically 1.0)
 * 5. Apply `all_in_cap_bp` to final rate (if specified) - applied AFTER spread and gearing
 *
 * Formula: `cap(gearing * (floor(index) + spread))`
 *
 * # Negative Rate Handling
 *
 * Negative index rates are supported and will flow through calculations
 * unless constrained by floors. For markets with negative rates (EUR, JPY, CHF):
 *
 * - Set `index_floor_bp: Some(0.0)` to floor the index at zero
 * - Set `all_in_floor_bp: Some(0.0)` to floor the total coupon at zero
 * - Omit floors to allow negative coupons (rare but valid in some structures)
 *
 * The implementation does not reject negative rates; the policy is controlled
 * by the floor configuration.
 *
 * # Seasoned Instruments (Historical Fixings)
 *
 * Historical fixings **are supported** via the `MarketContext`: store a
 * `ScalarTimeSeries` under the canonical id `FIXING:{forward_curve_id}` (see
 * `finstack_quant_core::market_data::fixings`) containing realized index
 * observations. Observation dates strictly before the forward curve base
 * date then resolve from that series instead of the curve:
 *
 * - **Overnight observations** (compounded/averaged paths) use LOCF lookup
 *   (last observation carried forward), matching RFR publication
 *   conventions where a fixing carries over non-publication days
 *   (ARRC 2020 SOFR conventions; ISDA 2021 Supp. 70 §7.1(g)). A partially
 *   seasoned compounding window seamlessly mixes realized fixings and
 *   curve-projected forwards with identical `(rate, days)` weighting.
 * - **Term-rate resets** use exact-date lookup on the (business-day
 *   adjusted) reset date — a term rate fixes on a specific published date.
 *   The fixing is the index rate only; gearing/spread/floors/caps apply on
 *   top exactly as for projected rates.
 *
 * An observation exactly on the curve base date prefers a published
 * same-day fixing when the series has one, otherwise projects from `t = 0`.
 *
 * The [`FloatingRateFallback`] policy applies only when **no** fixing
 * series is provided: `Error` (the default) fails the build with a
 * descriptive message naming the date, index, and expected series id;
 * `FixedRate(r)` uses `r` as the index rate for the affected coupon;
 * `SpreadOnly` projects spread-only.
 *
 * # Example
 *
 * ```rust
 * use finstack_quant_core::dates::Tenor;
 * use finstack_quant_cashflows::builder::{FloatingRateSpec, OvernightIndexConstraintApplication};
 * use rust_decimal_macros::dec;
 *
 * // 3M SOFR + 200bps with 0% floor
 * let spec = FloatingRateSpec {
 *     forward_curve_id: "USD-SOFR-3M".into(),
 *     spread_bp: dec!(200.0),
 *     gearing: dec!(1.0),
 *     gearing_includes_spread: true,
 *     index_floor_bp: Some(dec!(0.0)),
 *     all_in_floor_bp: None,
 *     all_in_cap_bp: None,
 *     index_cap_bp: None,
 *     overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
 *     reset_frequency: Tenor::quarterly(),
 *     index_tenor: None,
 *     reset_lag_days: 2,
 *     fixing_calendar_id: None,
 *     compounding: None,
 *     overnight_basis: None,
 *     fallback: Default::default(),
 * };
 * ```
 */
export interface FloatingRateSpec {
  /**
   * Cap on all-in rate in basis points (applied after spread and gearing).
   *
   * Example: all_in_cap_bp = Some(1000.0) ensures all-in rate <= 10%.
   */
  all_in_cap_bp?: DecimalWire | null;
  /**
   * Floor on all-in rate in basis points (Min Coupon).
   *
   * Applied to the final calculated rate after gearing and spread.
   */
  all_in_floor_bp?: DecimalWire | null;
  /**
   * How each accrual period's fixings combine into the period rate.
   *
   * An overnight variant (anything but `simple`) computes the period rate
   * from daily overnight fixings. `simple` projects one term forward over
   * the period. `None` leaves the choice to the instrument: pricers that
   * know the index resolve it from the rate-index convention registry, and
   * the bare cashflow builder treats it as `simple`.
   */
  compounding?: FloatingLegCompounding | null;
  /**
   * Policy when forward curve lookup fails during emission.
   *
   * Defaults to `Error`, which surfaces curve lookup failures.
   * Set to `SpreadOnly` for spread-only projection, or `FixedRate(r)`
   * to use a fixed index rate.
   */
  fallback?: FloatingRateFallback;
  /**
   * Optional calendar for rate fixing (reset lag).
   *
   * If not provided, defaults to the coupon schedule calendar.
   */
  fixing_calendar_id?: Id | null;
  /**
   * Forward curve identifier (e.g., "USD-SOFR-3M", "EUR-EURIBOR-6M").
   */
  forward_curve_id: Id;
  /**
   * Gearing/leverage multiplier applied to the all-in rate (default: 1.0).
   *
   * Example: gearing = 2.0 means the rate is doubled.
   *
   * **Restriction:** gearing must be strictly positive (`gearing > 0`);
   * projection rejects zero or negative gearing, so inverse floaters
   * (negative gearing) are not currently expressible with this field.
   */
  gearing?: DecimalWire;
  /**
   * Whether gearing includes the spread (default: true).
   *
   * - `true`: `rate = (index + spread) * gearing`
   * - `false`: `rate = (index * gearing) + spread` (Affine model)
   */
  gearing_includes_spread?: boolean;
  /**
   * Cap on index rate in basis points (applied to index component).
   */
  index_cap_bp?: DecimalWire | null;
  /**
   * Floor on index rate in basis points (applied to index component).
   *
   * Example: index_floor_bp = Some(0.0) ensures index rate >= 0%.
   */
  index_floor_bp?: DecimalWire | null;
  /**
   * Diagnostic tenor for term-index projection error context.
   *
   * The named forward curve is already the term index (for example a 3M
   * EURIBOR curve). Projection is `fwd.rate(reset_date)`, not a FRA-style
   * average over `[reset, reset + tenor]`. This field (or
   * [`Self::reset_frequency`] when `None`) is used only to compute
   * `index_maturity` for error messages. Ignored for overnight-compounded
   * legs. When set, the builder warns at build time if it disagrees with
   * the resolved curve's tenor by more than 10% — the curve remains
   * authoritative.
   */
  index_tenor?: Tenor | null;
  /**
   * Day-count basis for the overnight compounding denominator.
   *
   * This controls the annualization factor used when compounding daily
   * overnight fixings (e.g., 360 for SOFR/€STR/TONA, 365 for SONIA).
   * It is independent of the leg's accrual day count when set explicitly.
   *
   * When `None` and `compounding` is an overnight variant, the coupon
   * `schedule.day_count` is used if it is `Act360` or `Act365F`. Other
   * coupon day counts (for example `Thirty360`) error unless an explicit
   * `Act360` or `Act365F` basis is supplied. Ignored when
   * `compounding` is not an overnight variant.
   */
  overnight_basis?: DayCount | null;
  /**
   * Index floor/cap application policy for overnight-compounded coupons.
   */
  overnight_index_constraints?: OvernightIndexConstraintApplication;
  /**
   * Reset frequency for rate fixings.
   *
   * This is the cadence at which the rate refixes. When
   * [`Self::index_tenor`] is `None`, it is also the tenor used only to
   * build the diagnostic index-maturity date in projection error context.
   */
  reset_frequency: Tenor;
  /**
   * Reset lag in business days (e.g., 2 for T-2 SOFR convention).
   */
  reset_lag_days?: number;
  /**
   * Spread/margin over index in basis points. Uses Decimal for exact representation.
   */
  spread_bp: DecimalWire;
}
/**
 * Step-up/step-down coupon specification.
 *
 * Defines a coupon that changes rate at specified dates, commonly used
 * in bank capital instruments (AT1/Tier 2) and some agency bonds.
 *
 * The rate for each coupon period is determined by the last step date
 * that falls on or before the period start date. If no step has occurred,
 * the initial rate is used.
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_core::dates::{Date, DayCount, Tenor, BusinessDayConvention, StubKind};
 * use finstack_quant_cashflows::builder::{CouponType, ScheduleParams, StepUpCouponSpec};
 * use rust_decimal_macros::dec;
 * use time::Month;
 *
 * let spec = StepUpCouponSpec {
 *     coupon_type: CouponType::Cash,
 *     initial_rate: dec!(0.03),
 *     step_schedule: vec![
 *         (Date::from_calendar_date(2027, Month::January, 1).unwrap(), dec!(0.04)),
 *         (Date::from_calendar_date(2029, Month::January, 1).unwrap(), dec!(0.05)),
 *     ],
 *     schedule: ScheduleParams {
 *         frequency: Tenor::semi_annual(),
 *         day_count: DayCount::Thirty360,
 *         business_day_convention: BusinessDayConvention::Following,
 *         calendar_id: "weekends_only".into(),
 *         stub: StubKind::None,
 *         end_of_month: false,
 *         payment_lag_days: 0,
 *         adjust_accrual_dates: false,
 *         roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
 *     },
 * };
 * ```
 */
export interface StepUpCouponSpec {
  /**
   * Whether accrual-period boundaries are business-day adjusted with
   * `business_day_convention`.
   *
   * - `false` (default, bond convention): accrual periods run between the
   *   unadjusted schedule anchors; only payment dates roll.
   * - `true` (swap convention, ISDA 2006 §4.10 / ARRC SOFR conventions):
   *   both accrual-period boundaries are rolled with `business_day_convention` before year
   *   fractions and overnight observation windows are computed. The swap
   *   presets ([`Self::usd_sofr_swap`], [`Self::eur_estr_swap`],
   *   [`Self::gbp_sonia_swap`], [`Self::jpy_tona_swap`]) set this to
   *   `true`.
   *
   * Serialized only when `true`, so existing wire payloads are unchanged.
   */
  adjust_accrual_dates?: boolean;
  /**
   * Business-day convention applied when rolling **payment dates** onto
   * valid business days.
   *
   * Accrual boundaries are left unadjusted (bond/ICMA convention) unless
   * [`Self::adjust_accrual_dates`] is `true`, in which case the same
   * convention also rolls both accrual-period boundaries (swap/ISDA 2006
   * §4.10 convention).
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Holiday calendar identifier used together with `business_day_convention`.
   *
   * Use `"weekends_only"` when only Saturday/Sunday adjustment is needed.
   */
  calendar_id: Id;
  /**
   * Coupon type (Cash/PIK/Split).
   */
  coupon_type?: CouponType;
  /**
   * Day-count convention used to convert each generated accrual period into a
   * year fraction.
   */
  day_count: DayCount;
  /**
   * Whether end-of-month rolling should be preserved when generating the
   * schedule.
   */
  end_of_month?: boolean;
  /**
   * Accrual and payment frequency used to generate the schedule boundaries.
   */
  frequency: Tenor;
  /**
   * Initial coupon rate (annual, decimal). Used until the first step date.
   */
  initial_rate: DecimalWire;
  /**
   * Payment lag in business days after the adjusted accrual end date.
   */
  payment_lag_days?: number;
  /**
   * Roll-date rule for schedule anchors (standard IMM or CDS IMM grids).
   *
   * [`RollRule::None`] (default) keeps plain tenor stepping. The IMM modes
   * override `frequency`/`stub` with quarterly / short-back; see [`RollRule`].
   *
   * Serialized only when not `None`, so existing wire payloads are
   * unchanged.
   */
  roll_rule?: RollRule;
  /**
   * Step schedule: (effective_date, new_rate). Must be sorted by date.
   * Each entry sets the rate from that date forward until the next step.
   *
   * **Date convention:** `effective_date` is compared against each
   * accrual period's *unadjusted* `accrual_start`. Specify dates as
   * unadjusted accrual-period boundaries (typically the issue date plus
   * integer multiples of `frequency`); business-day adjustment is not
   * applied here. The rate is set at accrual start (per market
   * convention for step-up bonds).
   */
  step_schedule: [unknown, unknown][];
  /**
   * Stub-handling rule used when the start/end dates do not fit an exact
   * whole number of periods.
   */
  stub?: StubKind;
}
/**
 * Cashflow schedule output from the composable builder.
 *
 * Contains ordered cashflows plus notional and a representative `DayCount`.
 * Methods provide convenient accessors commonly used by pricing and analysis.
 */
export interface CashFlowSchedule {
  /**
   * Day count convention for interest calculations
   */
  day_count: DayCount;
  /**
   * Ordered cashflows (coupons, principal payments, fees)
   */
  flows: CashFlow[];
  /**
   * Additional metadata (calendars, facility limits)
   */
  meta: CashFlowMeta;
  /**
   * Notional schedule (constant or amortizing)
   */
  notional: Notional;
}
/**
 * A single dated cash-flow (payment or reset).
 *
 * Represents a monetary flow at a specific date with metadata
 * for proper classification and risk calculation.
 */
export interface CashFlow {
  /**
   * Optional contractual accrual metadata owned by this flow.
   */
  accrual?: CashFlowAccrual | null;
  /**
   * Accrual factor used for coupon amount and sensitivity.
   */
  accrual_factor: number;
  /**
   * Monetary amount including its currency.
   */
  amount: Money;
  /**
   * Payment date (or payment date for principal/fee, or reset date for `CFKind::FloatReset`).
   */
  date: DateWire;
  /**
   * Category/kind of cash-flow.
   */
  kind: CFKind;
  /**
   * Economic date of the principal movement, independent of cash payment.
   * When absent, principal changes on `date`. Scheduled amortization and
   * PIK use the contractual accrual boundary even when payment is adjusted.
   */
  principal_date?: DateWire | null;
  /**
   * Explicit change in outstanding principal, in the cashflow currency.
   * Positive increases outstanding. When absent, the flow kind and amount
   * determine the balance movement. Initial funding remains represented by
   * the schedule's initial notional rather than being counted twice.
   */
  principal_delta?: Money | null;
  /**
   * Effective rate used to calculate this cashflow (None if not rate-based or unknown).
   *
   * For interest/fees: the annual rate used in the calculation
   * For notional/amortization/PIK: typically None
   *
   * This is stored at cashflow creation time when available.
   * For instruments with intra-period events (e.g., revolving credit with draws/repays),
   * this may represent a time-weighted average rate across sub-periods.
   */
  rate?: number | null;
  /**
   * Optional index reset date (for floating coupons).
   */
  reset_date?: DateWire | null;
}
/**
 * Contractual accrual metadata attached to one cashflow.
 */
export interface CashFlowAccrual {
  /**
   * Calendar identifier for calendar-dependent day counts, including BUS/252.
   * Required for BUS/252; otherwise optional. Joint calendars use `+`.
   */
  calendar_id?: string | null;
  /**
   * Regular reference coupon period for ACT/ACT ICMA, including stub accrual.
   * `None` leaves reference-period selection to the schedule accrual caller.
   *
   * @minItems 2
   * @maxItems 2
   */
  coupon_period?: [unknown, unknown] | null;
  /**
   * Day-count convention used for the accrual factor.
   */
  day_count: DayCount;
  /**
   * Contractual accrual-period end date.
   */
  end: DateWire;
  /**
   * Whether `end` is the instrument termination date, for the 30E/360 ISDA
   * February exception. Intermediate coupon and rate-step ends are false.
   */
  end_is_termination_date?: boolean;
  /**
   * Projected index rate before spread, gearing, caps, or floors.
   */
  projected_index_rate?: number | null;
  /**
   * Contractual accrual-period start date.
   */
  start: DateWire;
}
/**
 * Metadata shared by an entire cashflow schedule.
 *
 * Tracks referenced calendar IDs, optional facility limits, and the instrument's
 * issue date for use by downstream engines (e.g., accrual calculation).
 */
export interface CashFlowMeta {
  /**
   * Holiday calendar IDs used for schedule adjustments.
   */
  calendar_ids: string[];
  /**
   * Total facility commitment for revolving and delayed-draw facilities;
   * `None` for instruments without one.
   */
  commitment?: Money | null;
  /**
   * Issue date of the instrument, when known.
   *
   * Used by the accrual engine to establish the first coupon period start
   * date precisely, avoiding the inverse day count approximation that can
   * be off by 1-2 days.
   */
  issue_date?: DateWire | null;
  /**
   * Contractual maturity date, distinct from an adjusted final payment date.
   */
  maturity?: DateWire | null;
  /**
   * Raw index observations used by projected floating coupons, before spread,
   * gearing, caps and floors; retained for deterministic time-roll fixings.
   */
  projected_fixings?: ProjectedFixing[];
  /**
   * Meaning of the schedule relative to waterfall policy.
   */
  representation?: CashflowRepresentation;
}
/**
 * One raw index observation needed by a projected floating coupon.
 */
export interface ProjectedFixing {
  /**
   * Contractual index observation date, after fixing-calendar adjustments.
   */
  date: DateWire;
  /**
   * Canonical market-series identifier, including the `FIXING:` prefix.
   */
  series_id: string;
  /**
   * Raw observed quantity in the index convention: annualized decimal rate
   * before spread/gearing/caps/floors for rates, or quote currency per base
   * currency for an FX fixing series. The series identifier fixes orientation.
   * `None` records an observation for which the coupon's fallback policy
   * masked a missing projection dependency; a time roll must supply an
   * existing fixing or fail explicitly when crossing that date.
   */
  value?: number | null;
}
/**
 * Notional amount with an optional amortisation rule.
 *
 * Combines initial principal with amortization behavior for complete
 * notional lifecycle management.
 */
export interface Notional {
  /**
   * Amortisation rule applied after each period.
   */
  amort: AmortizationSpec;
  /**
   * Initial principal amount outstanding at leg inception.
   */
  initial: Money;
}
/**
 * Instrument-owned pricing inputs that can materially change valuation.
 */
export interface InstrumentPricingOverrides {
  /**
   * Market-quoted values (prices, implied vol, spreads, upfront payments).
   */
  market_quotes?: MarketQuoteOverrides;
  /**
   * Model selection and tree pricing parameters.
   */
  model_config?: ModelConfig;
}
/**
 * Overrides for market-quoted values (prices, vols, spreads, upfront payments).
 *
 * # Price-driving fields
 *
 * The following fields, when set, override the model PV returned by
 * [`Instrument::base_value`](crate::instruments::common_impl::traits::Instrument::base_value)
 * for bonds. At most one may be set at a time — [`Self::validate`] enforces this.
 * Precedence (applied top-to-bottom inside `Bond::base_value`):
 *
 * 1. `quoted_dirty_price_currency` — currency units (bond native currency)
 * 2. `quoted_clean_price_pct` — percentage of par
 * 3. `quoted_ytm` — decimal YTM (e.g. `0.055` = 5.5%)
 * 4. `quoted_ytw` — decimal yield-to-worst
 * 5. `quoted_z_spread` — decimal Z-spread
 * 6. `quoted_oas` — decimal OAS
 * 7. `quoted_discount_margin` — decimal DM (FRNs)
 * 8. `quoted_i_spread` — decimal I-spread
 * 9. `quoted_asw_market` — decimal ASW (market convention)
 * 10. `quoted_japanese_simple_yield` — decimal Tokyo simple yield (単利)
 */
export interface MarketQuoteOverrides {
  /**
   * CreditDefaultSwap clean par-spread quote in basis points.
   *
   * Used only by CreditDefaultSwap risk replay, where it replaces the
   * matching contractual hazard-curve pillar. It does not drive PV, and no
   * other instrument (CdsIndex included) reads it.
   */
  cds_quote_bp?: number | null;
  /**
   * Implied volatility (overrides vol surface). When set on surface-driven
   * pricers, it is used as a flat σ across tenor and strike. Options on
   * futures read it as their only volatility input: decimal lognormal for
   * Black-76, futures-price points per √year for the normal model.
   */
  implied_volatility?: number | null;
  /**
   * Quoted asset-swap spread (market convention) in decimal.
   */
  quoted_asw_market?: number | null;
  /**
   * Quoted clean price in percent of par (e.g., `99.5` = 99.5% of par).
   *
   * Inflation-linked bonds quote this per 100 of real (unindexed) face.
   */
  quoted_clean_price_pct?: number | null;
  /**
   * Quoted dirty price in the bond's currency units.
   */
  quoted_dirty_price_currency?: number | null;
  /**
   * Quoted discount margin (for FRNs) in decimal.
   */
  quoted_discount_margin?: number | null;
  /**
   * Quoted I-spread in decimal.
   */
  quoted_i_spread?: number | null;
  /**
   * Quoted Japanese simple yield (単利) in decimal.
   *
   * Seeds a JGB from the Tokyo quoted yield without touching Street
   * [`Self::quoted_ytm`].
   */
  quoted_japanese_simple_yield?: number | null;
  /**
   * Quoted OAS (option-adjusted spread) in decimal.
   */
  quoted_oas?: number | null;
  /**
   * Observed option premium: the total trade PV in the instrument currency
   * (notional, contract multiplier and position included).
   *
   * This is the target every `ImpliedVol` calculator inverts; it does not
   * drive PV and is not one of the mutually exclusive price-driving fields.
   */
  quoted_premium?: number | null;
  /**
   * Quoted yield-to-maturity in decimal (e.g., `0.055` = 5.5%).
   */
  quoted_ytm?: number | null;
  /**
   * Quoted yield-to-worst in decimal.
   */
  quoted_ytw?: number | null;
  /**
   * Quoted Z-spread in decimal (e.g., `0.0125` = 125bp).
   */
  quoted_z_spread?: number | null;
}
/**
 * Model selection and tree pricing parameters.
 */
export interface ModelConfig {
  /**
   * Optional forward curve identifier for asset-swap spread metrics.
   *
   * When set, ASW par/market metrics project the floating receiver leg from
   * this forward curve instead of using a discount-curve par-rate proxy.
   */
  asw_forward_curve_id?: Id | null;
  /**
   * Black-Derman-Toy lognormal short-rate volatility (σ), as an annual
   * decimal proportion of the short rate (`0.20` = 20%).
   *
   * Read only by the rates-only bond tree when `tree_model = black_derman_toy` selects
   * BDT for a bond with embedded exercise rights, where it is required.
   * It is a relative (lognormal) volatility, unlike the absolute
   * [`Self::hw1f_sigma`]; typical values are 0.10–0.40. The BDT lattice has
   * no mean reversion. Must be finite and non-negative; `0.0` prices on the
   * deterministic curve.
   */
  bdt_sigma?: number | null;
  /**
   * Exercise friction cost for issuer/borrower calls, expressed as **cents per 100 of par**.
   *
   * This models the real-world costs of refinancing / reissue (fees, OID, documentation),
   * by requiring the issuer/borrower to see sufficient economic benefit before exercising.
   *
   * ## Convention
   * - `0.0` (or `None`) means frictionless optimal exercise (pure model)
   * - `50.0` means **$0.50 per $100** of outstanding principal (0.50 points)
   * - `200.0` means **$2.00 per $100** of outstanding principal (2.00 points)
   *
   * The friction affects the **exercise decision threshold**, but redemption still occurs
   * at the contractual call price.
   */
  call_friction_cents?: number | null;
  /**
   * Mean-reversion speed of the hazard factor (κ_λ) on the rates-credit
   * lattice, annualised.
   *
   * `None` and `0.0` both mean no reversion. Capped by
   * [`KAPPA_MAX`](finstack_quant_models::trees::two_factor_rates_credit::KAPPA_MAX),
   * above which the binomial lattice's conditional variance collapses far
   * enough to distort option values. Requires `credit_curve_id`.
   *
   * Note that mean reversion narrows the feasible correlation range: it
   * skews the per-node marginal transition probabilities away from ½, and
   * two Bernoulli marginals admit only correlations inside their Fréchet
   * bounds. At `KAPPA_MAX` on both factors over a five-year lattice the
   * feasible `|ρ|` can fall to around `0.12`. Calibration rejects an
   * unattainable correlation and reports the lattice-wide maximum, which is
   * also available up front from
   * [`RatesCreditTree::max_feasible_correlation`](finstack_quant_models::trees::two_factor_rates_credit::RatesCreditTree::max_feasible_correlation).
   */
  hazard_mean_reversion?: number | null;
  /**
   * Credit hazard-rate volatility for the two-factor rates-credit callable
   * lattice (σ_λ), annualised, in **absolute** decimal hazard-rate points
   * per √year.
   *
   * This is an additive-normal hazard volatility on the same scale as the
   * hazard rate itself: `0.02` means the instantaneous hazard diffuses by
   * about 2 percentage points of hazard per √year. It is **not** a relative
   * or lognormal credit-spread volatility — inserting a CDS-option quote
   * such as `0.35` here would be roughly an order of magnitude too large.
   * Convert a fractional spread vol first (see
   * [`models::credit::market_anchored`](finstack_quant_models::credit::market_anchored)):
   * `σ_λ = σ_fractional · λ_ref`.
   *
   * `None` and `0.0` are equivalent and both mean a **deterministic** credit
   * factor: the lattice still reprices the survival curve exactly, it just
   * carries no hazard diffusion. Requires `credit_curve_id` on the
   * instrument; setting it without one is a validation error rather than a
   * silent no-op.
   */
  hazard_sigma?: number | null;
  /**
   * Hull-White 1F mean-reversion speed override (κ), in annualised units.
   *
   * Companion to [`Self::hw1f_sigma`]. The two values must be supplied
   * together unless the market context contains a complete pre-fitted
   * scalar pair or volatility schedule. Typical values: 0.01–0.10.
   */
  hw1f_mean_reversion?: number | null;
  /**
   * Hull-White 1F short-rate absolute volatility override (σ), in annual decimal units.
   *
   * This is the **short-rate** σ used directly in the HW1F stochastic differential
   * equation `dr = [θ(t) − κr] dt + σ dW`. It is **not** an option implied
   * volatility (Black/Normal) and must not be confused with `implied_volatility`.
   *
   * Typical values: 0.005–0.015 (50–150 bp/year annualised short-rate vol).
   * A value of 0.20 (a typical lognormal swaption vol) would be approximately
   * 13–40× too large and would produce a wildly mis-priced HW tree.
   *
   * This override is valid only with [`Self::hw1f_mean_reversion`]. Pricing
   * requires a complete, positive, finite parameter pair (or a complete
   * pre-fitted pair/schedule in the market context), except for the
   * explicit zero the bond lattices accept (below); partial inputs are
   * rejected and no volatility surface is queried.
   *
   * This is the only short-rate volatility input of both bond lattices:
   * the rates-only Hull-White tree (callable bond without
   * `credit_curve_id`) and the **rates-credit** callable path. Neither
   * reads `implied_volatility`, which is an option quote; the rates-credit
   * path rejects it outright. On both bond lattices an explicit `0.0`
   * selects deterministic rates (the rates-only tree still requires a
   * positive `hw1f_mean_reversion`); it is the only rates-only setting
   * that prices floating coupons. The rates-credit mean reversion is
   * additionally capped by
   * [`KAPPA_MAX`](finstack_quant_models::trees::two_factor_rates_credit::KAPPA_MAX);
   * Hull-White trees on other paths keep their own wider range.
   */
  hw1f_sigma?: number | null;
  /**
   * Optional piecewise-constant Hull-White short-rate volatility schedule.
   *
   * When supplied, this replaces the scalar [`Self::hw1f_sigma`] override.
   * The schedule is left-continuous, starts at time zero, and carries
   * absolute annual short-rate volatilities.
   */
  hw1f_sigma_schedule?: PiecewiseConstantCurve | null;
  /**
   * Pre-calibrated flat LMM forward-rate volatility loading scale.
   *
   * This is the positive annualized decimal scale applied to the Bermudan
   * LMM loading shape. Obtain it from the top-level calibration helper;
   * pricing never reads or fits a volatility surface.
   */
  lmm_base_vol?: number | null;
  /**
   * Optional antithetic-variates override for Monte Carlo pricing.
   *
   * `None` keeps the selected pricer's default. Structured equity pricers
   * default to `true`; set `Some(false)` only for controlled diagnostics.
   */
  mc_antithetic?: boolean | null;
  /**
   * Optional independent-estimator count for Monte Carlo pricers.
   *
   * When set, overrides the selected pricer's default simulation size.
   * Antithetic sampling evaluates two factor paths per independent estimator.
   * The rates-credit bond engine defaults to 20,000 estimators per training
   * and pricing stage; other pricers retain their own defaults. Intended for
   * tests, benchmarks, and controlled revaluation, not as a market quote.
   */
  mc_paths?: number | null;
  /**
   * Optional Monte Carlo seed label.
   *
   * Monte Carlo pricers derive their RNG seed as
   * `derive_seed(instrument_id, label)`, so the same label always replays
   * the same random streams for the same instrument. `None` seeds with the
   * pricer's base label. Finite-difference Greeks set a fixed label on the
   * repriced clone so base and bumped legs share common random numbers.
   */
  mc_seed_scenario?: string | null;
  /**
   * Optional absolute target for the Monte Carlo confidence-interval
   * half-width in instrument currency.
   *
   * Engines that support adaptive sampling may stop before `mc_paths` after
   * their minimum sample count when this positive finite target is reached.
   * The rates-credit bond engine always consumes its fixed estimator budget
   * and validates this target against the final 95% confidence interval.
   */
  mc_target_ci_half_width?: number | null;
  /**
   * Merton Monte Carlo configuration for structural credit PIK pricing.
   *
   * When set (at `instrument_pricing_overrides.model_config.merton_mc_config`
   * or via the Rust builder), the `MertonMc` pricer in the registry uses this
   * model and grid; the path count, antithetic flag and seed label come from
   * `mc_paths`, `mc_antithetic` and `mc_seed_scenario` on this config.
   */
  merton_mc_config?: MertonMcOverride | null;
  /**
   * Price/accrual target convention for OAS inversion.
   */
  oas_price_basis?: OasPriceBasis;
  /**
   * Quote compounding convention for OAS inputs and outputs.
   */
  oas_quote_compounding?: Compounding;
  /**
   * Instantaneous correlation between the short-rate and hazard-rate
   * shocks on the rates-credit lattice, in `[-1, 1]`.
   *
   * `None` and `0.0` both mean independent factors. A non-zero value is
   * only meaningful when **both** factor volatilities are positive;
   * otherwise it is rejected as an inert input rather than ignored.
   * Requires `credit_curve_id`.
   *
   * Feasibility depends on the mean-reversion settings — see
   * [`Self::hazard_mean_reversion`].
   */
  rate_credit_correlation?: number | null;
  /**
   * Pool-granularity policy for structured-credit copula default models.
   *
   * When set, overrides the default
   * [`PoolGranularity::PerName`]
   * finite-pool simulation. Pass
   * `PoolGranularity::LargeHomogeneous` to opt into the closed-form LHP
   * fast-path for genuinely granular pools. Ignored by non-copula default
   * models and by non-structured-credit instruments.
   */
  structured_credit_pool_granularity?: PoolGranularity | null;
  /**
   * Optional discount curve identifier for tree-based option/OAS models.
   *
   * Some vendor OAS screens use a model curve distinct from the bond's pricing
   * or spread curve. When set, tree pricers calibrate to this curve while
   * non-tree spread metrics continue to use the instrument's discount curve.
   */
  tree_discount_curve_id?: Id | null;
  /**
   * Short-rate lattice for the rates-only bond tree (`hull_white` or
   * `black_derman_toy`). `None` selects Hull-White.
   */
  tree_model?: ShortRateTreeModel | null;
  /**
   * Number of time steps for tree-based pricing (e.g., 100)
   */
  tree_steps?: number | null;
  /**
   * Volatility surface extrapolation policy when `implied_volatility` is not set.
   */
  vol_surface_extrapolation?: VolSurfaceExtrapolation;
}
/**
 * A finite, left-continuous piecewise-constant curve.
 */
export interface PiecewiseConstantCurve {
  times: number[];
  values: number[];
  [k: string]: unknown;
}
/**
 * Configuration for Monte Carlo PIK bond pricing.
 */
export interface MertonMcConfig {
  /**
   * Barrier-crossing policy used for `MertonBarrierType::FirstPassage`.
   *
   * Default: `BrownianBridge` when the Merton model uses `FirstPassage`,
   * otherwise `Discrete`.
   */
  barrier_crossing: BarrierCrossing;
  /**
   * Optional market-calibration specification.
   *
   * When set, the pricer first calibrates a structural parameter
   * (barrier or asset vol) to match a market quote using low-path MC
   * with common random numbers, then re-prices with full paths.
   */
  calibration?: MertonMcCalibrationSpec | null;
  /**
   * Pre-computed discount factors for term-structure cashflow discounting.
   *
   * Each entry is `(year_fraction, discount_factor)`, sorted by time.
   * When set, cashflows are discounted using log-linear interpolation of
   * these factors instead of the flat `discount_rate`. The flat rate is
   * still used for the Merton risk-neutral drift.
   */
  cashflow_dfs?: [unknown, unknown][] | null;
  /**
   * Optional dynamic (notional-dependent) recovery rate model.
   *
   * Recovery on default is evaluated pathwise as
   * `DynamicRecoverySpec::recovery_at_notional(N(τ))`, which hard-clamps
   * the result to `[0, base_recovery]`. The clamp introduces a small kink
   * in recovery as a function of the accreted notional; paths far into
   * the clamped region all contribute the same (floored/capped) recovery.
   * No smoothed (e.g. logistic) recovery rule is applied.
   */
  dynamic_recovery?: DynamicRecoverySpec | null;
  /**
   * Optional endogenous (leverage-dependent) hazard rate model.
   */
  endogenous_hazard?: EndogenousHazardSpec | null;
  /**
   * Merton structural credit model.
   */
  merton: MertonModel;
  /**
   * PIK schedule controlling per-coupon cash/PIK/toggle behavior.
   */
  pik_schedule: PikSchedule;
  /**
   * Recovery on default as a decimal fraction in `[0, 1]`.
   *
   * `dynamic_recovery`, when set, takes precedence over this flat rate.
   */
  recovery_rate: number;
  /**
   * Time steps per year for the simulation grid.
   */
  steps_per_year: number;
  /**
   * Optional toggle exercise model for PIK/cash coupon decisions.
   * Active only for coupon dates where [`PikSchedule`] resolves to
   * [`PikMode::Toggle`].
   */
  toggle_model?: ToggleExerciseModel | null;
}
/**
 * Calibration settings for MC-to-market matching.
 *
 * When set on [`MertonMcConfig::calibration`], the pricer runs a low-path
 * bisection to solve for a structural parameter so that the cash base-case
 * MC price matches the target market quote, then re-prices with full paths.
 */
export interface MertonMcCalibrationSpec {
  /**
   * Search bracket for the calibrated parameter (low, high).
   * When `None`, auto-brackets based on the calibration parameter type.
   *
   * @minItems 2
   * @maxItems 2
   */
  bracket?: [unknown, unknown] | null;
  /**
   * Number of independent MC estimators used during calibration iterations (low paths).
   */
  low_paths: number;
  /**
   * Maximum bisection iterations.
   */
  max_iterations: number;
  /**
   * Which structural parameter to solve for.
   */
  parameter: CalibrationParameter;
  /**
   * Optional seed override used for the calibration run.
   */
  seed?: number | null;
  /**
   * Target market quote to match (interpreted at quote/settlement date).
   */
  target: BondQuoteInput;
  /**
   * Absolute tolerance on the **PV residual** (currency units at `as_of`).
   */
  tolerance_pv: number;
}
/**
 * Specification for dynamic (notional-dependent) recovery rate.
 *
 * Models the relationship between the accreted notional and the recovery
 * rate in default. As PIK accrual increases the notional relative to the
 * original base, recovery declines according to the chosen [`RecoveryModel`].
 */
export interface DynamicRecoverySpec {
  /**
   * Base (reference) notional `N_0`.
   */
  base_notional: number;
  /**
   * Base (reference) recovery rate `R_0`.
   */
  base_recovery: number;
  /**
   * Recovery model governing the notional-to-recovery mapping.
   */
  model: RecoveryModel;
}
/**
 * Specification for endogenous (leverage-dependent) hazard rate.
 *
 * Models the relationship between a firm's leverage and its instantaneous
 * hazard rate, enabling a feedback loop where PIK accrual increases the
 * notional (and hence leverage), which drives the hazard rate higher.
 */
export interface EndogenousHazardSpec {
  /**
   * Base (reference) hazard rate `lambda_0`.
   */
  base_hazard_rate: number;
  /**
   * Base (reference) leverage level `L_0`.
   */
  base_leverage: number;
  /**
   * Mapping function from leverage to hazard rate.
   */
  leverage_hazard_map: LeverageHazardMap;
}
/**
 * Merton structural credit model.
 *
 * Models a firm's equity as a call option on its assets, where default
 * occurs when asset value falls below the debt barrier.
 *
 * # Fields
 *
 * - `asset_value` (V_0): Current market value of the firm's assets.
 * - `asset_vol` (sigma_V): Annualized volatility of asset returns.
 * - `debt_barrier` (B): Face value of debt / default point.
 * - `risk_free_rate` (r): Continuous risk-free rate.
 * - `payout_rate` (q): Continuous dividend / payout yield on assets.
 * - `barrier_type`: Terminal or first-passage barrier monitoring.
 * - `dynamics`: Asset return dynamics specification.
 *
 * # Wire format
 *
 * Deserialization is routed through [`MertonModel::new_with_dynamics`] via
 * [`RawMertonModel`], so a model loaded from JSON satisfies exactly the same
 * invariants as one built in Rust. The serialized field set is unchanged.
 */
export interface MertonModel {
  /**
   * Current firm asset value `V_0`, in the issuer's reporting currency.
   * Strictly positive.
   */
  asset_value: number;
  /**
   * Asset volatility `sigma_V`, annualized and expressed as a decimal
   * fraction (`0.25` is 25%). Strictly positive.
   */
  asset_vol: number;
  /**
   * Whether default is tested only at maturity or continuously over the
   * life of the debt.
   */
  barrier_type: MertonBarrierType;
  /**
   * Default barrier `B`, the debt face value the asset value is compared
   * against. Strictly positive and in the same currency as `asset_value`.
   */
  debt_barrier: number;
  /**
   * Stochastic process governing the asset value.
   */
  dynamics: AssetDynamics;
  /**
   * Continuous payout (dividend) rate on assets, as a decimal fraction.
   * Reduces the drift of the asset process under the risk-neutral measure.
   */
  payout_rate: number;
  /**
   * Continuously compounded risk-free rate `r`, as a decimal fraction.
   */
  risk_free_rate: number;
  [k: string]: unknown;
}
/**
 * Hard threshold toggle configuration.
 */
export interface ThresholdToggle {
  /**
   * Direction for comparison.
   */
  direction: ThresholdDirection;
  /**
   * Credit metric to observe.
   */
  state_variable: CreditStateVariable;
  /**
   * Threshold value.
   */
  threshold: number;
  [k: string]: unknown;
}
/**
 * Stochastic (sigmoid) toggle configuration.
 */
export interface StochasticToggle {
  /**
   * Intercept of the logistic function: `P(PIK) = 1 / (1 + exp(-(intercept + sensitivity * state)))`.
   */
  intercept: number;
  /**
   * Sensitivity (slope) of the logistic function with respect to the state variable.
   */
  sensitivity: number;
  /**
   * Credit metric to observe.
   */
  state_variable: CreditStateVariable;
  [k: string]: unknown;
}
/**
 * Optimal toggle configuration using nested Monte Carlo simulation.
 *
 * At each coupon date the toggle runs a small nested MC to estimate the
 * equity value (call-option payoff on the firm's assets) under two
 * scenarios:
 *
 * 1. **Cash** – the firm pays out the coupon, reducing asset value by
 *    the coupon amount while notional stays unchanged.
 * 2. **PIK** – the coupon accretes to notional (no cash outflow), so
 *    asset value is preserved but the default barrier rises.
 *
 * PIK is elected when the estimated equity value under PIK exceeds
 * that under cash.  The nested simulation uses a simple GBM forward
 * evolution of asset value with a first-passage barrier check.
 */
export interface OptimalToggle {
  /**
   * Annualised asset volatility for the nested GBM simulation.
   */
  asset_vol: number;
  /**
   * Equity holder discount rate applied to the nested-MC payoff.
   *
   * Note: the nested simulation drifts under the risk-neutral measure
   * (`risk_free_rate`) but discounts here, so the intermediate equity
   * figures are decision inputs, not measure-consistent prices. The
   * rate cancels in the cash-vs-PIK comparison and does not bias the
   * elected branch.
   */
  equity_discount_rate: number;
  /**
   * Forward-looking horizon in years for the nested simulation (e.g. 1.0).
   */
  horizon: number;
  /**
   * Number of nested Monte Carlo paths for continuation value estimation.
   * Recommended range: 100–500.
   */
  nested_paths: number;
  /**
   * Risk-free rate (continuous) used as drift in the nested simulation.
   */
  risk_free_rate: number;
  [k: string]: unknown;
}
/**
 * Metric-time overrides derived from an instrument's pricing metadata.
 */
export interface MetricPricingOverrides {
  /**
   * Basis used for bond duration, convexity, and DV01-style risk metrics.
   */
  bond_risk_basis?: BondRiskBasis | null;
  /**
   * Breakeven configuration: which parameter to solve for and solve mode.
   */
  breakeven_config?: BreakevenConfig | null;
  /**
   * Bump sizes for finite-difference sensitivities.
   */
  bump_config?: BumpConfig;
  /**
   * Day basis for the per-day analytic option theta (`calendar_365` or
   * `trading_252`). `None` uses calendar-day theta (annual theta / 365).
   */
  theta_day_basis?: ThetaDayBasis | null;
  /**
   * Theta / carry horizon over which time decay is measured (for example
   * 1D, 1W, 1M, 3M; wire form `{"count": 1, "unit": "weeks"}`). Day and
   * week tenors roll a fixed number of days, month and year tenors roll
   * calendar months (EOM-aware). `None` uses one day.
   */
  theta_period?: Tenor | null;
  /**
   * Historical VaR / Expected Shortfall configuration override.
   */
  var_config?: VarConfig | null;
}
/**
 * Configuration for the breakeven calculator.
 */
export interface BreakevenConfig {
  /**
   * Solve mode.
   */
  mode: BreakevenMode;
  /**
   * Which valuation parameter to solve for.
   */
  target: BreakevenTarget;
}
/**
 * Bump sizes for finite-difference sensitivity calculations.
 */
export interface BumpConfig {
  /**
   * Enable adaptive bump sizes based on volatility and moneyness
   *
   * When true, bump sizes are scaled based on:
   * - Volatility level (higher vol → larger bumps)
   * - Time to expiry (longer dated → larger bumps)
   * - Moneyness (deep ITM/OTM → smaller bumps)
   *
   * Default: false (use fixed bump sizes)
   */
  adaptive_bumps?: boolean;
  /**
   * Custom credit spread bump size override (in basis points, e.g., 1.0 for 1bp).
   *
   * Used by CS01 calculations that bump par spreads / hazard calibration quotes.
   */
  credit_spread_bump_bp?: number | null;
  /**
   * Parallel rate bump in basis points (1.0 = 1bp).
   *
   * Sizes DV01, rho, foreign rho and forward PV01. Results stay reported per
   * 1bp: `(pv_bumped - pv) / rate_bump_bp`. `None` uses the
   * `valuations.sensitivities.v1` value (default 1bp).
   */
  rate_bump_bp?: number | null;
  /**
   * Spot bump as a decimal fraction of spot (0.01 = 1%).
   *
   * Sizes every finite-difference spot greek (delta, gamma, vanna, charm,
   * speed, color, FX delta). When set it also replaces the adaptive spot
   * bump. `None` uses the `valuations.sensitivities.v1` value (default 1%).
   */
  spot_bump_decimal?: number | null;
  /**
   * Absolute volatility bump in decimal volatility (0.01 = 1 vol point).
   *
   * Sizes every finite-difference volatility greek (vega, vanna, volga, FX
   * vega). `None` uses the `valuations.sensitivities.v1` value (default
   * 1 vol point). Results stay reported per 1 vol point.
   */
  vol_bump_decimal?: number | null;
  /**
   * Yield bump in basis points (1.0 = 1bp) for numerical yield duration and
   * convexity: InflationLinkedBond `RealDuration` and structured-credit
   * `DurationMod`/`Convexity`.
   *
   * `None` keeps each metric's default shock (1bp for duration, 10bp for
   * structured-credit convexity).
   */
  ytm_bump_bp?: number | null;
}
/**
 * Configuration for VaR calculation.
 *
 * Controls statistical properties such as confidence level and pricing method.
 * The historical window/observation count is derived from [`MarketHistory`].
 */
export interface VarConfig {
  /**
   * Confidence level (e.g., 0.95 for 95% VaR, 0.99 for 99% VaR)
   */
  confidence_level?: OpenUnitIntervalF64Wire;
  /**
   * VaR calculation method
   */
  method?: VarMethod;
  /**
   * Optional reporting currency for portfolio aggregation.
   *
   * When omitted, same-currency portfolios use their natural currency.
   * Mixed-currency portfolios must set this explicitly.
   */
  reporting_currency?: Currency | null;
}
/**
 * Guaranteed minimum-return call protection on a bond or loan.
 *
 * Attaching this spec declares that on any early issuer-called or prepaid
 * redemption within the protection window, the redemption price will be
 * floored so the investor's realized return meets the target. This is the
 * standard private-credit loan model (often called a "prepayment premium" or
 * "call protection" in credit agreements). The floor is anchored at the issue
 * date and issue price `V0`. The spec is lowered into a concrete call schedule
 * deterministically or evaluated path by path by the rates-credit LSMC engine.
 *
 * **Call-protection only**: the floor applies to EARLY issuer redemptions
 * within the [`ProtectionWindow`] and never at maturity. The held-to-maturity
 * path is unfloored. Use [`crate::metrics::MetricId::MoicToWorst`] /
 * [`crate::metrics::MetricId::XirrToWorst`] to see the honest worst-case
 * return across all paths including the unfloored maturity path.
 *
 * # Mathematical Foundation
 *
 * Let:
 * - `V0` = invested capital (issue price).
 * - `t` = early-redemption date.
 * - `cash_through(t)` = sum of positive cashflows received by the holder in
 *   the half-open interval `(issue, t]` (coupons and amortization; excludes
 *   the redemption itself).
 * - `yf(a, b)` = year fraction from date `a` to date `b` under `Act/365F`.
 *
 * **MOIC floor** — minimum money-on-invested-capital multiple `m`:
 *
 * ```text
 * dirty_required(t) = m · V0 − cash_through(t)
 * ```
 *
 * The clean redemption amount is `dirty_required(t) − accrued(t)`, floored by
 * the contractual clean call amount and par. The exercise engine adds
 * `accrued(t)` exactly once, so the holder's total cash meets the target when
 * the return floor binds.
 *
 * **XIRR floor** — minimum annualized IRR `r`:
 *
 * ```text
 * dirty_required(t) = (1 + r)^yf(issue, t)
 *                     · (V0 − Σ_{q ≤ t} coupon_q / (1 + r)^yf(issue, q))
 * ```
 *
 * The day-count convention for `yf` defaults to `Act/365F`, matching
 * [`finstack_quant_core::cashflow::xirr`] so the verification metrics
 * reproduce the floor target exactly.
 *
 * For both floors, accrued interest is subtracted before the clean redemption
 * is clamped at par and divided by outstanding notional. Accrued is added once
 * to the exercise proceeds by the pricing kernel.
 *
 * # Pricing behavior
 *
 * - Deterministic pricing produces a daily concrete call schedule.
 * - Stochastic factors use pathwise cash and PIK state in the bond LSMC engine,
 *   including term-reset and overnight floating coupons.
 * - Contractual make-whole calls compose with the floor: every issuer exercise
 *   amount is the greater of the return floor and that call's effective amount.
 * - **`min_moic` / `min_xirr` shortcuts** set
 *   [`ProtectionWindow::Full`] (prepayable across the bond's entire life).
 *   Narrow the window via [`ReturnFloorSpec::window`] if a no-call period
 *   applies.
 *
 * # Invariants
 *
 * - MOIC multiple must be positive (`> 0`).
 * - XIRR rate must be finite and greater than `-1` (i.e., `-100%`).
 * - A [`ProtectionWindow::Between`] window must have `start < end`.
 *
 * # References
 *
 * - **MOIC**: Standard private-equity and private-credit return metric.
 *   MOIC = total distributions / invested capital. See e.g. Rosenbaum, J. &
 *   Pearl, J. (2013). *Investment Banking: Valuation, Leveraged Buyouts, and
 *   Mergers & Acquisitions* (2nd ed.). Wiley Finance. `docs/REFERENCES.md#rosenbaum-pearl-2020`
 * - **XIRR / IRR**: Internal rate of return. See Brealey, R. A., Myers, S.
 *   C. & Allen, F. (2023). *Principles of Corporate Finance* (14th ed.).
 *   McGraw-Hill. Chapter 5 ("Net Present Value and Other Investment Criteria").
 * - **Call-protection / prepayment premiums**: Standard private-credit
 *   agreement term. LSTA (Loan Syndications and Trading Association).
 *   *The Handbook of Loan Syndications and Trading* (2nd ed.). Chapter on
 *   loan documentation and call protection.
 * - **Act/365F day count**: ISDA 2006 Definitions, Section 4.16(f). `docs/REFERENCES.md#isda-2006-definitions`
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_valuations::instruments::fixed_income::bond::{
 *     ReturnFloorSpec, IssuePrice,
 * };
 * use finstack_quant_core::types::Rate;
 *
 * // 1.20× MOIC floor at par, full protection window
 * let spec = ReturnFloorSpec::moic(1.20);
 * assert!(spec.validate().is_ok());
 *
 * // 10% XIRR floor with 98 OID, validated
 * let spec = ReturnFloorSpec::xirr(Rate::from_percent(10.0).expect("valid rate fixture"))
 *     .issue_price(IssuePrice::PctOfPar(98.0));
 * assert!(spec.validate().is_ok());
 * ```
 *
 * Attach to a bond using the fluent builder:
 *
 * ```rust
 * use finstack_quant_valuations::instruments::fixed_income::bond::{
 *     Bond, ReturnFloorSpec, ProtectionWindow,
 * };
 * use finstack_quant_core::currency::Currency;
 * use finstack_quant_core::money::Money;
 * use finstack_quant_core::types::Rate;
 * use time::macros::date;
 *
 * // 5-year loan with 1.25× MOIC floor active after a 2-year no-call period.
 * let loan = Bond::fixed(
 *     "LOAN-001",
 *     Money::from((1_000_000_i64, Currency::USD)),
 *     Rate::from_percent(10.0).expect("valid rate fixture"),
 *     date!(2025 - 01 - 01),
 *     date!(2030 - 01 - 01),
 *     finstack_quant_core::dates::StubKind::None,
 *     "USD-OIS",
 * )?
 * .with_return_floor(
 *     ReturnFloorSpec::moic(1.25)
 *         .window(ProtectionWindow::From(date!(2027 - 01 - 01))),
 * );
 * # Ok::<(), Box<dyn std::error::Error>>(())
 * ```
 */
export interface ReturnFloorSpec {
  /**
   * Day count convention for XIRR discounting. Defaults to Act/365F
   * (matching `core::cashflow::xirr`) when `None`. Ignored for MOIC floors.
   */
  day_count?: DayCount | null;
  /**
   * Issue price = invested capital `V0` (amount funded at issue). Default: par.
   */
  issue_price?: IssuePrice;
  /**
   * Which return metric is guaranteed, and its target.
   */
  kind: ReturnFloorKind;
  /**
   * Protection window. Default: [`ProtectionWindow::Full`].
   */
  window?: ProtectionWindow;
}
/**
 * Scenario-only valuation adjustments.
 */
export interface ScenarioPricingOverrides {
  /**
   * Scenario price shock as a decimal fraction (`-0.05` = a -5% price
   * shock).
   *
   * When set, valuation helpers apply it as a multiplier:
   * `price * (1 + scenario_price_shock_decimal)`.
   */
  scenario_price_shock_decimal?: number | null;
  /**
   * Scenario spread shock in basis points (e.g., `150.0` for +150 bp widening).
   *
   * Applied as an additional flat Z-spread during valuation by pricers that
   * support spread-based revaluation. Currently consumed by `Bond::base_value`
   * for bonds without embedded options, without an assigned credit curve, and
   * without a price-pinning quote override other than `quoted_z_spread`
   * (where the shock is additive on the quoted spread). See
   * [`Instrument::scenario_spread_shock_supported`](crate::instruments::common_impl::traits::Instrument::scenario_spread_shock_supported).
   *
   * Setting this on an unsupported configuration produces a validation error
   * at pricing time rather than a silent no-op. For hazard-priced (credit
   * curve) bonds, shock the hazard curve instead (e.g. a par-CDS curve bump).
   */
  scenario_spread_shock_bp?: number | null;
}
/**
 * Interest rate option instrument.
 *
 * # Pre-1.0 API evolution
 *
 * Compounded-RFR contractual terms and contractual spread are typed public
 * fields. Adding them is intentionally source-breaking for downstream exhaustive
 * struct literals; canonical constructors and the generated builder retain
 * backward-compatible defaults. Prefer those construction APIs for forward
 * compatibility.
 */
export interface CapFloor {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Schedule business day convention
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional holiday calendar identifier for schedule and roll conventions
   */
  calendar_id?: Id | null;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Discount curve identifier
   */
  discount_curve_id: Id;
  /**
   * Exercise style (defaults to European; caps/floors are virtually always European)
   */
  exercise_style?: ExerciseStyle;
  /**
   * Forward curve identifier
   */
  forward_curve_id: Id;
  /**
   * Payment frequency for caps/floors
   */
  frequency: Tenor;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * End date of underlying period
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount
   */
  notional: Money;
  /**
   * Optional compounded-overnight coupon terms.
   *
   * `None` preserves the legacy term-index/simple-forward caplet contract.
   * Set this explicitly for caps on compounded SOFR, SONIA, €STR, or another
   * overnight RFR coupon.
   */
  overnight_coupon?: OvernightCouponConvention | null;
  /**
   * Optional dated premium paid by the cap/floor holder.
   *
   * A positive amount is an outflow from the holder and reduces NPV while
   * the payment date is strictly after `as_of`. The premium currency must
   * match the notional currency.
   *
   * @minItems 2
   * @maxItems 2
   */
  premium?: [unknown, unknown] | null;
  /**
   * Option type
   */
  rate_option_type: RateOptionType;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Settlement type (defaults to Cash; caps/floors are virtually always cash-settled)
   */
  settlement?: SettlementType;
  /**
   * Contractual margin added to the referenced rate, in basis points (10 = 10bp).
   *
   * Term-index coupons add this spread after projecting the index. For
   * overnight coupons, [`OvernightSpreadCompounding`] determines whether it
   * is added after compounding or included in every daily factor.
   */
  spread_bp?: DecimalWire;
  /**
   * Start date of underlying period
   */
  start_date: DateWire;
  /**
   * Strike (as decimal, e.g., 0.05 for 5%)
   */
  strike: DecimalWire;
  /**
   * Schedule stub convention
   */
  stub?: StubKind;
  /**
   * Displacement shift for shifted-lognormal pricing (default: 0.0 = no shift).
   *
   * When `vol_type = ShiftedLognormal`, rates and strikes are shifted by this amount:
   * `F' = F + vol_shift`, `K' = K + vol_shift`.
   *
   * Typical values are 0.01–0.03 (1%–3%) to push rates into positive territory
   * in low-rate environments. A shift of 0.0 is equivalent to plain lognormal.
   *
   * **Validation**: Must be ≥ 0.0. The shifted forward `F + vol_shift` must be
   * positive for the Black model to be well-defined.
   */
  vol_shift?: number;
  /**
   * Volatility surface identifier
   */
  vol_surface_id: Id;
  /**
   * Volatility type convention (lognormal/Black or normal/Bachelier).
   *
   * **Critical**: This must match the convention of your vol surface data.
   * Using lognormal vol with a normal surface (or vice versa) will produce
   * incorrect prices.
   *
   * - `Auto` (default): follow source convention and displacement metadata.
   * - `Lognormal`: Black model, with the source displacement if present;
   *   shifted forward and strike must both be positive.
   * - `Normal`: Bachelier model using normal quotes in decimal rate units;
   *   handles negative rates. Incompatible source conventions return errors.
   */
  vol_type?: CapFloorVolType;
}
/**
 * Contractual terms for an option on a compounded overnight RFR coupon.
 *
 * The shared [`FloatingLegCompounding`] type is reused so lookback,
 * observation-shift, and rate-cutoff semantics cannot drift from IRS pricing.
 * Payment and fixing calendars are separate because operational payment
 * delays need not use the index publication calendar.
 */
export interface OvernightCouponConvention {
  /**
   * Daily overnight compounding convention.
   */
  compounding: FloatingLegCompounding;
  /**
   * Calendar used for overnight observations and fixings.
   */
  fixing_calendar_id?: Id | null;
  /**
   * Calendar used to apply the payment delay.
   */
  payment_calendar_id?: Id | null;
  /**
   * Payment lag in business days after the accrual end date.
   */
  payment_lag_days?: number;
  /**
   * Whether any contractual spread is compounded or added afterward.
   */
  spread_compounding?: OvernightSpreadCompounding;
}
/**
 * Aggregated cashflows from capital structure instruments by period.
 *
 * Instances of this type are produced by the evaluator and exposed to the DSL
 * via the `cs.*` namespace. It keeps both per-instrument details and totals so
 * that downstream consumers can drill down or report aggregates.
 *
 * Monetary fields are stored as [`Money`] to preserve currency identity. The
 * accessor methods return raw `f64` amounts in the reporting currency for
 * convenience; callers that need full currency fidelity should inspect the
 * underlying maps directly.
 *
 * # Example
 *
 * ```rust
 * # use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
 * # use finstack_quant_core::dates::PeriodId;
 * # use finstack_quant_core::money::Money;
 * # use finstack_quant_core::currency::Currency;
 * let mut cs = CapitalStructureCashflows::new();
 * let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
 * cs.by_instrument
 *     .entry("BOND-1".into())
 *     .or_default()
 *     .insert(period, CashflowBreakdown {
 *         interest_expense_cash: Money::from((10_000_i64, Currency::USD)),
 *         interest_income_cash: None,
 *         interest_expense_pik: Money::from((2_500_i64, Currency::USD)),
 *         principal_payment: Money::from((100_000_i64, Currency::USD)),
 *         fees: Money::from((0_i64, Currency::USD)),
 *         debt_balance: Money::from((4_900_000_i64, Currency::USD)),
 *         accrued_interest: Money::from((5_000_i64, Currency::USD)),
 *     });
 * cs.totals.insert(period, CashflowBreakdown {
 *     interest_expense_cash: Money::from((10_000_i64, Currency::USD)),
 *     interest_income_cash: None,
 *     interest_expense_pik: Money::from((2_500_i64, Currency::USD)),
 *     principal_payment: Money::from((100_000_i64, Currency::USD)),
 *     fees: Money::from((0_i64, Currency::USD)),
 *     debt_balance: Money::from((4_900_000_i64, Currency::USD)),
 *     accrued_interest: Money::from((5_000_i64, Currency::USD)),
 * });
 *
 * assert_eq!(cs.get_total_interest(&period).unwrap(), 12_500.0);
 * ```
 */
export interface CapitalStructureCashflows {
  /**
   * Map of instrument_id → (period_id → cashflow_type → amount)
   */
  by_instrument: {
    [k: string]: {
      [k: string]: CashflowBreakdown;
    };
  };
  /**
   * Post-waterfall residual cash distributed to equity per period.
   *
   * Only populated by waterfall evaluation when `available_cash_node` is
   * configured: it is the cash remaining after fees, interest, and
   * principal allocations. With the per-period allocations this satisfies
   * `fees + cash interest + principal + equity == available cash`.
   */
  equity_distribution?: {
    [k: string]: Money;
  };
  /**
   * Reporting currency used for `totals` (if populated)
   */
  reporting_currency?: Currency | null;
  /**
   * Total cashflows across all instruments in the reporting currency (if available)
   */
  totals?: {
    [k: string]: CashflowBreakdown;
  };
  /**
   * Totals bucketed by native instrument currency
   */
  totals_by_currency?: {
    [k: string]: {
      [k: string]: CashflowBreakdown;
    };
  };
}
/**
 * Breakdown of cashflows by type for a single period.
 *
 * Outflow-like fields such as interest, fees, and principal payments are
 * stored as positive amounts representing debt service paid or accrued during
 * the period.
 *
 * Interest expense is split into cash and PIK components for visibility
 * into non-cash interest accrual. Use `interest_expense_total()` for the
 * combined value. All monetary fields use the Money type for currency safety.
 *
 * Interest *expense* and interest *income* are tracked separately rather than
 * as one signed field: a two-leg instrument (an interest-rate swap) can net to
 * a receipt in a period — a pay-fixed hedge is in the money whenever the
 * floating leg exceeds the fixed leg — and a receipt is not debt service. The
 * waterfall allocates cash against expense claims, so folding a receipt into
 * `interest_expense_cash` as a negative claim would corrupt pro-rata
 * allocation. See [`Self::net_interest_expense_cash`] for the combined view.
 */
export interface CashflowBreakdown {
  /**
   * Accrued interest not yet paid (liability)
   */
  accrued_interest: Money;
  /**
   * Outstanding debt balance at period end
   */
  debt_balance: Money;
  /**
   * Fees (commitment fees, etc.)
   */
  fees: Money;
  /**
   * Cash interest payments (coupons, floating resets)
   */
  interest_expense_cash: Money;
  /**
   * PIK (payment-in-kind) interest accrued but not paid in cash
   */
  interest_expense_pik: Money;
  /**
   * Net cash interest **received** during the period.
   *
   * Non-zero only for two-leg instruments whose legs net to a receipt (e.g.
   * an in-the-money pay-fixed swap). Stored as a positive amount, like the
   * outflow-oriented fields, and reported through the `cs.interest_income`
   * namespace.
   *
   * `None` means that no income leg applies. Read it via
   * [`Self::interest_income_cash_or_zero`] when a currency-preserving zero
   * is more convenient than an optional value.
   */
  interest_income_cash?: Money | null;
  /**
   * Principal repayments (amortization, maturity)
   */
  principal_payment: Money;
}
/**
 * Capital structure specification.
 */
export interface CapitalStructureSpec {
  /**
   * Debt instruments (bonds, loans, swaps)
   */
  debt_instruments?: DebtInstrumentSpec[];
  /**
   * Optional FX conversion policy override.
   *
   * When omitted, `cs.*` cash items and balances convert on the inclusive
   * period-end date (`FxConversionPolicy::PeriodEnd`). Conversion applies to
   * the already-aggregated period bucket, not per contractual cashflow date.
   */
  fx_policy?: FxConversionPolicy | null;
  /**
   * Additional metadata
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Optional reporting currency override for capital structure totals
   */
  reporting_currency?: Currency | null;
  /**
   * Optional waterfall specification for dynamic cash flow allocation
   */
  waterfall?: WaterfallSpec | null;
}
/**
 * Debt instrument specification.
 *
 * An identifier paired with a supported financial-statement instrument.
 */
export interface DebtInstrumentSpec {
  /**
   * Instrument identifier (key within the capital structure).
   */
  id: string;
  /**
   * Tagged instrument payload: `{"type": "...", "spec": {...}}`.
   */
  spec: FinancialStatementInstrument;
}
/**
 * Convertible bond instrument with embedded equity conversion option.
 *
 * This fixed income instrument combines debt characteristics (coupons, principal)
 * with equity optionality (conversion rights). Uses the `CashFlowBuilder` for
 * robust schedule generation and tree-based pricing for the hybrid valuation.
 */
export interface ConvertibleBond {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Optional call/put schedule (issuer/holder redemption before maturity).
   */
  call_put?: CallPutSchedule | null;
  /**
   * Coupon leg (fixed, floating, step-up or amortizing), the same type as
   * `Bond.cashflow_spec`. A zero-coupon convertible is a fixed spec with
   * rate `0`.
   */
  cashflow_spec: CashflowSpec;
  /**
   * Conversion terms for equity conversion.
   */
  conversion: ConversionSpec;
  /**
   * Issuer hazard curve identifier (a `HazardCurve`, as on every credit
   * instrument). When `None`, the cash component is discounted at the
   * risk-free `discount_curve_id` (no credit spread).
   *
   * The pricer derives the zero-recovery risky discount factor
   * `risky_df = rf_df × S(t)` from the curve's survival probabilities and
   * blends it with risk-free discounting as `risky × (1 − R) + rf × R`
   * using [`Self::recovery_rate`]. The hazard curve's own recovery rate is
   * used only to convert spread bumps into hazard shifts for CS01.
   */
  credit_curve_id?: Id | null;
  /**
   * Discount curve identifier for the debt component (risk-free or funding).
   */
  discount_curve_id: Id;
  /**
   * Optional unitless continuous dividend-yield scalar id (decimal,
   * 0.02 = 2%). `None` means a zero dividend yield; a configured id must
   * resolve.
   */
  div_yield_id?: Id | null;
  /**
   * Unique identifier for the instrument.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Issue date.
   */
  issue_date: DateWire;
  /**
   * Maturity date.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Principal amount.
   */
  notional: Money;
  /**
   * Assumed recovery rate on default, as a fraction (e.g., 0.40 = 40%).
   *
   * Used in the Tsiveriotis-Zhang credit model to blend risky and risk-free
   * discounting on the cash component. A recovery rate of 0 reduces to the
   * standard zero-recovery TZ model. Typical values:
   * - **Investment grade**: 0.40 (ISDA standard assumption)
   * - **High yield**: 0.25-0.35
   * - **Distressed**: 0.10-0.20
   *
   * Required when `credit_curve_id` is set; otherwise absence means that no
   * credit adjustment is requested.
   */
  recovery_rate?: number | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Number of business days from trade date to settlement date.
   *
   * When set, accrued interest and clean price are computed relative to the
   * settlement date (trade date + settlement_days business days) rather than
   * the valuation date. Standard values:
   * - **US corporate convertibles**: 2 (T+2)
   * - **US Treasury**: 1 (T+1)
   *
   * If `None`, settlement is assumed same-day (as_of = settlement date).
   */
  settlement_days?: number | null;
  /**
   * Optional soft-call trigger condition.
   *
   * When set, the issuer can only exercise call provisions if the underlying
   * stock price satisfies the trigger condition (e.g., above 130% of conversion
   * price for 20 of 30 trading days).
   *
   * **Modeling scope**: a single trigger gates the ENTIRE callable life —
   * every window in [`Self::call_put`] is subject to it. A soft-call period
   * followed by an unconditional hard-call period is not representable
   * (omit the trigger to model an unconditional call). In the tree the
   * trigger is evaluated on the instantaneous node spot with a
   * Broadie-Glasserman-Kou-style barrier adjustment approximating the
   * k-of-n-days observation window; the realized path is not tracked.
   */
  soft_call_trigger?: PriceTrigger | null;
  /**
   * Market-scalar id (`MarketContext::get_price`) of the underlying share
   * price, as a price in the bond's currency or a unitless level.
   */
  spot_id: Id;
  /**
   * Equity volatility id: a volatility surface read at the conversion strike
   * and maturity, or a unitless scalar holding a flat volatility.
   */
  vol_surface_id: Id;
}
/**
 * Conversion specification for the instrument.
 */
export interface ConversionSpec {
  /**
   * Anti-dilution protection policy.
   */
  anti_dilution: AntiDilutionPolicy;
  /**
   * Historical dilution events that affect the conversion ratio, recorded
   * in chronological (non-decreasing date) order and applied in that
   * order. Validation rejects out-of-order events.
   */
  dilution_events?: DilutionEvent[];
  /**
   * Dividend adjustment mechanism.
   */
  dividend_adjustment: DividendAdjustment;
  /**
   * Policy governing conversion timing/conditions.
   */
  policy: ConversionPolicy;
  /**
   * Conversion price (price per share). If not provided, derive from ratio.
   */
  price?: number | null;
  /**
   * Conversion ratio (shares per bond). If not provided, derive from price.
   */
  ratio?: number | null;
}
/**
 * A dilutive event that triggers anti-dilution adjustment.
 *
 * Records details of an equity issuance or corporate action that may
 * affect the conversion ratio under the bond's anti-dilution provisions.
 */
export interface DilutionEvent {
  /**
   * Date of the dilutive event.
   */
  date: DateWire;
  /**
   * New issue price per share (for below-market issuances).
   */
  new_issue_price: number;
  /**
   * Number of new shares issued.
   */
  new_shares_issued: number;
  /**
   * Number of shares outstanding before the event.
   */
  shares_outstanding_before: number;
}
/**
 * Share-price trigger of a convertible bond: the last sale price must be at
 * least `threshold_pct` of the conversion price on `required_days_above` of
 * `observation_days` consecutive trading days.
 *
 * Used for the issuer's soft call (`ConvertibleBond::soft_call_trigger`) and
 * the holder's contingent conversion
 * (`ConversionPolicy::UponEvent(ConversionEvent::PriceTrigger(..))`).
 *
 * # Industry Practice
 *
 * The standard trigger is 130% of the conversion price on 20 of 30
 * consecutive trading days; some issues use 120% or 150%.
 *
 * # Modeling scope
 *
 * The tree evaluates the trigger on the instantaneous node spot. The soft
 * call applies a Broadie-Glasserman-Kou-style barrier shift scaled by
 * `required_days_above / observation_days`; contingent conversion compares
 * the node spot with the nominal level and does not model the observation
 * window.
 */
export interface PriceTrigger {
  /**
   * Number of trading days in the observation window (e.g., 30).
   */
  observation_days: number;
  /**
   * Minimum number of days within the window on which the share price
   * must be at or above the level (e.g., 20 of 30).
   */
  required_days_above: number;
  /**
   * Trigger level as a percent of the conversion price (`130.0` = 130%);
   * must exceed 100.
   */
  threshold_pct: number;
}
/**
 * Revolving credit facility instrument.
 *
 * Models a credit facility with draws/repayments, interest payments on drawn
 * amounts, and fees (commitment, usage, facility, upfront). Supports both
 * deterministic schedules and stochastic utilization via Monte Carlo.
 *
 * See unit tests and `examples/` for usage.
 */
export interface RevolvingCredit {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Business-day convention applied to payment dates (interest, fees and
   * principal) and to the fixing-date roll. Accrual boundaries stay
   * unadjusted. Defaults to `ModifiedFollowing`.
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Holiday calendar identifier (for example `"usny"`) used to adjust
   * payment dates, roll fixing dates and count settlement days. `None`
   * adjusts for weekends only. Validation rejects an unknown identifier.
   * `FloatingRateSpec::fixing_calendar_id` overrides it for the fixing
   * date alone.
   */
  calendar_id?: Id | null;
  /**
   * Opening commitment of the facility, in force from `issue_date`
   * until the first entry of `commitment_steps`.
   */
  commitment: Money;
  /**
   * Scheduled commitment changes (amortizing commitments, availability
   * expiries, accordions), each in force from its date until the next.
   * Dates must be strictly increasing, after `issue_date` and on or
   * before `maturity`; the drawn balance plus outstanding letters of
   * credit must never exceed the commitment in force. A step down pays its
   * `reduction_fee_bp` on the reduced amount. Empty by default.
   */
  commitment_steps?: CommitmentStep[];
  /**
   * Optional credit curve identifier for credit risk modeling.
   *
   * When provided, survival probabilities from the hazard curve are applied
   * to discount cashflows, adjusting for default risk.
   */
  credit_curve_id?: Id | null;
  /**
   * Day count convention for interest accrual.
   */
  day_count: DayCount;
  /**
   * Discount curve identifier for pricing.
   */
  discount_curve_id: Id;
  /**
   * Draw and repayment schedule (deterministic or stochastic).
   */
  draw_repay_spec: DrawRepaySpec;
  /**
   * Drawn balance at the simulation anchor, the later of `issue_date`
   * and the valuation date, in both deterministic and stochastic mode.
   *
   * For a new facility this is the balance funded at commitment; for a
   * seasoned facility it is the balance observed on the valuation date.
   * Deterministic draw/repay events describe the future only: an event
   * dated on or before the valuation date is rejected by the cashflow
   * engine, because the position at the anchor is defined by this field
   * alone. The accrual period containing the valuation date accrues on
   * this balance from its accrual start.
   */
  drawn: Money;
  /**
   * Fee structure for the facility.
   */
  fees: RevolvingCreditFees;
  /**
   * Payment frequency for interest and fees.
   */
  frequency: Tenor;
  /**
   * Unique identifier for the facility.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Date when the facility becomes available.
   */
  issue_date: DateWire;
  /**
   * Letter-of-credit sub-facility. Outstanding letters of credit reduce
   * availability and the commitment-fee base, count as usage for fee
   * tiers, accrue the LC fee (the floating margin unless `fee_bp` is set)
   * plus the fronting fee, and are contingent exposure at default through
   * their own `leq`. `None` (the default) means no LC sublimit.
   */
  lc?: LetterOfCreditSpec | null;
  /**
   * Loan-equivalent exposure: the fraction of the undrawn commitment assumed
   * to be drawn at default (Basel credit conversion factor), as a decimal in
   * `[0, 1]`.
   *
   * Enters the default leg as additional exposure that the lender funds at
   * par and recovers at `recovery_rate`, so each unit of LEQ draw costs
   * `(1 − recovery_rate)` at default. Typical values are 0.3–0.75 depending
   * on rating and covenant protection. Defaults to `0.0` (no draw at
   * default), which reproduces the plain recovery leg.
   */
  leq?: number;
  /**
   * Dated margin changes, cumulative from their dates: a leverage or
   * ratings grid the analyst has forecast, a scheduled step-up or a
   * default-rate margin. `delta_bp` shifts the floating spread or the fixed
   * rate. Dates must be strictly increasing and strictly inside the
   * facility life. Empty by default.
   */
  margin_steps?: MarginStep[];
  /**
   * Date when the facility expires.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Effective-interest-rate reporting switch for the
   * `oid_eir_amortization` metric. `None` (the default) reports with fees
   * included, the same as `Some(OidEirSpec::default())`.
   */
  oid_eir?: OidEirSpec | null;
  /**
   * Business days between an accrual end and its payment date, on
   * `calendar_id`. `0` (the default) pays on the adjusted accrual end.
   */
  payment_lag_days?: number;
  /**
   * Base rate specification (fixed or floating).
   */
  rate: RateSpec;
  /**
   * Recovery rate on default (used when credit_curve_id is present).
   *
   * Represents the fraction of exposure recovered in the event of default.
   * Typical values: 0.30-0.50 for senior secured facilities.
   * Callers must provide the value explicitly; zero recovery remains valid.
   */
  recovery_rate: number;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Dated fixed fees (amendment, waiver, extension, consent), each paid on
   * its date and emitted as a generic fee flow. Dates must lie after the
   * commitment date and on or before maturity. Empty by default.
   */
  scheduled_fees?: ScheduledFee[];
  /**
   * Business days from the valuation date to the settlement date used by
   * quote metrics (discount margin, yield, accrued interest). `0` (the
   * default) settles on the valuation date. The base present value is
   * always anchored at the valuation date.
   */
  settlement_days?: number;
  /**
   * Stub rule for schedule generation when dates don't align with frequency.
   *
   * Determines how to handle partial periods at the start or end of the schedule:
   * - `ShortFront`: Short stub at the beginning (most common for RCFs)
   * - `ShortBack`: Short stub at the end
   * - `LongFront`: Long stub at the beginning
   * - `LongBack`: Long stub at the end
   * - `None`: No stub allowed (dates must align exactly)
   *
   * Defaults to `ShortFront` for maximum flexibility with unaligned dates.
   */
  stub?: StubKind;
}
/**
 * A scheduled change of a facility's commitment.
 *
 * The commitment equals `amount` from `date` forward until the next step.
 * Steps down are amortizing commitments, availability expiries and voluntary
 * reductions; steps up are accordion exercises. Utilization is always drawn
 * balance over the commitment in force, so a stochastic revolving facility
 * books the implied principal change at the step.
 *
 * A delayed-draw term loan (`DdtlSpec::commitment_steps`) accepts only
 * non-increasing steps inside its availability window and no reduction fee
 * (`reduction_fee_bp` must be zero).
 */
export interface CommitmentStep {
  /**
   * Commitment in force from `date`, in the facility currency (zero ends
   * availability, as at a term-out). The drawn balance plus outstanding
   * letters of credit must not exceed it.
   */
  amount: Money;
  /**
   * Date the new commitment takes effect (strictly after the commitment
   * date, on or before maturity).
   */
  date: DateWire;
  /**
   * One-off reduction or cancellation fee, in basis points of the reduced
   * amount (not per annum), paid by the borrower on `date` when the
   * commitment steps down. Ignored on a step up. Defaults to zero.
   */
  reduction_fee_bp?: DecimalWire;
}
/**
 * A single draw or repayment event.
 */
export interface DrawRepayEvent {
  /**
   * Amount being drawn or repaid (absolute value).
   */
  amount: Money;
  /**
   * Date of the draw or repayment.
   */
  date: DateWire;
  /**
   * True if this is a draw, false if it's a repayment.
   */
  is_draw: boolean;
}
/**
 * Specification for stochastic utilization modeling.
 *
 * Defines the stochastic process for Monte Carlo pricing with uncertain
 * draw/repayment patterns. Credit risk is incorporated via hazard-rate
 * survival weighting (no explicit default events). The estimator count,
 * antithetic flag and seed label are pricing settings, read from
 * `instrument_pricing_overrides.model_config` (`mc_paths`, `mc_antithetic`,
 * `mc_seed_scenario`); see [`RevolvingCreditMcRun::resolve`].
 */
export interface StochasticUtilizationSpec {
  /**
   * Advanced Monte Carlo configuration (optional).
   *
   * When present, enables multi-factor modeling with credit spread
   * and interest rate dynamics, correlation, and default modeling.
   */
  mc_config?: McConfig | null;
  /**
   * Use Sobol quasi-Monte Carlo RNG instead of Philox (default: false).
   * Mutually exclusive with `model_config.mc_antithetic = true`; validation
   * rejects the combination.
   */
  use_sobol_qmc?: boolean;
  /**
   * Utilization process specification.
   */
  utilization_process: UtilizationProcess;
}
/**
 * Advanced Monte Carlo configuration for revolving credit facilities.
 *
 * Enables multi-factor modeling with credit risk, interest rate dynamics,
 * correlation between factors, and default modeling.
 */
export interface McConfig {
  /**
   * Correlation matrix (3x3) between [utilization, rate, credit].
   *
   * Must be symmetric, positive definite, with ones on diagonal.
   * If None, factors are assumed independent.
   *
   * @minItems 3
   * @maxItems 3
   */
  correlation_matrix?: [[number, number, number], [number, number, number], [number, number, number]] | null;
  /**
   * Credit spread process specification. Recovery on default is the
   * facility's `recovery_rate`; the hazard-to-spread mapping reads it there.
   */
  credit_spread_process: CreditSpreadProcessSpec;
  /**
   * Interest rate process specification (for floating rates).
   *
   * If None, assumes fixed rate (no stochastic dynamics).
   */
  interest_rate_process?: InterestRateProcessSpec | null;
  /**
   * Optional utilization–credit correlation used when `correlation_matrix` is None.
   *
   * If provided, builds a 3×3 matrix with:
   *   [ [1, 0, rho], [0, 1, 0], [rho, 0, 1] ]
   * representing correlation between utilization and credit, with the rate
   * factor uncorrelated (kept fixed in 2‑factor mode).
   */
  util_credit_corr?: number | null;
}
/**
 * Interest rate process specification (for floating rates).
 */
export interface InterestRateProcessSpec {
  hull_white_1f: {
    /**
     * Hull-White 1F mean reversion speed κ, per year.
     */
    hw1f_mean_reversion: number;
    /**
     * Hull-White 1F short-rate volatility σ, absolute annual decimal
     * (`0.01` = 100 bp).
     */
    hw1f_sigma: number;
    /**
     * Initial short rate used only when `hw1f_sigma == 0`. For a stochastic
     * process, the pricer derives the initial rate from the facility's
     * discount curve at the valuation anchor.
     */
    initial: number;
    /**
     * Constant mean reversion level used only when `hw1f_sigma == 0`. For a
     * stochastic process, the pricer fits a time-dependent θ(t) to the
     * facility's discount curve.
     */
    theta: number;
    [k: string]: unknown;
  };
}
/**
 * Utilization process for stochastic draws/repayments.
 *
 * For the 80/20 implementation, we support a single mean-reverting process.
 * This can be extended in the future to support other processes (jump-diffusion,
 * regime-switching, etc.).
 */
export interface UtilizationProcess {
  mean_reverting: {
    /**
     * Mean-reversion speed κ, per year.
     */
    kappa: number;
    /**
     * Utilization volatility σ, annualized (absolute utilization units).
     */
    sigma: number;
    /**
     * Sensitivity of the utilization target to the simulated credit
     * spread (adverse selection), as a decimal per unit of relative
     * spread change.
     *
     * The target used by the OU step becomes
     * `θ(t) = clamp(theta + spread_sensitivity · (s(t) / s(0) − 1), 0, 1)`
     * where `s(t)` is the simulated spread and `s(0)` its initial level,
     * so a spread that doubles raises the target by `spread_sensitivity`.
     * Defaults to `0.0` (no link); the utilization/credit shock
     * correlation in `McConfig` applies on top of it. Ignored when the
     * facility has no credit-spread process.
     */
    spread_sensitivity?: number;
    /**
     * Long-run utilization level θ, as a fraction in `[0, 1]`.
     */
    theta: number;
  };
}
/**
 * Fee structure for a revolving credit facility.
 *
 * Contains the various fees charged on the facility:
 * - Upfront: one-time fee at commitment
 * - Commitment: annual fee on undrawn amount (can be tiered by utilization)
 * - Usage: annual fee on drawn amount (can be tiered by utilization)
 * - Facility: annual fee on total commitment
 *
 * Flat fees can be represented as single-tier vectors.
 */
export interface RevolvingCreditFees {
  /**
   * Commitment fee tiers (utilization-based). Empty vector means no commitment fee.
   * Tiers should be sorted by threshold ascending.
   */
  commitment_fee_tiers?: FeeTier[];
  /**
   * Annual facility fee rate on total commitment (basis points).
   * Facility fee is not tiered (applies to total commitment regardless of utilization).
   */
  facility_fee_bp: number;
  /**
   * Dated fee changes, cumulative from their dates, each shifting every
   * tier of the corresponding fee in basis points per annum. Dates must be
   * strictly increasing and strictly inside the facility life. Empty by
   * default.
   */
  steps?: FeeStep[];
  /**
   * One-time upfront (arrangement or OID) fee paid by the borrower to the
   * lender on the commitment date, as an absolute amount or a fraction of
   * the opening commitment. Enters the present value only while the
   * commitment date lies after the valuation date, and the effective-rate
   * metrics always.
   */
  upfront_fee?: UpfrontFee | null;
  /**
   * Usage fee tiers (utilization-based). Empty vector means no usage fee.
   * Tiers should be sorted by threshold ascending.
   */
  usage_fee_tiers?: FeeTier[];
}
/**
 * Fee tier for utilization-based fee structures.
 *
 * Tiers are evaluated in order: the first tier where utilization >= threshold applies.
 * Tiers must be sorted by threshold (ascending); [`evaluate_fee_tiers`]
 * rejects unordered tiers.
 */
export interface FeeTier {
  /**
   * Fee rate in basis points for this tier.
   */
  bp: DecimalWire;
  /**
   * Utilization threshold (0.0 to 1.0). Fee applies when utilization >= this threshold.
   */
  threshold: DecimalWire;
}
/**
 * A dated change of a revolving facility's running fees.
 *
 * Each delta shifts every tier of the corresponding fee, in basis points per
 * annum, from `date` forward. A leverage or ratings grid the analyst has
 * forecast is entered as one step per grid change.
 */
export interface FeeStep {
  /**
   * Change to the commitment fee on the undrawn commitment, in basis
   * points per annum. Defaults to `0.0`.
   */
  commitment_delta_bp?: number;
  /**
   * Date the deltas take effect (strictly inside the facility life).
   */
  date: DateWire;
  /**
   * Change to the facility fee on the total commitment, in basis points
   * per annum. Defaults to `0.0`.
   */
  facility_delta_bp?: number;
  /**
   * Change to the usage fee on the drawn balance, in basis points per
   * annum. Defaults to `0.0`.
   */
  usage_delta_bp?: number;
}
/**
 * Letter-of-credit sub-facility of a revolving credit facility.
 *
 * Outstanding letters of credit reduce availability and the commitment-fee
 * base, count as usage for fee tiers, accrue an LC fee plus a fronting fee,
 * and are contingent exposure at default. LC usage is deterministic in both
 * pricing modes; a stochastic utilization process is capped at
 * `1 − LC(t) / C(t)`.
 */
export interface LetterOfCreditSpec {
  /**
   * Future issuances and expiries, replayed on top of `outstanding`.
   */
  events?: LcEvent[];
  /**
   * LC fee on the outstanding face, in basis points per annum. `None`
   * accrues the facility's floating margin (the market convention for
   * standby letters of credit); a fixed-rate facility must supply it.
   */
  fee_bp?: number | null;
  /**
   * Fronting fee paid to the issuing bank on the outstanding face, in
   * basis points per annum. Defaults to `0.0`.
   */
  fronting_fee_bp?: number;
  /**
   * Fraction of the outstanding face assumed drawn at default (credit
   * conversion factor), as a decimal in `[0, 1]`; funded at par and
   * recovered at the facility recovery rate. Defaults to `0.0`.
   */
  leq?: number;
  /**
   * LC face outstanding at the simulation anchor, in the facility currency.
   */
  outstanding: Money;
  /**
   * Maximum LC face outstanding at any time, in the facility currency.
   */
  sublimit: Money;
}
/**
 * Issuance or expiry of a letter of credit under a facility's LC sublimit.
 */
export interface LcEvent {
  /**
   * Face amount issued or expiring, in the facility currency (positive).
   */
  amount: Money;
  /**
   * Date the letter of credit is issued or expires (strictly after the
   * simulation anchor, on or before maturity).
   */
  date: DateWire;
  /**
   * `true` for an issuance (LC outstanding rises), `false` for an expiry
   * or cancellation.
   */
  is_issue: boolean;
}
/**
 * Dated margin step (covenant penalty, scheduled change or pricing-grid move).
 *
 * Shifts the interest margin by `delta_bp` from `date` onward (effective-from
 * date). Steps are cumulative. A negative `delta_bp` steps the margin down (a
 * leverage-grid improvement); term loans accept only non-negative steps.
 */
export interface MarginStep {
  /**
   * Effective-from date of the margin change.
   */
  date: DateWire;
  /**
   * Change in margin, in basis points (`100` = 1%); negative steps down.
   */
  delta_bp: DecimalWire;
}
/**
 * Optional configuration for effective interest rate (EIR) amortization schedules.
 *
 * When enabled, EIR amortization schedules are computed for reporting using
 * the loan's full cashflow schedule (including OID effects).
 */
export interface OidEirSpec {
  /**
   * Include fee cashflows (upfront, commitment, usage) in the EIR schedule.
   *
   * Defaults to true because these fees are typically part of the effective yield.
   */
  include_fees?: boolean;
}
/**
 * A dated fixed fee under a facility: amendment, waiver, extension or
 * consent fees the borrower pays on a known date.
 */
export interface ScheduledFee {
  /**
   * Fee amount in the facility currency (non-negative).
   */
  amount: Money;
  /**
   * Payment date (strictly after the commitment date, on or before
   * maturity).
   */
  date: DateWire;
}
/**
 * Term loan instrument with covenant and DDTL support.
 *
 * Represents a fully-validated institutional term loan with support for:
 * - Fixed or floating interest rates
 * - Delayed-draw term loan (DDTL) features
 * - Payment-in-kind (PIK) interest
 * - Flexible amortization schedules
 * - Covenant-driven events (margin step-ups, cash sweeps, PIK toggles)
 * - Original issue discount (OID) handling
 * - Borrower call schedules
 *
 * # Construction
 *
 * Build with [`TermLoan::builder()`]; `build()` validates the complete
 * contract (dates, currencies, DDTL draws against the commitment in force,
 * covenant and call schedules):
 *
 * ```
 * use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
 *
 * let loan = TermLoan::example()?;
 * loan.validate()?;
 * # Ok::<(), finstack_quant_core::Error>(())
 * ```
 *
 * # Cashflow Generation
 *
 * Uses the [`CashflowProvider`](crate::cashflow::traits::CashflowProvider) trait:
 * - `dated_cashflows()` returns signed canonical schedule flows (coupons, amortization, redemptions)
 * - `cashflow_schedule()` returns the signed canonical schedule with `CFKind` metadata
 *
 * # Pricing
 *
 * Implements [`Instrument::value()`](crate::instruments::common_impl::traits::Instrument::value)
 * using deterministic cashflow discounting. PIK interest is capitalized and excluded from PV.
 *
 * # Invariants
 *
 * - `issue < maturity`
 * - `notional_limit.currency() == currency`
 * - All monetary amounts are in the same currency
 * - Amortization does not exceed outstanding principal
 *
 * # Thread Safety
 *
 * This type is `Send + Sync` as all fields are thread-safe.
 */
export interface TermLoan {
  /**
   * Scheduled principal amortization (the shared cashflows
   * `AmortizationSpec`). `LinearTo` and `StepRemaining` are rejected;
   * `PercentOfOriginalPerPeriod` and `LinearBetween` apply per funded draw.
   */
  amortization: AmortizationSpec;
  /**
   * Attributes for tagging and scenarios
   */
  attributes: Attributes;
  /**
   * Business day convention
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional calendar id for adjustments
   */
  calendar_id?: Id | null;
  /**
   * Optional call schedule (borrower callability)
   */
  call_schedule?: LoanCallSchedule | null;
  /**
   * Coupon split type (Cash/PIK/Split)
   */
  coupon_type?: CouponType;
  /**
   * Optional covenant spec
   */
  covenants?: TermLoanCovenantEvents | null;
  /**
   * Optional credit curve identifier (defaults to discount_curve_id if None)
   */
  credit_curve_id?: Id | null;
  /**
   * Currency for all cashflows
   */
  currency: Currency;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Optional DDTL parameters; None => plain term loan
   */
  ddtl?: DdtlSpec | null;
  /**
   * Discount curve identifier
   */
  discount_curve_id: Id;
  /**
   * Payment frequency for coupons/fees
   */
  frequency: Tenor;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Issue (effective) date
   */
  issue_date: DateWire;
  /**
   * Maturity date
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Maximum commitment / notional limit
   */
  notional_limit: Money;
  /**
   * Optional EIR amortization settings for reporting schedules
   */
  oid_eir?: OidEirSpec | null;
  /**
   * Rate specification (fixed or floating)
   */
  rate: RateSpec;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Settlement days (T+n) used as the valuation/accrued anchor. Default is 2.
   *
   * Note: the LSTA target settlement for par/near-par secondary loan trades
   * is T+7 (with delayed compensation beyond T+7, and T+20 for distressed).
   * The T+2 default here is a pricing-anchor choice, not an LSTA
   * convention; set `settlement_days: 7` to anchor at the LSTA par-trade
   * target.
   */
  settlement_days?: number;
  /**
   * Stub rule
   */
  stub?: StubKind;
  /**
   * Upfront (arrangement or OID) fee paid on `issue_date`, as an amount or
   * a fraction of the commitment (the DDTL `commitment`, else
   * `notional_limit`).
   */
  upfront_fee?: UpfrontFee | null;
}
/**
 * Complete call schedule for callable term loans.
 *
 * Aggregates all borrower call provisions, typically with step-down
 * premiums as the loan ages (e.g., 103% in year 1, 102% in year 2, par thereafter).
 *
 * # Examples
 *
 * ```text
 * use finstack_quant_valuations::instruments::fixed_income::term_loan::spec::{LoanCallSchedule, LoanCall};
 * use finstack_quant_core::dates::create_date;
 * use time::Month;
 *
 * # fn example() -> Result<(), Box<dyn std::error::Error>> {
 * let schedule = LoanCallSchedule {
 *     calls: vec![
 *         LoanCall {
 *             date: create_date(2027, Month::January, 15)?,
 *             price_pct_of_par: 103.0,  // 3% premium in year 2
 *         },
 *         LoanCall {
 *             date: create_date(2028, Month::January, 15)?,
 *             price_pct_of_par: 101.5,  // 1.5% premium in year 3
 *         },
 *         LoanCall {
 *             date: create_date(2029, Month::January, 15)?,
 *             price_pct_of_par: 100.0,  // At par thereafter
 *         },
 *     ],
 * };
 * # Ok(())
 * # }
 * ```
 */
export interface LoanCallSchedule {
  /**
   * Ordered call provisions (typically sorted by date with descending premiums)
   */
  calls: LoanCall[];
}
/**
 * Borrower call option on term loan.
 *
 * Represents the borrower's right to prepay the loan at a specified
 * redemption price (typically at premium to par for early calls,
 * approaching par near maturity).
 *
 * # Call Types
 *
 * The `call_type` field determines how the call is exercised:
 * - `Hard`: Standard call at `price_pct_of_par` on or after `date`
 * - `Soft`: Premium call during protection period
 * - `MakeWhole`: PV-based redemption at the reference curve plus spread
 *
 * For `MakeWhole` calls, `price_pct_of_par` serves as the minimum
 * (floor) redemption price. The actual price is the greater of
 * `price_pct_of_par` and the make-whole amount.
 */
export interface LoanCall {
  /**
   * Type of call provision. Defaults to `Hard` when unspecified.
   */
  call_type?: LoanCallType;
  /**
   * Call date (earliest prepayment date for this call provision)
   */
  date: DateWire;
  /**
   * Redemption price as percentage of par (e.g., 102.0 = 102% of par).
   * For make-whole calls, this is the minimum (floor) price.
   */
  price_pct_of_par: number;
}
/**
 * Covenant-driven events for term loans.
 *
 * Aggregates all covenant-triggered or scheduled events that modify
 * loan terms, including margin increases, PIK toggles, cash sweeps,
 * and draw restrictions.
 */
export interface TermLoanCovenantEvents {
  /**
   * Cash sweep (mandatory prepayment) schedule
   */
  cash_sweeps: CashSweepEvent[];
  /**
   * Dates on which draws are prohibited (covenant breach or scheduled)
   */
  draw_stop_dates: DateWire[];
  /**
   * Margin steps, each a non-negative cumulative bp change effective from
   * the start of the first interest period on or after its date.
   */
  margin_steps: MarginStep[];
  /**
   * PIK toggle schedule
   */
  pik_toggles: PikToggle[];
}
/**
 * Cash sweep event (mandatory prepayment from excess cash flow).
 *
 * Represents scheduled or covenant-triggered prepayment from borrower's
 * excess cash flow, reducing outstanding principal.
 */
export interface CashSweepEvent {
  /**
   * Amount of mandatory prepayment
   */
  amount: Money;
  /**
   * Date of cash sweep prepayment
   */
  date: DateWire;
}
/**
 * Payment-in-kind (PIK) toggle event.
 *
 * Enables or disables PIK interest at a specified date. When enabled,
 * a portion of interest may be capitalized rather than paid in cash.
 */
export interface PikToggle {
  /**
   * Date PIK feature is toggled
   */
  date: DateWire;
  /**
   * True to enable PIK, false to disable
   */
  enable_pik: boolean;
}
/**
 * Delayed-draw term loan (DDTL) specification.
 *
 * Models a term loan with commitment period during which borrower may draw
 * down funds, subject to availability dates, step-downs, and fees.
 *
 * # Industry Practice
 *
 * DDTLs are common in:
 * - **Construction financing**: Funds released as construction milestones are met
 * - **Acquisition financing**: Delayed funding for earn-outs or contingent payments
 * - **Working capital facilities**: Drawn as needed within commitment period
 *
 * Typical features:
 * - Commitment period: 6-24 months
 * - Commitment fees: 25-50 bp on undrawn amounts
 * - Usage fees: 0-25 bp on drawn amounts
 * - Step-downs: Commitment reduces at milestones (e.g., construction completion)
 *
 * # Fee Conventions
 *
 * - **Commitment fee**: Paid on undrawn commitment (compensates lender for availability)
 * - **Usage fee**: Paid on drawn amounts (additive to interest margin)
 * - **OID**: May be withheld at each draw or tracked separately
 *
 * # Examples
 *
 * ```text
 * use finstack_quant_valuations::instruments::fixed_income::term_loan::spec::*;
 * use finstack_quant_core::money::Money;
 * use finstack_quant_core::currency::Currency;
 * use finstack_quant_core::dates::create_date;
 * use time::Month;
 *
 * # fn example() -> Result<(), Box<dyn std::error::Error>> {
 * let ddtl = DdtlSpec {
 *     commitment: Money::from((10_000_000_i64, Currency::USD)),
 *     availability_start: create_date(2025, Month::January, 1)?,
 *     availability_end: create_date(2026, Month::January, 1)?,
 *     draws: vec![],
 *     commitment_steps: vec![],
 *     usage_fee_bp: dec!(50),        // 50 bp usage fee
 *     commitment_fee_bp: dec!(25),   // 25 bp commitment fee
 *     fee_base: CommitmentFeeBase::Undrawn,
 *     oid_policy: None,
 * };
 * # Ok(())
 * # }
 * ```
 */
export interface DdtlSpec {
  /**
   * Last date draws are permitted (commitment expiry)
   */
  availability_end: DateWire;
  /**
   * First date draws are permitted
   */
  availability_start: DateWire;
  /**
   * Total commitment available for draws, in the loan currency.
   */
  commitment: Money;
  /**
   * Commitment fee on the undrawn commitment, in basis points per annum
   * (non-negative; `50` = 0.50%).
   */
  commitment_fee_bp: DecimalWire;
  /**
   * Commitment steps, each effective from its date: strictly increasing
   * dates inside the availability window, non-increasing `amount`s in the
   * loan currency, and a zero `reduction_fee_bp` (term loans carry no
   * reduction fee).
   */
  commitment_steps: CommitmentStep[];
  /**
   * Scheduled or actual draw events
   */
  draws: DrawEvent[];
  /**
   * Basis for commitment fee calculation
   */
  fee_base: CommitmentFeeBase;
  /**
   * Original issue discount policy, if applicable
   */
  oid_policy?: OidPolicy | null;
  /**
   * Usage fee on drawn amounts, in basis points per annum (non-negative;
   * `25` = 0.25%).
   */
  usage_fee_bp: DecimalWire;
}
/**
 * A scheduled draw `{date, amount}` on a committed facility.
 *
 * A delayed-draw term loan funds on `date` (inside its availability
 * window); an asset-backed facility funds on the first payment date on or
 * after `date`.
 */
export interface DrawEvent {
  /**
   * Amount drawn from the available commitment, in the facility currency
   * (positive).
   */
  amount: Money;
  /**
   * Date of the draw.
   */
  date: DateWire;
}
/**
 * Interest rate swap with fixed and floating legs.
 *
 * Represents a standard interest rate swap where one party pays
 * a fixed rate and the other pays a floating rate plus spread.
 *
 * # Market Standards & Citations
 *
 * ## ISDA Definitions
 *
 * Current RFR and term-rate contracts are represented under the **ISDA 2021
 * Interest Rate Derivatives Definitions**. Legacy transactions may retain
 * terms from the 2006 Definitions, including historical IBOR reset conventions.
 * Fixed/floating day counts, calendars, lags, compounding, and payment rules
 * are explicit leg terms or are resolved from the rate-index convention registry.
 *
 * ## USD Market Convention
 *
 * The canonical legacy USD term-index example uses:
 * - **Fixed Leg:** Semi-annual, 30/360, Modified Following
 * - **Floating Leg:** Quarterly, ACT/360, Modified Following
 * - **Reset Lag:** T-2 (2 business days before period start)
 * - **Discounting:** OIS curve under the collateral agreement
 *
 * ## Day-Count Convention Notes
 *
 * The USD standard uses different day-count conventions for different purposes:
 * - **Fixed leg accrual:** 30/360 (Bond Basis)
 * - **Floating leg accrual:** ACT/360 (Money Market)
 * - **Discount curve:** Typically ACT/365F or ACT/360 depending on construction
 *
 * This day-count mismatch between accrual and discounting is market-standard
 * and reflects the different conventions used in bond vs money markets.
 * The impact on par rates is typically < 0.5bp for USD swaps.
 *
 * ## Validation
 *
 * Use [`InterestRateSwap::validate()`] to check swaps constructed via
 * the builder pattern.
 *
 * ## References
 *
 * - ISDA 2021 Interest Rate Derivatives Definitions (current contract framework) `docs/REFERENCES.md#isda-2021-definitions`
 * - ISDA 2006 Definitions (legacy transactions) `docs/REFERENCES.md#isda-2006-definitions`
 * - Sadr, A. *Interest Rate Swaps and Their Derivatives*.
 *   `docs/REFERENCES.md#sadr-2009-irs`
 * - Bloomberg SWPM screen conventions.
 *   `docs/REFERENCES.md#bloomberg-swpm`
 */
export interface InterestRateSwap {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Fixed leg specification.
   */
  fixed_leg: FixedLegSpec;
  /**
   * Floating leg specification.
   */
  float_leg: FloatLegSpec;
  /**
   * Unique identifier for the swap.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Optional OTC margin specification for VM/IM.
   *
   * When present, enables margin calculation using SIMM or schedule-based
   * methodologies. For cleared swaps, specify clearing house in
   * `OtcMarginSpec::cleared()`.
   */
  margin_spec?: OtcMarginSpec | null;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount for both legs.
   */
  notional: Money;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Direction of the swap (Pay or Receive).
   */
  side: PayReceive;
}
/**
 * Specification for fixed rate legs in interest rate swaps
 */
export interface FixedLegSpec {
  /**
   * Business day convention for payment dates
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional calendar for business day adjustments
   */
  calendar_id?: Id | null;
  /**
   * Day count convention for accrual
   */
  day_count: DayCount;
  /**
   * Discount curve identifier for pricing
   */
  discount_curve_id: Id;
  /**
   * End date of the fixed leg
   */
  end: DateWire;
  /**
   * End-of-month roll convention (default: false).
   *
   * When `true`, if the start date falls on the last business day of a month,
   * all subsequent roll dates will also fall on the last business day of their
   * respective months. This matches QuantLib's `MakeOIS` default behavior.
   *
   * # Market Standard
   *
   * Per ISDA 2006 Definitions Section 4.18, the End-of-Month convention should
   * be applied when the effective date is the last business day of a month.
   * Most professional systems (QuantLib, Bloomberg SWDF) default to `true`.
   */
  end_of_month?: boolean;
  /**
   * Payment frequency
   */
  frequency: Tenor;
  /**
   * Optional par-rate calculation method override
   */
  par_method?: ParRateMethod | null;
  /**
   * Payment lag in business days after period end (default: 0).
   *
   * Bloomberg OIS swaps typically use 2 business days payment lag.
   * The actual payment date is adjusted from the period end date by
   * this many business days using the leg's calendar.
   */
  payment_lag_days?: number;
  /**
   * Fixed rate (e.g., 0.05 for 5%)
   */
  rate: DecimalWire;
  /**
   * Start date of the fixed leg
   */
  start: DateWire;
  /**
   * Stub period handling rule
   */
  stub?: StubKind;
}
/**
 * Specification for floating rate legs in interest rate swaps
 */
export interface FloatLegSpec {
  /**
   * Business day convention for payment dates
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional calendar for business day adjustments
   */
  calendar_id?: Id | null;
  /**
   * Compounding method for floating coupons.
   *
   * Determines how floating rate coupons are calculated:
   * - `simple` (default): one term forward per period
   * - `compounded_*`: SOFR/SONIA-style daily compounding
   * - `simple_average` is rejected by [`Self::validate`]
   *
   * # Implementation Notes
   *
   * Compounded-in-arrears is implemented for IRS pricing in `instruments::irs` with
   * support for lookback and observation shift conventions. For seasoned (already
   * started) compounded swaps, pricing requires explicit fixings for observation
   * dates prior to `as_of`.
   */
  compounding?: FloatingLegCompounding;
  /**
   * Day count convention for accrual
   */
  day_count: DayCount;
  /**
   * Discount curve identifier for pricing
   */
  discount_curve_id: Id;
  /**
   * End date of the floating leg
   */
  end: DateWire;
  /**
   * End-of-month roll convention (default: false).
   *
   * When `true`, if the start date falls on the last business day of a month,
   * all subsequent roll dates will also fall on the last business day of their
   * respective months. This matches QuantLib's `MakeOIS` default behavior.
   *
   * # Market Standard
   *
   * Per ISDA 2006 Definitions Section 4.18, the End-of-Month convention should
   * be applied when the effective date is the last business day of a month.
   * Most professional systems (QuantLib, Bloomberg SWDF) default to `true`.
   */
  end_of_month?: boolean;
  /**
   * Optional calendar for rate fixing (reset lag)
   */
  fixing_calendar_id?: Id | null;
  /**
   * Forward curve identifier for rate projections
   */
  forward_curve_id: Id;
  /**
   * Payment frequency
   */
  frequency: Tenor;
  /**
   * Payment lag in business days after period end (default: 0).
   *
   * Bloomberg OIS swaps typically use 2 business days payment lag.
   * The actual payment date is adjusted from the period end date by
   * this many business days using the leg's calendar.
   */
  payment_lag_days?: number;
  /**
   * Reset lag in business days for floating rate fixing (default: 0).
   *
   * - **0** (default): fixing on the accrual start date. A swap whose first
   *   accrual period starts on or after the valuation date then prices off
   *   the forward curve without historical fixings.
   * - **Positive** (e.g., 2): T-2 fixing (2 business days before accrual
   *   start). Use `RateIndexConventions::default_reset_lag_days` (or
   *   `InterestRateSwap::from_conventions`) for the market default of a
   *   given index.
   * - **Negative** (e.g., -1): resolved at pricing time to the registered
   *   index convention default; rejected when the forward curve id is not a
   *   registered rate index.
   */
  reset_lag_days?: number;
  /**
   * Spread in basis points added to the forward rate
   */
  spread_bp: DecimalWire;
  /**
   * Start date of the floating leg
   */
  start: DateWire;
  /**
   * Stub period handling rule
   */
  stub?: StubKind;
}
/**
 * OTC derivative margin specification (ISDA CSA compliant).
 *
 * This is the standard margin specification for bilateral and cleared
 * OTC derivatives. It combines CSA terms with clearing-specific parameters.
 *
 * # Usage
 *
 * Attach this to any OTC derivative instrument that requires margining:
 * - Interest Rate Swaps (IRS)
 * - Credit Default Swaps (CDS)
 * - CDS Indices
 * - Total Return Swaps (TRS)
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{
 *     OtcMarginSpec, CsaSpec, SimmCreditClassification, SimmCreditSector,
 * };
 *
 * # fn main() -> finstack_quant_core::Result<()> {
 * // Bilateral (uncleared) derivative
 * let bilateral_spec = OtcMarginSpec::bilateral_simm(CsaSpec::usd_regulatory()?);
 * let credit_spec = bilateral_spec.with_simm_credit_classification(
 *     SimmCreditClassification::Qualifying {
 *         sector: SimmCreditSector::Financial,
 *     },
 * );
 *
 * // Cleared derivative
 * let cleared_spec = OtcMarginSpec::cleared("LCH", finstack_quant_core::currency::Currency::USD)?;
 * # Ok(())
 * # }
 * ```
 */
export interface OtcMarginSpec {
  /**
   * Clearing status: bilateral or cleared through CCP
   */
  clearing_status: ClearingStatus;
  /**
   * Full CSA specification (for bilateral trades)
   *
   * For cleared trades, this represents the terms with the CCP.
   */
  csa: CsaSpec;
  /**
   * Initial margin calculation methodology
   *
   * - Bilateral: SIMM or Schedule
   * - Cleared: ClearingHouse (CCP-specific)
   */
  im_methodology: ImMethodology;
  /**
   * Settlement lag for margin transfers (business days)
   */
  settlement_lag: number;
  /**
   * Explicit SIMM credit classification for credit-sensitive instruments.
   *
   * Required when a credit product uses `ImMethodology::Simm`; leave `None`
   * for non-credit instruments and non-SIMM margin methodologies.
   */
  simm_credit_classification?: SimmCreditClassification | null;
  /**
   * Variation margin exchange frequency
   */
  vm_frequency: MarginTenor;
}
/**
 * Credit Support Annex specification (ISDA standard).
 *
 * The CSA governs the exchange of collateral between counterparties
 * for OTC derivatives. This specification captures all key commercial
 * terms needed for margin calculation and management.
 *
 * # ISDA Documentation
 *
 * This type represents terms from:
 * - ISDA 2016 Credit Support Annex for Variation Margin (VM CSA)
 * - ISDA 2018 Credit Support Annex for Initial Margin (IM CSA)
 *
 * # References
 *
 * - ISDA 2016 VM CSA: `docs/REFERENCES.md#isda-vm-csa-2016`
 * - ISDA 2018 IM CSA: `docs/REFERENCES.md#isda-im-csa-2018`
 * - BCBS-IOSCO uncleared margin framework: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{
 *     CsaSpec, VmParameters, ImParameters, EligibleCollateralSchedule,
 *     MarginCallTiming, ImMethodology, MarginTenor,
 * };
 * use finstack_quant_core::currency::Currency;
 * use finstack_quant_core::money::Money;
 *
 * # fn main() -> finstack_quant_core::Result<()> {
 * let csa = CsaSpec {
 *     id: "USD-CSA-2024".to_string(),
 *     base_currency: Currency::USD,
 *     vm_params: VmParameters::regulatory_standard(Currency::USD)?,
 *     im_params: Some(ImParameters::simm_standard(Currency::USD)?),
 *     eligible_collateral: EligibleCollateralSchedule::bcbs_standard()?,
 *     call_timing: MarginCallTiming::regulatory_standard()?,
 *     collateral_curve_id: "USD-OIS".into(),
 *     calendar_id: "usny".into(),
 * };
 * # Ok(())
 * # }
 * ```
 */
export interface CsaSpec {
  /**
   * Base currency for margin calculations.
   *
   * All exposures and collateral values are converted to this currency
   * for netting and comparison with thresholds.
   */
  base_currency: Currency;
  /**
   * Contractual business-day calendar for calls and settlements.
   */
  calendar_id: string;
  /**
   * Margin call timing parameters.
   */
  call_timing: MarginCallTiming;
  /**
   * Discount curve ID for collateral valuation.
   *
   * Cash collateral is typically discounted at OIS/RFR rates.
   * This curve should match the CSA's collateral interest rate.
   */
  collateral_curve_id: Id;
  /**
   * Eligible collateral schedule.
   *
   * Defines what collateral types are acceptable and associated haircuts.
   */
  eligible_collateral: EligibleCollateralSchedule;
  /**
   * CSA identifier (e.g., "USD-CSA-STANDARD", "COUNTERPARTY-XYZ-CSA")
   */
  id: string;
  /**
   * Initial margin parameters (optional).
   *
   * If None, no IM is exchanged (either not in scope for regulations
   * or trade is cleared).
   */
  im_params?: ImParameters | null;
  /**
   * Variation margin parameters.
   *
   * Governs daily mark-to-market collateral exchange.
   */
  vm_params: VmParameters;
}
/**
 * Margin call timing parameters.
 *
 * Specifies the operational timing for margin calls including
 * notification and dispute resolution windows.
 */
export interface MarginCallTiming {
  /**
   * Grace period for collateral delivery (business days)
   */
  delivery_grace_days: number;
  /**
   * Dispute resolution window (business days)
   */
  dispute_resolution_days: number;
  /**
   * Notification deadline (hours after valuation, e.g., 13:00 local time)
   */
  notification_deadline_hours: number;
  /**
   * Response deadline (hours after notification)
   */
  response_deadline_hours: number;
}
/**
 * Eligible collateral schedule with haircuts.
 *
 * Defines the complete set of collateral types accepted under a CSA
 * or margin agreement, along with associated haircuts and constraints.
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{CollateralEligibility, EligibleCollateralSchedule};
 *
 * // Start from a standard schedule (BCBS-IOSCO compliant)
 * let schedule = EligibleCollateralSchedule::bcbs_standard()?;
 * # let _ = schedule;
 * # Ok::<(), finstack_quant_core::Error>(())
 * ```
 */
export interface EligibleCollateralSchedule {
  /**
   * Default haircut for unlisted collateral (if accepted)
   *
   * If None, only explicitly listed collateral types are accepted.
   */
  default_haircut?: number | null;
  /**
   * List of eligible collateral types with haircuts
   */
  eligible: CollateralEligibility[];
  /**
   * Whether rehypothecation of posted collateral is permitted
   *
   * For IM under BCBS-IOSCO rules, rehypothecation is prohibited.
   * For VM, rehypothecation may be permitted by bilateral agreement.
   */
  rehypothecation_allowed?: boolean;
}
/**
 * Single collateral eligibility entry.
 *
 * Defines eligibility criteria and haircut for a specific type of collateral.
 */
export interface CollateralEligibility {
  /**
   * Asset class
   */
  asset_class: CollateralAssetClass;
  /**
   * Concentration limit as fraction of total collateral (optional)
   *
   * E.g., 0.30 means max 30% of collateral can be this type.
   */
  concentration_limit?: number | null;
  /**
   * Additional FX haircut for currency mismatch (decimal)
   *
   * Applied when collateral currency differs from settlement currency.
   */
  fx_haircut_addon?: number;
  /**
   * Haircut as decimal (e.g., 0.02 = 2%)
   */
  haircut: number;
  /**
   * Remaining maturity constraints
   */
  maturity_constraints?: MaturityConstraints | null;
  /**
   * Minimum credit rating requirement (e.g., "A-", "BBB")
   *
   * If None, no rating constraint applies.
   */
  min_rating?: string | null;
}
/**
 * Maturity constraints for eligible collateral.
 *
 * Some CSAs restrict collateral based on remaining maturity to limit
 * duration risk in the collateral portfolio.
 */
export interface MaturityConstraints {
  /**
   * Maximum remaining years to maturity (if any)
   */
  max_remaining_years?: number | null;
  /**
   * Minimum remaining years to maturity (if any)
   */
  min_remaining_years?: number | null;
}
/**
 * Initial margin parameters.
 *
 * Initial margin is collateral posted to cover potential future exposure (PFE)
 * during the close-out period following a default. IM is required for
 * non-centrally cleared derivatives under BCBS-IOSCO rules.
 *
 * # Margin Period of Risk (MPOR)
 *
 * The MPOR determines the horizon over which PFE is calculated:
 * - Standard: 10 business days for bilateral derivatives
 * - Reduced: 5 days for certain liquid products
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{ImMethodology, ImParameters};
 * use finstack_quant_core::currency::Currency;
 * use finstack_quant_core::money::Money;
 *
 * let im_params = ImParameters {
 *     methodology: ImMethodology::Simm,
 *     mpor_days: 10,
 *     threshold: Money::from((50_000_000_i64, Currency::USD)),
 *     mta: Money::from((0_i64, Currency::USD)), // Combined with VM MTA
 *     segregated: true,
 * };
 * ```
 */
export interface ImParameters {
  /**
   * IM calculation methodology.
   *
   * Options include SIMM, regulatory schedule, or CCP methodology.
   */
  methodology: ImMethodology;
  /**
   * Margin Period of Risk in business days.
   *
   * Standard is 10 days under BCBS-IOSCO. CCPs may use shorter periods.
   */
  mpor_days: number;
  /**
   * Minimum Transfer Amount for IM.
   *
   * Combined IM+VM MTA must not exceed €500,000 under BCBS-IOSCO.
   */
  mta: Money;
  /**
   * Whether IM must be held in a segregated account.
   *
   * Under BCBS-IOSCO, IM must be segregated with a third-party custodian
   * to protect it in case of the collecting party's insolvency.
   */
  segregated: boolean;
  /**
   * Group-level IM threshold allocated to this CSA by the caller.
   *
   * BCBS-IOSCO permits €50M aggregate threshold at group level.
   * Many large dealers operate with zero threshold by agreement.
   */
  threshold: Money;
}
/**
 * Variation margin parameters.
 *
 * These parameters govern the daily (or periodic) exchange of variation margin
 * under a CSA agreement. VM is exchanged to eliminate mark-to-market exposure.
 *
 * # ISDA CSA Standard Terms
 *
 * The 2016 VM CSA introduced standardized terms for variation margin:
 * - Zero threshold for in-scope entities
 * - Daily exchange with T+1 settlement
 * - Cash or highly liquid securities as collateral
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{MarginTenor, VmParameters};
 * use finstack_quant_core::currency::Currency;
 * use finstack_quant_core::money::Money;
 *
 * let vm_params = VmParameters {
 *     threshold: Money::from((10_000_000_i64, Currency::USD)),
 *     mta: Money::from((500_000_i64, Currency::USD)),
 *     rounding: Money::from((10_000_i64, Currency::USD)),
 *     independent_amount: Money::from((0_i64, Currency::USD)),
 *     frequency: MarginTenor::Daily,
 *     settlement_lag: 1,
 * };
 * ```
 */
export interface VmParameters {
  /**
   * Margin call frequency.
   *
   * Under BCBS-IOSCO, daily margin exchange is required.
   */
  frequency: MarginTenor;
  /**
   * Independent Amount (IA) / Additional Margin.
   *
   * Fixed collateral amount required regardless of exposure.
   * Often used for credit enhancement or as a buffer.
   */
  independent_amount: Money;
  /**
   * Minimum Transfer Amount (MTA).
   *
   * Margin calls below MTA are not made. BCBS-IOSCO permits combined
   * IM+VM MTA up to €500,000 equivalent.
   */
  mta: Money;
  /**
   * Rounding increment for margin amounts.
   *
   * Margin calls are typically rounded to the nearest multiple of this amount.
   */
  rounding: Money;
  /**
   * Settlement lag in business days (T+n).
   *
   * Standard is T+1 for VM under 2016 VM CSA.
   */
  settlement_lag: number;
  /**
   * Threshold amount below which no margin is exchanged.
   *
   * Under BCBS-IOSCO rules for covered entities, VM threshold must be zero.
   * Legacy bilateral CSAs may have non-zero thresholds.
   */
  threshold: Money;
}
/**
 * Swaption instrument
 *
 * # Exercise lifecycle boundary
 *
 * `Instrument::value` prices the option claim through expiry. At expiry it
 * returns model-free intrinsic value; after expiry it returns zero. For
 * physical settlement, trade lifecycle infrastructure must materialize the
 * delivered [`InterestRateSwap`] from `underlying_fixed_leg`,
 * `underlying_float_leg`, `notional`, and `option_type`. This instrument does
 * not retain an exercised swap position after expiry.
 */
export interface Swaption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Cash settlement annuity method (only used when settlement = Cash).
   *
   * - `CollateralizedCashPrice` (default): Actual collateral-discounted fixed-leg annuity
   * - `ParYield`: Legacy flat-yield cash annuity
   * - `IsdaParPar`: Legacy par-par annuity from the discount curve
   * - `ZeroCoupon`: Single discount to swap maturity
   */
  cash_settlement_method: CashSettlementMethod;
  /**
   * Exercise style (European, Bermudan, American). Defaults to European.
   */
  exercise_style: ExerciseStyle;
  /**
   * Option expiry date
   */
  expiry: DateWire;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount of underlying swap
   */
  notional: Money;
  /**
   * Option type (payer or receiver swaption)
   */
  option_type: OptionType;
  /**
   * Optional SABR volatility model parameters
   */
  sabr_params?: SabrParameters | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Settlement method (physical or cash)
   */
  settlement: SettlementType;
  /**
   * Complete fixed leg of the underlying swap.
   */
  underlying_fixed_leg: FixedLegSpec;
  /**
   * Complete floating leg of the underlying swap.
   */
  underlying_float_leg: FloatLegSpec;
  /**
   * Volatility model (Black or Normal)
   */
  vol_model: VolatilityModel;
  /**
   * Volatility surface ID for option pricing
   */
  vol_surface_id: Id;
}
/**
 * SABR model parameters
 */
export interface SabrParameters {
  /**
   * Initial volatility (alpha)
   */
  alpha: PositiveF64Wire;
  /**
   * CEV exponent (beta) - typically 0 to 1
   */
  beta: ClosedUnitIntervalF64Wire;
  /**
   * Volatility of volatility (nu/volvol)
   */
  nu: NonNegativeF64Wire;
  /**
   * Correlation between asset and volatility (rho)
   */
  rho: CorrelationWire;
  /**
   * Shift parameter for handling negative rates (optional)
   */
  shift?: PositiveF64Wire | null;
}
/**
 * Waterfall specification for dynamic cash flow allocation.
 *
 * Defines the priority of payments and sweep mechanics for capital structure.
 *
 * Payment priorities and optional sweep / PIK controls model common leveraged
 * finance behavior where scheduled debt service, excess cash flow sweeps, and
 * equity leakage compete for the same cash pool.
 *
 * # Limitations
 *
 * - **Payment classes.** When `payment_classes` is empty, allocation within a
 *   category is single-class pro-rata (today's behavior). When classes are
 *   set, each category walks unique ranks and allocates pro-rata inside a
 *   class before the next class sees remaining cash.
 * - **Prepayment penalties, call premiums, and original issue discount (OID)
 *   are unsupported.** Prepayments (sweep, mandatory, voluntary) are applied
 *   at par with no penalty or premium, and no OID accretion is modeled.
 */
export interface WaterfallSpec {
  /**
   * Formula or node reference for cash available to allocate in the waterfall.
   *
   * This is the **pre-waterfall** cash pool: cash before fees, interest,
   * amortization, and prepays allocated by this waterfall. Point it at a
   * standalone cash / FCF node (`cash`, `cash_available`, `free_cash_flow`).
   * Do not deduct `cs.interest_expense`, `cs.interest_expense_cash`,
   * `cs.principal_payment`, or `cs.fees` here — those are allocated by the
   * waterfall, and subtracting them from the pool double-pays debt service.
   *
   * Required. Without a cash pool the waterfall reports every scheduled fee,
   * coupon and amortization as paid in full regardless of whether the model
   * generated the cash — uses exceed sources and no shortfall can ever be
   * raised, so the structure cannot report insolvency.
   */
  available_cash_node: string;
  /**
   * Excess Cash Flow (ECF) sweep specification
   */
  ecf_sweep?: EcfSweepSpec | null;
  /**
   * Formula or node for the `MandatoryPrepayment` rung.
   *
   * Required when `MandatoryPrepayment` appears in `priority_of_payments`.
   * Sized independently of the ECF sweep and voluntary prepay buckets.
   */
  mandatory_prepay_node?: string | null;
  /**
   * Payment classes for intra-category seniority (e.g. 1L then 2L).
   *
   * Empty means one implicit class: today's single-class pro-rata. When
   * non-empty, every contractual instrument must appear in exactly one
   * class, ranks and ids must be unique, and allocation walks rank order.
   */
  payment_classes?: PaymentClassSpec[];
  /**
   * PIK toggle specification for switching between cash and PIK interest
   */
  pik_toggle?: PikToggleSpec | null;
  /**
   * Priority order of payments (default: Fees > Interest > Amortization > Sweep > Equity)
   */
  priority_of_payments?: PaymentPriority[];
  /**
   * Formula or node for the `VoluntaryPrepayment` rung.
   *
   * Required when `VoluntaryPrepayment` appears in `priority_of_payments`.
   * Sized independently of the ECF sweep and mandatory prepay buckets.
   */
  voluntary_prepay_node?: string | null;
}
/**
 * Excess Cash Flow (ECF) sweep specification.
 *
 * Defines how to calculate ECF and what percentage to sweep to pay down debt.
 *
 * # ECF Calculation
 *
 * The standard ECF formula deducts cash interest from EBITDA. Fees and
 * scheduled principal are also deducted when those payment categories rank
 * ahead of the prepayment priority:
 *
 * ```text
 * ECF = EBITDA - Taxes - CapEx - ΔWC - Cash Interest Paid
 *       - Fees Paid Ahead of Prepayment
 *       - Scheduled Principal Paid Ahead of Prepayment
 *   ```
 *
 * Set `cash_interest_node` to override the cash-interest input. If omitted,
 * contractual cash interest is deducted automatically using the period's
 * debt-service magnitude.
 *
 * # References
 *
 * - Fixed-income and leverage context: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
 */
export interface EcfSweepSpec {
  /**
   * Formula or node reference for capital expenditures (e.g., "capex")
   */
  capex_node?: string | null;
  /**
   * Formula or node reference for cash interest paid (e.g., "cs.interest_expense_cash.total").
   *
   * Per S&P LCD / standard LPA definitions, ECF should deduct cash interest paid.
   * If omitted, contractual cash interest is deducted automatically.
   */
  cash_interest_node?: string | null;
  /**
   * Formula or node reference for EBITDA (e.g., "ebitda" or "revenue - cogs - opex")
   */
  ebitda_node: string;
  /**
   * Sweep percentage (e.g., 0.5 for 50%, 0.75 for 75%)
   */
  sweep_percentage: number;
  /**
   * Target instrument ID for sweep payments (if None, applies to all term loans)
   */
  target_instrument_id?: string | null;
  /**
   * Formula or node reference for taxes (e.g., "taxes")
   */
  taxes_node?: string | null;
  /**
   * Formula or node reference for working capital change (e.g., "wc_change")
   */
  working_capital_node?: string | null;
}
/**
 * A seniority class for intra-category waterfall allocation.
 */
export interface PaymentClassSpec {
  /**
   * Class identifier (e.g. `"1L"`).
   */
  id: string;
  /**
   * Instrument ids that belong to this class. Each instrument may appear
   * in at most one class.
   */
  instrument_ids: string[];
  /**
   * Seniority rank; `0` is most senior. Ranks must be unique.
   */
  rank: number;
}
/**
 * PIK toggle specification.
 *
 * Defines conditions for switching between cash and PIK interest modes.
 *
 * # Hysteresis
 *
 * Set `min_periods_in_pik` to prevent oscillation when the liquidity metric
 * hovers near the threshold. Once PIK is triggered, it stays active for at
 * least that many periods before it can switch back.
 *
 * Thresholds use the same scalar units as the referenced `liquidity_metric`.
 */
export interface PikToggleSpec {
  /**
   * Node reference or formula for liquidity metric (e.g., "cash_balance" or "ebitda / interest_expense")
   */
  liquidity_metric: string;
  /**
   * Minimum number of periods PIK must stay active once triggered (hysteresis).
   * Prevents oscillation when the metric hovers near the threshold.
   * Default: 0 (no hysteresis, PIK can toggle every period).
   */
  min_periods_in_pik?: number;
  /**
   * Instruments that switch to PIK when the toggle triggers.
   *
   * Must be a non-empty list: instrument-level PIK capability is not
   * modeled, so `None` or an empty list is rejected by
   * [`PikToggleSpec::validate`] rather than meaning "every instrument".
   */
  target_instrument_ids?: string[] | null;
  /**
   * Threshold value: if metric < threshold, enable PIK; otherwise use cash
   */
  threshold: number;
}
/**
 * A single finding produced by a check for a specific period or node.
 */
export interface CheckFinding {
  /**
   * Identifier of the check that produced this finding.
   */
  check_id: string;
  /**
   * Materiality context, if applicable.
   */
  materiality?: Materiality | null;
  /**
   * Human-readable description of the issue.
   */
  message: string;
  /**
   * Node identifiers involved in the finding.
   */
  nodes?: NodeId[];
  /**
   * Period the finding relates to, if applicable.
   */
  period?: string | null;
  /**
   * Severity of the finding.
   */
  severity: CheckSeverity;
  [k: string]: unknown;
}
/**
 * Materiality context attached to a finding, describing its quantitative
 * significance.
 */
export interface Materiality {
  /**
   * Absolute amount of the discrepancy.
   */
  absolute: number;
  /**
   * Human-readable label for the reference (e.g. "total_assets").
   */
  reference_label: string;
  /**
   * The reference (denominator) value used when computing `relative_pct`.
   */
  reference_value: number;
  /**
   * Discrepancy as a percentage of the reference value.
   */
  relative_pct: number;
  [k: string]: unknown;
}
/**
 * Full report aggregating all [`CheckResult`]s from a check run.
 */
export interface CheckReport {
  /**
   * Individual results for each check.
   */
  results: CheckResult[];
  /**
   * Aggregate summary.
   */
  summary: CheckSummary;
  [k: string]: unknown;
}
/**
 * Outcome of a single check execution.
 */
export interface CheckResult {
  /**
   * Category this check belongs to.
   */
  category: CheckCategory;
  /**
   * Identifier of the check.
   */
  check_id: string;
  /**
   * Human-readable name of the check.
   */
  check_name: string;
  /**
   * Individual findings produced by the check.
   */
  findings: CheckFinding[];
  /**
   * Whether the check passed (no error-severity findings).
   */
  passed: boolean;
  [k: string]: unknown;
}
/**
 * Aggregate counts for a completed check run.
 */
export interface CheckSummary {
  /**
   * Total number of error-severity findings across all checks.
   */
  errors: number;
  /**
   * Number of checks that failed (at least one error-severity finding).
   */
  failed: number;
  /**
   * Total number of info-severity findings across all checks.
   */
  infos: number;
  /**
   * Number of checks that passed.
   */
  passed: number;
  /**
   * Total number of checks executed.
   */
  total_checks: number;
  /**
   * Total number of warning-severity findings across all checks.
   */
  warnings: number;
  [k: string]: unknown;
}
/**
 * Versioned financial statement model specification.
 */
export interface FinancialModelSpec {
  /**
   * Capital structure specification (optional)
   */
  capital_structure?: CapitalStructureSpec | null;
  /**
   * Unique model identifier
   */
  id: string;
  /**
   * Additional metadata
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Map of node_id → NodeSpec
   */
  nodes: {
    [k: string]: NodeSpec;
  };
  /**
   * Ordered list of periods (quarters, months, etc.).
   *
   * Evaluation follows this order end-to-end (dependency resolution and time-series
   * helpers assume a single coherent timeline).
   */
  periods: Period[];
  /**
   * Required schema version. Only version `1` is accepted.
   */
  schema_version: SchemaVersion;
}
/**
 * Specification for a single node (metric/line item) in the financial model.
 *
 * A node can be:
 * - **Value**: Explicit values only
 * - **Calculated**: Formula-derived only
 * - **Mixed**: Value OR Forecast OR Formula (precedence: Value > Forecast > Formula)
 */
export interface NodeSpec {
  /**
   * Point-in-time availability date for each explicit period value.
   *
   * When absent for a value, market-aware evaluation conservatively makes
   * that value visible on the period's exclusive end date. An explicit
   * entry permits filing/release dates later than period end, or earlier
   * availability for operational data known before the reporting period
   * closes.
   */
  availability_dates?: {
    [k: string]: DateWire;
  };
  /**
   * Forecast specification (for Mixed nodes)
   */
  forecast?: ForecastSpec | null;
  /**
   * Formula text (for Calculated and Mixed nodes)
   */
  formula_text?: string | null;
  /**
   * Additional metadata
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Human-readable name (optional)
   */
  name?: string | null;
  /**
   * Unique identifier for this node
   */
  node_id: NodeId;
  /**
   * Node computation type
   */
  node_type: NodeType;
  /**
   * Tags for grouping/filtering
   */
  tags?: string[];
  /**
   * Value type (monetary with currency or scalar)
   */
  value_type?: NodeValueType | null;
  /**
   * Explicit values per period (for Value and Mixed nodes)
   */
  values?: {
    [k: string]: AmountOrScalar;
  } | null;
  /**
   * Where clause for conditional evaluation (optional)
   */
  where_text?: string | null;
}
/**
 * Forecast method specification.
 *
 * Defines how to forecast future values for a node.
 */
export interface ForecastSpec {
  /**
   * Forecast method
   */
  method: ForecastMethod;
  /**
   * Method-specific parameters
   */
  params?: {
    [k: string]: unknown;
  };
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
 * Percentile summaries of a Monte Carlo statement-model evaluation.
 */
export interface MonteCarloResults {
  /**
   * Forecast (non-actual) periods included in the simulation.
   */
  forecast_periods: PeriodId[];
  /**
   * Number of Monte Carlo paths simulated.
   */
  n_paths: number;
  /**
   * Optional full path data in long-format table form.
   */
  path_data?: TableEnvelope | null;
  /**
   * Aggregated percentile results: `metric → PercentileSeries`.
   */
  percentile_results: {
    [k: string]: PercentileSeries;
  };
  /**
   * Percentiles computed for each metric/period.
   */
  percentiles: number[];
  /**
   * Warnings encountered while evaluating Monte Carlo paths.
   */
  warnings?: EvalWarning[];
  [k: string]: unknown;
}
/**
 * Serializable columnar table envelope.
 *
 * Tables preserve column order and validate that every column has the same row
 * count. Optional metadata can record domain-specific hints such as which
 * column is a metric, what a numeric field represents, or how a host-language
 * binding should interpret the data.
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
 * Per-metric percentile time series.
 */
export interface PercentileSeries {
  /**
   * Metric / node identifier.
   */
  metric: string;
  /**
   * Period → ordered list of `(percentile, value)` pairs.
   */
  values: {
    [k: string]: [unknown, unknown][];
  };
  [k: string]: unknown;
}
/**
 * Financial statement normalization policy and adjustment catalog.
 */
export interface NormalizationConfig {
  /**
   * List of adjustments to apply
   */
  adjustments?: Adjustment[];
  /**
   * The target node to normalize (e.g., "EBITDA")
   */
  target_node: string;
}
/**
 * Execution statistics for a statement-model evaluation.
 *
 * Distinct from [`finstack_quant_core::config::ResultsMeta`], which is the
 * workspace-wide *audit* stamp (numeric mode, rounding context, FX policy).
 * This type records how the evaluation *ran* — timing, graph size, warnings.
 */
export interface StatementEvalStats {
  /**
   * Evaluation time in milliseconds
   */
  eval_time_ms?: number | null;
  /**
   * Number of nodes evaluated
   */
  num_nodes: number;
  /**
   * Number of periods evaluated
   */
  num_periods: number;
  /**
   * Numeric mode used for evaluation
   */
  numeric_mode?: StatementNumericMode;
  /**
   * Whether parallel evaluation was used
   */
  parallel?: boolean;
  /**
   * Warnings encountered during evaluation (division by zero, NaN propagation, etc.)
   */
  warnings?: EvalWarning[];
}
/**
 * Versioned financial statement evaluation result.
 */
export interface StatementResult {
  /**
   * Check report from inline validation (None if no checks configured)
   */
  check_report?: CheckReport | null;
  /**
   * Capital structure cashflows (populated when model has a capital_structure)
   */
  cs_cashflows?: CapitalStructureCashflows | null;
  /**
   * Metadata about the evaluation
   */
  meta: StatementEvalStats;
  /**
   * Map of node_id → (period_id → Money) for monetary nodes
   */
  monetary_nodes?: {
    [k: string]: {
      [k: string]: Money;
    };
  };
  /**
   * Track value types for each node
   */
  node_value_types?: {
    [k: string]: NodeValueType;
  };
  /**
   * Map of node_id → (period_id → value). Native values are f64; JSON uses
   * numbers for finite values and `"nan"`, `"inf"`, `"-inf"` for non-finite cells.
   */
  nodes: {
    [k: string]: {
      [k: string]: ResultNumber;
    };
  };
  /**
   * Required wire-format schema version. Only numeric `1` is accepted.
   */
  schema_version: SchemaVersion;
}
