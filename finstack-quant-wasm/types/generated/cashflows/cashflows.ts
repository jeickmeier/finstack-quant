// Generated from the finstack-quant-cashflows JSON schemas by scripts/generate-contract-types.mjs. Do not edit.
import type * as valuations from '../valuations/index.js';

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
 * Cashflow fee specification.
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
 * Fixed and floating coupon specification.
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
