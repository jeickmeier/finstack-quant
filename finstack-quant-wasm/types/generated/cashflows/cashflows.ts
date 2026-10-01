// Generated from the finstack-quant-cashflows JSON schemas by scripts/generate-contract-types.mjs. Do not edit.
import type * as valuations from '../valuations/index.js';

/**
 * Generic accrual method usable across instruments.
 *
 * This mirrors the semantics of bond accrual methods but is defined at the
 * cashflow layer so it can be reused by any instrument that exposes a
 * `CashFlowSchedule`.
 */
export type AccrualMethod = "linear" | "compounded";
/**
 * Cashflow amortization specification.
 */
export type AmortizationSpec =
  | "none"
  | {
      linear_to: {
        /**
         * Target remaining principal at the end of the amortization schedule.
         */
        final_notional: valuations.Money;
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
        end: valuations.DateWire;
        /**
         * Amortization start; installments fall on payment dates strictly
         * after it.
         */
        start: valuations.DateWire;
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
 * One canonical coupon-program instruction.
 */
export type CouponLegSpec =
  | {
      kind: "fixed";
      /**
       * Canonical fixed coupon specification.
       */
      spec: FixedCouponSpec;
    }
  | {
      kind: "floating";
      /**
       * Canonical floating coupon specification.
       */
      spec: FloatingCouponSpec;
    }
  | {
      kind: "step_up";
      /**
       * Canonical step-up coupon specification.
       */
      spec: StepUpCouponSpec;
    }
  | {
      /**
       * Exclusive window end.
       */
      end: valuations.DateWire;
      kind: "fixed_window";
      /**
       * Fixed coupon specification for the window.
       */
      spec: FixedCouponSpec;
      /**
       * Inclusive window start.
       */
      start: valuations.DateWire;
    }
  | {
      /**
       * Exclusive window end.
       */
      end: valuations.DateWire;
      kind: "floating_window";
      /**
       * Floating coupon specification for the window.
       */
      spec: FloatingCouponSpec;
      /**
       * Inclusive window start.
       */
      start: valuations.DateWire;
    }
  | {
      /**
       * Fixed coupon rate, settlement type, and schedule before the switch.
       */
      fixed: FixedCouponSpec;
      /**
       * Floating coupon specification after the switch.
       */
      floating: FloatingCouponSpec;
      kind: "fixed_to_float";
      /**
       * Date on which the floating leg begins.
       */
      switch: valuations.DateWire;
    }
  | {
      /**
       * Base floating specification; its `spread_bp` applies from issue
       * until the first step.
       */
      base: FloatingCouponSpec;
      kind: "floating_margin_program";
      /**
       * Margin steps in strictly increasing date order; each spread applies
       * from its date until the next step (the last one to maturity).
       */
      steps: MarginStepSpec[];
    };
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
        cash_fraction: valuations.DecimalWire;
        /**
         * Fraction of the coupon capitalized as PIK, expressed as a decimal
         * share in `[0, 1]`.
         */
        pik_fraction: valuations.DecimalWire;
      };
    };
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
      fixed_rate: valuations.DecimalWire;
    };
/**
 * Where overnight index floors/caps are applied for daily-compounded rates.
 */
export type OvernightIndexConstraintApplication = "daily" | "period";
/**
 * Fee specification for fixed-fee and periodic-basis-point programs.
 *
 * Sign policy: any non-zero fee amount is emitted. Negative fixed amounts and
 * negative `bp` quotes (rebates) flow through as negative fee cashflows for
 * both variants.
 */
export type FeeSpec =
  | {
      fixed: {
        /**
         * Fee amount in currency units.
         */
        amount: valuations.Money;
        /**
         * Payment date of the fixed fee.
         */
        date: valuations.DateWire;
      };
    }
  | {
      periodic_bp: {
        /**
         * How the outstanding balance is sampled for fee calculation.
         */
        accrual_basis?: FeeAccrualBasis;
        /**
         * Economic balance used as the fee base.
         */
        base: FeeBase;
        /**
         * Fee quote in basis points per annum, stored as `Decimal` to preserve
         * the quoted value exactly.
         */
        bp: valuations.DecimalWire;
        /**
         * Business-day convention applied to generated fee dates.
         */
        business_day_convention: valuations.BusinessDayConvention;
        /**
         * Holiday calendar identifier used with `business_day_convention`.
         *
         * Use `"weekends_only"` when only weekend adjustment is required.
         */
        calendar_id: string;
        /**
         * Day-count convention used to annualize the fee accrual.
         */
        day_count: valuations.DayCount;
        /**
         * Accrual and payment frequency for the fee schedule.
         */
        frequency: valuations.Tenor;
        /**
         * Stub-handling rule for irregular first or last fee periods
         * (`ShortFront` when omitted from the wire form).
         */
        stub?: StubKind;
      };
    };
/**
 * Controls how the outstanding balance is sampled during fee accrual.
 */
export type FeeAccrualBasis = "point_in_time" | "time_weighted_average";
/**
 * Fee base for periodic bp fees.
 */
export type FeeBase =
  | "drawn"
  | {
      undrawn: {
        /**
         * Total facility commitment used to compute the undrawn amount.
         */
        commitment: valuations.Money;
      };
    };
/**
 * One canonical payment-split instruction.
 */
export type PaymentProgramSpec =
  | {
      /**
       * Exclusive window end.
       */
      end: valuations.DateWire;
      kind: "window";
      /**
       * Settlement behavior active in the window.
       */
      split: CouponType;
      /**
       * Inclusive window start.
       */
      start: valuations.DateWire;
    }
  | {
      kind: "program";
      /**
       * Dated settlement steps in strictly increasing order.
       */
      steps: PaymentStepSpec[];
    };
/**
 * Whether the builder emits issue-funding and maturity-redemption notionals.
 *
 * Outstanding still starts at the configured initial principal for coupon
 * math. Scheduled [`AmortizationSpec`](super::AmortizationSpec) payments and
 * explicit principal events still emit. This is not the cross-currency
 * `NotionalExchange` policy (no final-only or MTM-resetting variants).
 *
 * # Variants
 *
 * - **`None`**: track outstanding only; no `CFKind::Notional` issue or
 *   redemption flows. Vanilla IRS and basis swaps use this.
 * - **`InitialAndFinal`** (default): emit issue funding and the maturity
 *   balloon on the lagged redemption date.
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_cashflows::builder::PrincipalExchange;
 *
 * assert_eq!(
 *     PrincipalExchange::default(),
 *     PrincipalExchange::InitialAndFinal
 * );
 * ```
 */
export type PrincipalExchange = "none" | "initial_and_final";
/**
 * Default curve shape.
 */
export type DefaultCurve =
  | {
      curve: "constant";
      [k: string]: unknown;
    }
  | {
      curve: "sda";
      /**
       * Speed multiplier (1.0 = 100% SDA)
       */
      speed_multiplier: number;
      [k: string]: unknown;
    }
  | {
      curve: "vector";
      /**
       * Annual CDR per month of seasoning as decimals, month 1 first.
       */
      monthly_cdr: number[];
      [k: string]: unknown;
    }
  | {
      /**
       * Cumulative net loss in percent of the original balance per month
       * of seasoning (`1.5` = 1.5%), non-decreasing, month 1 first; the
       * last value is held.
       */
      cumulative_net_loss_pct: number[];
      curve: "cumulative_loss";
      /**
       * Loss severity as a decimal fraction of defaulted par in `(0, 1]`.
       */
      severity: number;
      [k: string]: unknown;
    }
  | {
      /**
       * Share of lifetime defaults occurring in each year of seasoning, in
       * percent (e.g. `[15, 30, 30, 15, 10]`); must sum to 100.
       */
      annual_pct: number[];
      /**
       * Lifetime defaults as a decimal fraction of the original balance.
       */
      cumulative_default_rate: number;
      curve: "timing";
      [k: string]: unknown;
    };
/**
 * Prepayment curve shape.
 */
export type PrepaymentCurve =
  | {
      curve: "constant";
      [k: string]: unknown;
    }
  | {
      curve: "psa";
      /**
       * Speed multiplier (1.0 = 100% PSA)
       */
      speed_multiplier: number;
      [k: string]: unknown;
    }
  | {
      curve: "cmbs_lockout";
      /**
       * Number of months with zero prepayment (e.g., 60 for 5-year lockout)
       */
      lockout_months: number;
      [k: string]: unknown;
    }
  | {
      curve: "abs";
      /**
       * Monthly prepayment as a decimal fraction of the original balance
       * (`0.015` = 1.5% ABS).
       */
      speed: number;
      [k: string]: unknown;
    }
  | {
      curve: "vector";
      /**
       * Annual CPR per month of seasoning as decimals, month 1 first.
       */
      monthly_cpr: number[];
      [k: string]: unknown;
    };

/**
 * Schedule-driven accrued-interest configuration.
 */
export interface AccrualConfig {
  /**
   * Optional ex-coupon rule applied to coupon dates.
   */
  ex_coupon?: ExCouponRule | null;
  /**
   * Coupon frequency — required for ACT/ACT ISMA day count.
   *
   * When `None` and the schedule uses ACT/ACT ISMA, year-fraction
   * calculations return
   * [`InputError::MissingFrequencyForActActIsma`](finstack_quant_core::InputError::MissingFrequencyForActActIsma);
   * there is no fallback to ISDA semantics.
   */
  frequency?: valuations.Tenor | null;
  /**
   * Whether to include PIK interest in the accrued amount.
   */
  include_pik: boolean;
  /**
   * Accrual method (Linear or Compounded).
   */
  method: AccrualMethod;
}
/**
 * Ex-coupon convention applied to coupon flows.
 */
export interface ExCouponRule {
  /**
   * Optional calendar ID for business day calculation.
   *
   * - `Some(id)`: Subtract N business days from payment date.
   * - `None`: Subtract N calendar days from payment date.
   */
  calendar_id?: valuations.Id | null;
  /**
   * Number of days before coupon date that go ex.
   *
   * Values greater than 366 are rejected by [`ExCouponRule::ex_date`].
   */
  days_before_coupon: number;
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
  amount: valuations.Money;
  /**
   * Payment date (or payment date for principal/fee, or reset date for `CFKind::FloatReset`).
   */
  date: valuations.DateWire;
  /**
   * Category/kind of cash-flow.
   */
  kind: CFKind;
  /**
   * Economic date of the principal movement, independent of cash payment.
   * When absent, principal changes on `date`. Scheduled amortization and
   * PIK use the contractual accrual boundary even when payment is adjusted.
   */
  principal_date?: valuations.DateWire | null;
  /**
   * Explicit change in outstanding principal, in the cashflow currency.
   * Positive increases outstanding. When absent, the flow kind and amount
   * determine the balance movement. Initial funding remains represented by
   * the schedule's initial notional rather than being counted twice.
   */
  principal_delta?: valuations.Money | null;
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
  reset_date?: valuations.DateWire | null;
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
  day_count: valuations.DayCount;
  /**
   * Contractual accrual-period end date.
   */
  end: valuations.DateWire;
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
  start: valuations.DateWire;
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
  commitment?: valuations.Money | null;
  /**
   * Issue date of the instrument, when known.
   *
   * Used by the accrual engine to establish the first coupon period start
   * date precisely, avoiding the inverse day count approximation that can
   * be off by 1-2 days.
   */
  issue_date?: valuations.DateWire | null;
  /**
   * Contractual maturity date, distinct from an adjusted final payment date.
   */
  maturity?: valuations.DateWire | null;
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
  date: valuations.DateWire;
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
 * Canonical cashflow schedule.
 */
export interface CashFlowSchedule {
  /**
   * Day count convention for interest calculations
   */
  day_count: valuations.DayCount;
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
  initial: valuations.Money;
}
/**
 * Cashflow schedule build specification.
 */
export interface CashflowScheduleBuildSpec {
  /**
   * Coupon instructions, applied in order through the canonical builder.
   */
  coupon_program?: CouponLegSpec[];
  /**
   * Fee legs to add to the schedule.
   */
  fees?: FeeSpec[];
  /**
   * Contract issue date.
   */
  issue_date: valuations.DateWire;
  /**
   * Contract maturity date.
   */
  maturity: valuations.DateWire;
  /**
   * Principal amount and amortization behavior.
   */
  notional: Notional;
  /**
   * Payment-split instructions, applied after coupon instructions.
   */
  payment_program?: PaymentProgramSpec[];
  /**
   * Explicit principal events to add after the base principal setup.
   */
  principal_events?: PrincipalEventSpec[];
  /**
   * Whether to emit issue funding and maturity redemption notionals.
   *
   * Defaults to [`PrincipalExchange::InitialAndFinal`]. Set to
   * [`PrincipalExchange::None`] for coupon-only schedules (vanilla IRS).
   */
  principal_exchange?: PrincipalExchange;
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
  business_day_convention?: valuations.BusinessDayConvention;
  /**
   * Holiday calendar identifier used together with `business_day_convention`.
   *
   * Use `"weekends_only"` when only Saturday/Sunday adjustment is needed.
   */
  calendar_id: valuations.Id;
  /**
   * Coupon settlement behavior: cash, PIK, or an explicit split of the
   * coupon amount.
   */
  coupon_type?: CouponType;
  /**
   * Day-count convention used to convert each generated accrual period into a
   * year fraction.
   */
  day_count: valuations.DayCount;
  /**
   * Whether end-of-month rolling should be preserved when generating the
   * schedule.
   */
  end_of_month?: boolean;
  /**
   * Accrual and payment frequency used to generate the schedule boundaries.
   */
  frequency: valuations.Tenor;
  /**
   * Payment lag in business days after the adjusted accrual end date.
   */
  payment_lag_days?: number;
  /**
   * Coupon rate as a decimal (e.g., 0.05 for 5%). Uses Decimal for exact representation.
   */
  rate: valuations.DecimalWire;
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
  business_day_convention?: valuations.BusinessDayConvention;
  /**
   * Holiday calendar identifier used together with `business_day_convention`.
   *
   * Use `"weekends_only"` when only Saturday/Sunday adjustment is needed.
   */
  calendar_id: valuations.Id;
  /**
   * Coupon type (Cash/PIK/Split).
   */
  coupon_type?: CouponType;
  /**
   * Day-count convention used to convert each generated accrual period into a
   * year fraction.
   */
  day_count: valuations.DayCount;
  /**
   * Whether end-of-month rolling should be preserved when generating the
   * schedule.
   */
  end_of_month?: boolean;
  /**
   * Accrual and payment frequency used to generate the schedule boundaries.
   */
  frequency: valuations.Tenor;
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
  all_in_cap_bp?: valuations.DecimalWire | null;
  /**
   * Floor on all-in rate in basis points (Min Coupon).
   *
   * Applied to the final calculated rate after gearing and spread.
   */
  all_in_floor_bp?: valuations.DecimalWire | null;
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
  fixing_calendar_id?: valuations.Id | null;
  /**
   * Forward curve identifier (e.g., "USD-SOFR-3M", "EUR-EURIBOR-6M").
   */
  forward_curve_id: valuations.Id;
  /**
   * Gearing/leverage multiplier applied to the all-in rate (default: 1.0).
   *
   * Example: gearing = 2.0 means the rate is doubled.
   *
   * **Restriction:** gearing must be strictly positive (`gearing > 0`);
   * projection rejects zero or negative gearing, so inverse floaters
   * (negative gearing) are not currently expressible with this field.
   */
  gearing?: valuations.DecimalWire;
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
  index_cap_bp?: valuations.DecimalWire | null;
  /**
   * Floor on index rate in basis points (applied to index component).
   *
   * Example: index_floor_bp = Some(0.0) ensures index rate >= 0%.
   */
  index_floor_bp?: valuations.DecimalWire | null;
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
  index_tenor?: valuations.Tenor | null;
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
  overnight_basis?: valuations.DayCount | null;
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
  reset_frequency: valuations.Tenor;
  /**
   * Reset lag in business days (e.g., 2 for T-2 SOFR convention).
   */
  reset_lag_days?: number;
  /**
   * Spread/margin over index in basis points. Uses Decimal for exact representation.
   */
  spread_bp: valuations.DecimalWire;
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
  business_day_convention?: valuations.BusinessDayConvention;
  /**
   * Holiday calendar identifier used together with `business_day_convention`.
   *
   * Use `"weekends_only"` when only Saturday/Sunday adjustment is needed.
   */
  calendar_id: valuations.Id;
  /**
   * Coupon type (Cash/PIK/Split).
   */
  coupon_type?: CouponType;
  /**
   * Day-count convention used to convert each generated accrual period into a
   * year fraction.
   */
  day_count: valuations.DayCount;
  /**
   * Whether end-of-month rolling should be preserved when generating the
   * schedule.
   */
  end_of_month?: boolean;
  /**
   * Accrual and payment frequency used to generate the schedule boundaries.
   */
  frequency: valuations.Tenor;
  /**
   * Initial coupon rate (annual, decimal). Used until the first step date.
   */
  initial_rate: valuations.DecimalWire;
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
 * A dated floating-margin step, effective from its date.
 */
export interface MarginStepSpec {
  /**
   * Effective-from date: the new spread applies to coupon periods from
   * this date until the next step.
   */
  date: valuations.DateWire;
  /**
   * Floating margin over the index from `date`, in basis points
   * (`250` = 2.50%); replaces the base `spread_bp`.
   */
  spread_bp: valuations.DecimalWire;
}
/**
 * A dated payment-split step.
 */
export interface PaymentStepSpec {
  /**
   * Boundary date for the payment split.
   */
  date: valuations.DateWire;
  /**
   * Settlement behavior active for the step.
   */
  split: CouponType;
}
/**
 * JSON representation of an explicit principal event.
 */
export interface PrincipalEventSpec {
  /**
   * Optional cash leg. When omitted, the cash leg equals `delta`.
   */
  cash?: valuations.Money | null;
  /**
   * Event date.
   */
  date: valuations.DateWire;
  /**
   * Outstanding balance delta. Positive increases outstanding, negative repays.
   */
  delta: valuations.Money;
  /**
   * Cashflow classification to emit.
   */
  kind: CFKind;
  /**
   * Cash settlement date, independently adjusted from the economic event date.
   */
  payment_date: valuations.DateWire;
}
/**
 * Cashflow default-model specification.
 */
export interface DefaultModelSpec {
  /**
   * CDR: Constant Default Rate (annual, e.g., 0.02 for 2%).
   *
   * This field is **ignored** when any curve other than
   * [`DefaultCurve::Constant`] is active: the monthly rate is then derived
   * entirely from the curve.
   */
  cdr: number;
  /**
   * Optional curve shape (default: constant)
   */
  curve?: DefaultCurve | null;
}
/**
 * Currency-preserving cashflow totals by reporting period.
 */
export interface PeriodAggregation {
  [k: string]: {
    [k: string]: valuations.Money;
  };
}
/**
 * Cashflow prepayment-model specification.
 */
export interface PrepaymentModelSpec {
  /**
   * CPR: Constant Prepayment Rate (annual, e.g., 0.06 for 6%).
   *
   * This field is **ignored** when [`PrepaymentCurve::Psa`],
   * [`PrepaymentCurve::Abs`] or [`PrepaymentCurve::Vector`] is active: the
   * monthly rate is then derived entirely from the curve. It IS used by
   * [`PrepaymentCurve::CmbsLockout`] as the post-lockout CPR.
   */
  cpr: number;
  /**
   * Optional curve shape (default: constant)
   */
  curve?: PrepaymentCurve | null;
}
/**
 * Cashflow recovery-model specification.
 */
export interface RecoveryModelSpec {
  /**
   * Recovery rate as fraction (0.0 to 1.0, e.g., 0.40 for 40%)
   */
  rate: number;
  /**
   * Recovery lag in months
   */
  recovery_lag: number;
  /**
   * Loss severity (`1 − recovery`) by month of default as decimals in
   * `[0, 1]`, month 1 first; the last value is held. When present it
   * replaces `rate` for defaults in that month.
   */
  severity_vector?: number[] | null;
}
/**
 * Schedule-level inputs for building a schedule from existing flows.
 */
export interface ScheduleBuildOpts {
  /**
   * Schedule-level metadata.
   */
  meta?: CashFlowMeta;
  /**
   * Optional notional amount to stamp on the resulting schedule. When
   * `None`, the constructor uses a zero notional in the currency of the
   * first supplied flow (or USD if the list is empty).
   */
  notional_hint?: valuations.Money | null;
}
/**
 * Cashflow schedule construction parameters.
 */
export interface ScheduleParams {
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
  business_day_convention?: valuations.BusinessDayConvention;
  /**
   * Holiday calendar identifier used together with `business_day_convention`.
   *
   * Use `"weekends_only"` when only Saturday/Sunday adjustment is needed.
   */
  calendar_id: valuations.Id;
  /**
   * Day-count convention used to convert each generated accrual period into a
   * year fraction.
   */
  day_count: valuations.DayCount;
  /**
   * Whether end-of-month rolling should be preserved when generating the
   * schedule.
   */
  end_of_month?: boolean;
  /**
   * Accrual and payment frequency used to generate the schedule boundaries.
   */
  frequency: valuations.Tenor;
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
   * Stub-handling rule used when the start/end dates do not fit an exact
   * whole number of periods.
   */
  stub?: StubKind;
}
