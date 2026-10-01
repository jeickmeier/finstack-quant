// Generated from the finstack-quant-valuations JSON schemas by scripts/generate-contract-types.mjs. Do not edit.
import type * as cashflows from '../cashflows/index.js';

/**
 * Generic accrual method usable across instruments.
 *
 * This mirrors the semantics of bond accrual methods but is defined at the
 * cashflow layer so it can be reused by any instrument that exposes a
 * `CashFlowSchedule`.
 */
export type AccrualMethod = "linear" | "compounded";
/**
 * ISO 8601 calendar date string.
 */
export type DateWire = string;
/**
 * Servicer advancing policy for missed principal and interest.
 */
export type AdvancingPolicy =
  | {
      policy: "none";
    }
  | {
      policy: "principal_and_interest";
      /**
       * Cap on advances outstanding as a percent of the delinquent
       * balance (`100.0` = advance up to the full delinquent balance).
       */
      recoverability_cap_pct: number;
      /**
       * `true` reimburses non-recoverable advances from all pool
       * collections at the top of the waterfall (the trust bears the
       * shortfall); `false` (default) leaves them with the servicer.
       */
      reimburse_from_collections?: boolean;
    };
/**
 * Agency program enumeration.
 *
 * Identifies the government-sponsored enterprise (GSE) or government agency
 * that guarantees the mortgage-backed security.
 *
 * # GNMA Programs
 *
 * Ginnie Mae has two distinct programs with different payment delay conventions:
 * - **GNMA I**: Single-issuer pools with a 45-day stated delay. Payments on the 15th.
 * - **GNMA II**: Multi-issuer pools with a 50-day stated delay. Payments on the 20th.
 *
 * Use `GnmaI` or `GnmaII` to select the appropriate convention. Their
 * persisted values are exactly `gnma_i` and `gnma_ii`, respectively.
 */
export type AgencyProgram = "fnma" | "fhlmc" | "gnma_i" | "gnma_ii";
/**
 * Exact decimal encoded as a JSON string.
 */
export type DecimalWire = string;
/**
 * ISO 4217 currency code.
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
 * Day-count convention.
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
 * Opaque string identifier.
 */
export type Id = string;
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
 * Barrier-crossing detection policy for first-passage default simulation.
 *
 * `Discrete` only checks the barrier at grid points (fast but biased for
 * coarse time steps). `BrownianBridge` uses a Brownian-bridge crossing
 * probability between grid points to approximate continuous monitoring.
 */
export type BarrierCrossing = "discrete" | "brownian_bridge";
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
 * Time-varying PIK schedule for the MC engine.
 *
 * Controls per-coupon PIK behavior, either uniformly or as a step
 * function over time.
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
 * Unit of a tenor period.
 */
export type TenorUnit = "days" | "weeks" | "months" | "years";
/**
 * Finite JSON number in the open interval `(0, 1)`.
 */
export type OpenUnitIntervalF64Wire = number;
/**
 * VaR calculation method.
 */
export type VarMethod = "full_revaluation" | "taylor_approximation";
/**
 * Pool type classification.
 *
 * Distinguishes between generic (TBA-eligible) pools and specified pools
 * with known characteristics.
 */
export type PoolType = "generic" | "specified";
/**
 * CMO tranche type enumeration.
 */
export type CmoTrancheType = "sequential" | "pac" | "support" | "interest_only" | "principal_only" | "accrual";
/**
 * SIFMA MBS settlement class.
 *
 * SIFMA publishes distinct settlement dates for four classes of agency MBS.
 * The class determines which specific settlement date applies within a given
 * month. See <https://www.sifma.org/resources/general/mbs-notification-and-settlement-dates/>.
 */
export type SifmaSettlementClass = "a" | "b" | "c" | "d";
/**
 * TBA term enumeration (original loan term).
 */
export type TbaTerm = "fifteen_year" | "twenty_year" | "thirty_year";
/**
 * Allocation mode within a tier
 */
export type AllocationMode = "sequential" | "pro_rata";
/**
 * Event that ends the revolving period early and turns the facility out
 * (principal collections then repay the lender sequentially).
 */
export type AmortizationEvent =
  | {
      /**
       * Date revolving stops.
       */
      date: DateWire;
      kind: "date";
    }
  | {
      kind: "cumulative_loss";
      /**
       * Loss threshold as a decimal fraction of the original collateral
       * (`0.04` = 4%), in `(0, 1]`.
       */
      max_cumulative_loss: number;
    }
  | {
      kind: "excess_spread";
      /**
       * Excess-spread floor as an annual decimal (`0.01` = 1%).
       */
      min_excess_spread_3m: number;
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
 * Asian averaging method.
 *
 * Serialized as `"arithmetic"` / `"geometric"`; this is the single definition
 * shared by the Monte Carlo payoffs and the `averaging_method` wire field of
 * the Asian option instruments.
 */
export type AveragingMethod = "arithmetic" | "geometric";
/**
 * Option payoff direction used by analytical and numerical model engines.
 */
export type OptionType = "call" | "put";
/**
 * Dimension a concentration limit is measured on.
 */
export type ConcentrationScope = "obligor" | "industry" | "asset_class";
/**
 * Asset type classification for pool composition (flattened hierarchy)
 */
export type AssetType =
  | {
      type: "first_lien_loan";
    }
  | {
      type: "second_lien_loan";
    }
  | {
      type: "revolver_loan";
    }
  | {
      type: "bridge_loan";
    }
  | {
      type: "mezzanine_loan";
    }
  | {
      type: "high_yield_bond";
    }
  | {
      type: "investment_grade_bond";
    }
  | {
      type: "distressed_bond";
    }
  | {
      type: "emerging_markets_bond";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "single_family_mortgage";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "multifamily_mortgage";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "commercial_mortgage";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "industrial_mortgage";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "retail_mortgage";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "office_mortgage";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "hospitality_mortgage";
    }
  | {
      /**
       * Free-text description of the property type.
       */
      description: string;
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "other_mortgage";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "new_auto_loan";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "used_auto_loan";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "lease_auto_loan";
    }
  | {
      /**
       * Ltv.
       */
      ltv?: number | null;
      type: "fleet_auto_loan";
    }
  | {
      type: "prime_credit_card";
    }
  | {
      type: "sub_prime_credit_card";
    }
  | {
      type: "super_prime_credit_card";
    }
  | {
      type: "commercial_credit_card";
    }
  | {
      type: "federal_student_loan";
    }
  | {
      type: "private_student_loan";
    }
  | {
      type: "ffelp_student_loan";
    }
  | {
      type: "consolidation_student_loan";
    }
  | {
      /**
       * Equipment type.
       */
      equipment_type: string;
      type: "equipment";
    }
  | {
      /**
       * Asset class.
       */
      asset_class: string;
      /**
       * Description.
       */
      description: string;
      type: "generic";
    };
/**
 * Prepayment protection on a commercial mortgage.
 *
 * The premium a prepayment owes is collected by the trust as interest
 * (nothing is charged after `through`); a lockout blocks voluntary
 * prepayment outright.
 */
export type PrepaymentPenalty =
  | {
      kind: "lockout";
      /**
       * Last date of the lockout; `None` locks out to maturity.
       */
      through?: DateWire | null;
    }
  | {
      kind: "fixed";
      /**
       * Premium in percent of the prepaid balance (`3.0` = 3%).
       */
      pct: number;
      /**
       * Last date the penalty applies; `None` applies it to maturity.
       */
      through?: DateWire | null;
    }
  | {
      kind: "step_down";
      /**
       * Steps in ascending `through` order, at least one.
       */
      schedule: PenaltyStep[];
    }
  | {
      /**
       * Discount curve the lost coupons are valued on; `None` leaves them
       * undiscounted.
       */
      discount_curve_id?: Id | null;
      /**
       * Minimum premium in percent of the prepaid balance; `None` for no
       * floor.
       */
      floor_pct?: number | null;
      kind: "yield_maintenance";
      /**
       * Annual reinvestment rate as a decimal; `None` uses the curve's
       * zero rate to maturity (then `discount_curve_id` is required).
       */
      reinvestment_rate?: number | null;
      /**
       * Last date the penalty applies; `None` applies it to maturity.
       */
      through?: DateWire | null;
    };
/**
 * Unified credit rating scale with notch-level precision (agency-agnostic).
 *
 * Each variant represents a specific notch in the rating scale. Ratings
 * without a `Plus`/`Minus` suffix represent the "flat" (middle) notch.
 *
 * | Variant     | S&P/Fitch | Moody's |
 * |-------------|-----------|---------|
 * | `AAA`       | AAA       | Aaa     |
 * | `AAPlus`    | AA+       | Aa1     |
 * | `AA`        | AA        | Aa2     |
 * | `AAMinus`   | AA-       | Aa3     |
 * | `APlus`     | A+        | A1      |
 * | `A`         | A         | A2      |
 * | `AMinus`    | A-        | A3      |
 * | `BBBPlus`   | BBB+      | Baa1    |
 * | `BBB`       | BBB       | Baa2    |
 * | `BBBMinus`  | BBB-      | Baa3    |
 * | `BBPlus`    | BB+       | Ba1     |
 * | `BB`        | BB        | Ba2     |
 * | `BBMinus`   | BB-       | Ba3     |
 * | `BPlus`     | B+        | B1      |
 * | `B`         | B         | B2      |
 * | `BMinus`    | B-        | B3      |
 * | `CCCPlus`   | CCC+      | Caa1    |
 * | `CCC`       | CCC       | Caa2    |
 * | `CCCMinus`  | CCC-      | Caa3    |
 * | `CC`        | CC        | Ca      |
 * | `C`         | C         | C       |
 * | `D`         | D         | D       |
 * | `NR`        | NR        | NR      |
 *
 * # Investment Grade
 *
 * Ratings of `BBBMinus` and above are considered "investment grade":
 */
export type CreditRating =
  | "AAA"
  | "AA+"
  | "AA"
  | "AA-"
  | "A+"
  | "A"
  | "A-"
  | "BBB+"
  | "BBB"
  | "BBB-"
  | "BB+"
  | "BB"
  | "BB-"
  | "B+"
  | "B"
  | "B-"
  | "CCC+"
  | "CCC"
  | "CCC-"
  | "CC"
  | "C"
  | "D"
  | "NR";
/**
 * Primary structured credit deal classification
 */
export type DealType = "clo" | "cbo" | "abs" | "rmbs" | "cmbs" | "auto" | "card";
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
      fixed: cashflows.FixedCouponSpec;
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
 * Business-day adjustment convention.
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
 * # See Also
 *
 * - [`ScheduleBuilder::stub_rule`] to configure stub behavior
 */
export type StubKind = "none" | "short_front" | "short_back" | "long_front" | "long_back";
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
 * Rule the pool engine applies to issuer call options on the collateral.
 *
 * Set at the deal level on [`InstrumentCollateral::call_exercise`] and
 * overridable per instrument through [`InstrumentExerciseOverride`].
 */
export type CallExercisePolicy =
  | {
      policy: "contractual";
    }
  | {
      policy: "first_call";
    }
  | {
      policy: "worst";
    }
  | {
      policy: "refinancing_incentive";
      /**
       * Minimum coupon-over-refinancing-rate saving, in basis points, that
       * triggers exercise.
       */
      threshold_bp: number;
    };
/**
 * Rule the pool engine applies to holder put options on the collateral.
 */
export type PutExercisePolicy =
  | {
      policy: "never";
    }
  | {
      policy: "first_put";
    }
  | {
      policy: "reinvestment_incentive";
      /**
       * Minimum reinvestment-rate-over-coupon pickup, in basis points,
       * that triggers exercise.
       */
      threshold_bp: number;
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
 * Where the interest earned on the deal reserve account is paid.
 */
export type ReserveInterestDestination =
  | {
      kind: "waterfall";
    }
  | {
      kind: "tranche";
      /**
       * Identifier of the receiving tranche; must exist in the deal.
       */
      tranche_id: string;
    }
  | {
      kind: "retain";
    };
/**
 * Correlation structure specification.
 *
 * Captures the various correlation parameters needed for
 * stochastic structured credit modeling.
 */
export type CorrelationStructure =
  | {
      asset_correlation: number;
      prepay_default_correlation: number;
      structure: "flat";
    }
  | {
      inter_sector: number;
      intra_sector: number;
      prepay_default: number;
      structure: "sectored";
    }
  | {
      correlations: number[];
      labels: string[];
      structure: "matrix";
    };
/**
 * Stochastic default model specification.
 *
 * Allows default model selection and configuration without
 * constructing the full model.
 */
export type StochasticDefaultSpec =
  | {
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
      model: "deterministic";
    }
  | {
      /**
       * Base annual CDR
       */
      base_cdr: number;
      /**
       * Copula specification
       */
      copula_spec: CopulaSpec;
      /**
       * Asset correlation
       */
      correlation: number;
      model: "copula";
    }
  | {
      /**
       * Base annual hazard rate
       */
      base_hazard: number;
      /**
       * Asset correlation
       */
      correlation?: number;
      /**
       * Loading (β) on the systematic factor, clamped to [-1, 1]
       */
      factor_loading: number;
      /**
       * Mean reversion speed
       */
      mean_reversion: number;
      model: "intensity_process";
      /**
       * Intensity volatility
       */
      volatility: number;
    }
  | {
      /**
       * Base deterministic default specification
       */
      base_spec: cashflows.DefaultModelSpec;
      /**
       * CDR volatility
       */
      cdr_volatility: number;
      /**
       * Factor loading
       */
      factor_loading: number;
      model: "factor_correlated";
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
 * Copula model specification for configuration and serialization.
 *
 * Allows copula selection without constructing the full model,
 * enabling deferred construction with market data.
 */
export type CopulaSpec =
  | {
      type: "gaussian";
    }
  | {
      /**
       * Degrees of freedom (must be > 2 for finite variance)
       */
      degrees_of_freedom: number;
      type: "student_t";
    }
  | {
      /**
       * Volatility of the factor loading (correlation vol proxy)
       */
      loading_volatility: number;
      type: "random_factor_loading";
    }
  | {
      type: "multi_factor";
    };
/**
 * Stochastic prepayment model specification.
 *
 * Allows prepayment model selection and configuration without
 * constructing the full model, enabling serialization and deferred construction.
 */
export type StochasticPrepaySpec =
  | {
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
      model: "deterministic";
    }
  | {
      /**
       * Base deterministic prepayment specification
       */
      base_spec: cashflows.PrepaymentModelSpec;
      /**
       * CPR volatility (typical: 0.15-0.30)
       */
      cpr_volatility: number;
      /**
       * Factor loading (typical: 0.3-0.5)
       */
      factor_loading: number;
      model: "factor_correlated";
    }
  | {
      /**
       * Base CPR at full seasoning
       */
      base_cpr: number;
      /**
       * Burnout decay rate
       */
      burnout_rate: number;
      /**
       * CPR volatility
       */
      cpr_volatility?: number;
      /**
       * Factor loading for correlation
       */
      factor_loading?: number;
      model: "richard_roll";
      /**
       * AssetPool weighted average coupon
       */
      pool_coupon: number;
      /**
       * Refinancing sensitivity (gamma)
       */
      refi_sensitivity: number;
    }
  | {
      /**
       * CPR volatility for systematic prepayment shocks.
       */
      cpr_volatility?: number;
      /**
       * Factor loading for systematic prepayment shocks.
       */
      factor_loading?: number;
      /**
       * CPR in high prepayment regime
       */
      high_cpr: number;
      /**
       * CPR in low prepayment regime
       */
      low_cpr: number;
      model: "regime_switching";
      /**
       * Transition probability: high -> low (per month)
       */
      transition_down: number;
      /**
       * Transition probability: low -> high (per month)
       */
      transition_up: number;
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
 * Recovery model specification for configuration and serialization.
 *
 * Allows recovery model selection without constructing the full model.
 */
export type RecoverySpec =
  | {
      /**
       * Recovery rate ∈ [0, 1]
       */
      rate: number;
      type: "constant";
    }
  | {
      /**
       * Correlation with the systematic factor. Under the canonical
       * low-factor-stress convention this is typically POSITIVE: recovery
       * falls when the factor falls, so defaults (high in stress) and
       * recoveries co-move negatively.
       */
      factor_correlation: number;
      /**
       * Mean recovery rate
       */
      mean_recovery: number;
      /**
       * Recovery volatility (standard deviation)
       */
      recovery_volatility: number;
      type: "market_correlated";
    };
/**
 * Final payoff type for autocallable products.
 */
export type FinalPayoffType =
  | {
      capital_protection: {
        /**
         * Minimum return floor (e.g., 1.0 for 100% protection)
         */
        floor: number;
        /**
         * Decimal multiplier on the capped performance ratio (1.0 = 100%).
         */
        participation_rate: number;
      };
    }
  | {
      participation: {
        /**
         * Decimal multiplier on the capped upside (1.0 = 100% participation).
         */
        participation_rate: number;
      };
    }
  | {
      knock_in_put: {
        /**
         * Put strike as a ratio of the initial level (`1.0` = 100%, the same
         * units as `final_barrier`); the put loss is
         * `max(strike_ratio - S_T / S_0, 0)`.
         */
        strike_ratio: number;
      };
    };
/**
 * Explicit stochastic model policy for path-dependent equity structures.
 */
export type EquityPathModel = "atm_term_gbm";
/**
 * Side of spot on which a barrier sits.
 */
export type BarrierDirection = "up" | "down";
/**
 * Four-state barrier option classification.
 */
export type BarrierType = "up_and_out" | "up_and_in" | "down_and_out" | "down_and_in";
/**
 * Contractual barrier-monitoring convention.
 */
export type Monitoring =
  | {
      type: "continuous";
    }
  | {
      /**
       * Strictly increasing dates on which the barrier level is observed.
       */
      observation_dates: DateWire[];
      type: "discrete";
    };
/**
 * When a barrier-triggered cash amount is paid.
 *
 * Used for a knock-out rebate (`rebate_timing`) and for a one-touch payout
 * (`payout_timing`). At-expiry payments are discounted from expiry; at-hit
 * payments are discounted from the first-passage time. Knock-in rebates
 * always pay at expiry (only then is it known that no hit occurred), so this
 * setting does not affect them.
 */
export type PayoutTiming = "at_hit" | "at_expiry";
/**
 * Reference to a constituent asset in the basket
 */
export type ConstituentReference =
  | {
      kind: "instrument";
      value: InstrumentJson;
    }
  | {
      kind: "market_data";
      value: {
        /**
         * Type of asset for validation
         */
        asset_type: BasketAssetType;
        /**
         * Price identifier in MarketContext
         */
        price_id: Id;
      };
    };
/**
 * Tagged `{type, spec}` payload for one instrument, without the envelope marker.
 */
export type InstrumentJson =
  | {
      spec: Bond;
      type: "bond";
    }
  | {
      spec: ConvertibleBond;
      type: "convertible_bond";
    }
  | {
      spec: InflationLinkedBond;
      type: "inflation_linked_bond";
    }
  | {
      spec: TermLoan;
      type: "term_loan";
    }
  | {
      spec: RevolvingCredit;
      type: "revolving_credit";
    }
  | {
      spec: AgencyMbsPassthrough;
      type: "agency_mbs_passthrough";
    }
  | {
      spec: AgencyTba;
      type: "agency_tba";
    }
  | {
      spec: AgencyCmo;
      type: "agency_cmo";
    }
  | {
      spec: DollarRoll;
      type: "dollar_roll";
    }
  | {
      spec: InterestRateSwap;
      type: "interest_rate_swap";
    }
  | {
      spec: BasisSwap;
      type: "basis_swap";
    }
  | {
      spec: XccySwap;
      type: "xccy_swap";
    }
  | {
      spec: InflationSwap;
      type: "inflation_swap";
    }
  | {
      spec: YoYInflationSwap;
      type: "yoy_inflation_swap";
    }
  | {
      spec: InflationCapFloor;
      type: "inflation_cap_floor";
    }
  | {
      spec: ForwardRateAgreement;
      type: "forward_rate_agreement";
    }
  | {
      spec: Swaption;
      type: "swaption";
    }
  | {
      spec: BermudanSwaption;
      type: "bermudan_swaption";
    }
  | {
      spec: InterestRateFuture;
      type: "interest_rate_future";
    }
  | {
      spec: InterestRateFutureOption;
      type: "interest_rate_future_option";
    }
  | {
      spec: CapFloor;
      type: "cap_floor";
    }
  | {
      spec: CmsSwap;
      type: "cms_swap";
    }
  | {
      spec: CmsOption;
      type: "cms_option";
    }
  | {
      spec: Deposit;
      type: "deposit";
    }
  | {
      spec: Repo;
      type: "repo";
    }
  | {
      spec: CreditDefaultSwap;
      type: "credit_default_swap";
    }
  | {
      spec: CdsIndex;
      type: "cds_index";
    }
  | {
      spec: CdsTranche;
      type: "cds_tranche";
    }
  | {
      spec: CdsOption;
      type: "cds_option";
    }
  | {
      spec: Equity;
      type: "equity";
    }
  | {
      spec: EquityOption;
      type: "equity_option";
    }
  | {
      spec: AsianOption;
      type: "asian_option";
    }
  | {
      spec: BarrierOption;
      type: "barrier_option";
    }
  | {
      spec: LookbackOption;
      type: "lookback_option";
    }
  | {
      spec: VarianceSwap;
      type: "variance_swap";
    }
  | {
      spec: VolatilityIndexFuture;
      type: "volatility_index_future";
    }
  | {
      spec: VolatilityIndexFutureOption;
      type: "volatility_index_future_option";
    }
  | {
      spec: FxSpot;
      type: "fx_spot";
    }
  | {
      spec: FxSwap;
      type: "fx_swap";
    }
  | {
      spec: FxForward;
      type: "fx_forward";
    }
  | {
      spec: Ndf;
      type: "ndf";
    }
  | {
      spec: FxOption;
      type: "fx_option";
    }
  | {
      spec: FxDigitalOption;
      type: "fx_digital_option";
    }
  | {
      spec: FxTouchOption;
      type: "fx_touch_option";
    }
  | {
      spec: FxBarrierOption;
      type: "fx_barrier_option";
    }
  | {
      spec: FxVarianceSwap;
      type: "fx_variance_swap";
    }
  | {
      spec: QuantoOption;
      type: "quanto_option";
    }
  | {
      spec: CommodityOption;
      type: "commodity_option";
    }
  | {
      spec: CommodityAsianOption;
      type: "commodity_asian_option";
    }
  | {
      spec: CommodityForward;
      type: "commodity_forward";
    }
  | {
      spec: CommoditySwap;
      type: "commodity_swap";
    }
  | {
      spec: CommoditySwaption;
      type: "commodity_swaption";
    }
  | {
      spec: CommoditySpreadOption;
      type: "commodity_spread_option";
    }
  | {
      spec: CommodityFuture;
      type: "commodity_future";
    }
  | {
      spec: CommodityFutureOption;
      type: "commodity_future_option";
    }
  | {
      spec: FxFuture;
      type: "fx_future";
    }
  | {
      spec: FxFutureOption;
      type: "fx_future_option";
    }
  | {
      spec: EquityFuture;
      type: "equity_future";
    }
  | {
      spec: EquityFutureOption;
      type: "equity_future_option";
    }
  | {
      spec: EquityTotalReturnFuture;
      type: "equity_total_return_future";
    }
  | {
      spec: Autocallable;
      type: "autocallable";
    }
  | {
      spec: CliquetOption;
      type: "cliquet_option";
    }
  | {
      spec: RangeAccrual;
      type: "range_accrual";
    }
  | {
      spec: Tarn;
      type: "tarn";
    }
  | {
      spec: Snowball;
      type: "snowball";
    }
  | {
      spec: CmsSpreadOption;
      type: "cms_spread_option";
    }
  | {
      spec: EquityTotalReturnSwap;
      type: "trs_equity";
    }
  | {
      spec: FiIndexTotalReturnSwap;
      type: "trs_fixed_income_index";
    }
  | {
      spec: Basket;
      type: "basket";
    }
  | {
      spec: PrivateMarketsFund;
      type: "private_markets_fund";
    }
  | {
      spec: RealEstateAsset;
      type: "real_estate_asset";
    }
  | {
      spec: DiscountedCashFlow;
      type: "discounted_cash_flow";
    }
  | {
      spec: CallableRangeAccrual;
      type: "callable_range_accrual";
    }
  | {
      spec: BondFuture;
      type: "bond_future";
    }
  | {
      spec: StructuredCredit;
      type: "structured_credit";
    }
  | {
      spec: AssetBackedFacility;
      type: "asset_backed_facility";
    }
  | {
      spec: LeveredRealEstateEquity;
      type: "levered_real_estate_equity";
    }
  | {
      spec: CompositeInstrument;
      type: "composite";
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
 * Deflation protection type
 */
export type DeflationProtection = "none" | "maturity_only" | "all_payments";
/**
 * Indexation method for inflation adjustment
 */
export type IndexationMethod = "canadian" | "tips" | "uk" | "french" | "japanese";
/**
 * Publication lag for inflation index reference dates.
 *
 * Inflation indices are published with a delay (typically 2-4 weeks). Securities
 * using these indices incorporate a lag to ensure the reference index is published
 * by the settlement date.
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
 * Notional exchange convention for XCCY swaps.
 */
export type NotionalExchange =
  | "none"
  | "final"
  | "initial_and_final"
  | {
      mtm_resetting: {
        /**
         * Which leg (`Leg1` or `Leg2`) has its notional reset each period.
         */
        resetting_side: ResettingSide;
        [k: string]: unknown;
      };
    };
/**
 * Identifies which leg of an XCCY swap has its notional reset under
 * MtM-resetting. `Leg1` and `Leg2` refer to `XccySwap::leg1` and `XccySwap::leg2`
 * respectively.
 */
export type ResettingSide = "leg1" | "leg2";
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
 * Type of interest rate option
 */
export type RateOptionType = "cap" | "floor" | "caplet" | "floorlet";
/**
 * Cash settlement annuity method for cash-settled swaptions.
 *
 * The trade confirmation or ISDA settlement matrix determines the method.
 * Modern EUR cash-settled swaptions use collateralized cash price; legacy
 * trades may retain par-yield or ISDA par-par terms.
 */
export type CashSettlementMethod = "collateralized_cash_price" | "par_yield" | "isda_par_par" | "zero_coupon";
/**
 * Exercise schedule convention for option models.
 */
export type ExerciseStyle = "european" | "american" | "bermudan";
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
 * Settlement type for options
 */
export type SettlementType = "physical" | "cash";
/**
 * Volatility convention (Black lognormal or Bachelier normal) used to price
 * a swaption. Wire values: `black`, `normal`.
 */
export type VolatilityModel = "black" | "normal";
/**
 * Co-terminal vs non-co-terminal Bermudan exercise.
 *
 * This distinction affects pricing methodology and calibration:
 * - Co-terminal: All exercise dates lead to the same swap end date
 * - Non-co-terminal: Each exercise date may have a different remaining swap tenor
 */
export type BermudanType = "co_terminal" | "non_co_terminal";
/**
 * Position direction for futures and forwards.
 */
export type Position = "long" | "short";
/**
 * Final settlement mode for a listed future.
 */
export type ListedFutureSettlement =
  | {
      type: "cash";
    }
  | {
      /**
       * Deliverable asset, grade, location, or basket identifier.
       */
      asset: string;
      /**
       * Physical units delivered per exchange contract.
       */
      quantity_per_contract: number;
      type: "physical";
    };
/**
 * Quotation model used for an option on a futures price.
 */
export type FutureOptionModel = "black76" | "normal";
/**
 * Premium-settlement convention.
 */
export type FutureOptionPremiumStyle = "premium_paid" | "futures_style";
/**
 * Settlement delivered by exercise or assignment.
 */
export type FutureOptionSettlement =
  | {
      /**
       * Date on which the fixed exercise payoff is paid.
       */
      payment_date: DateWire;
      type: "cash";
    }
  | {
      type: "future";
      /**
       * Last trading date of the delivered underlying future.
       */
      underlying_last_trading_date: DateWire;
      /**
       * Final settlement date of the delivered underlying future.
       */
      underlying_settlement_date: DateWire;
      /**
       * Official final settlement of the delivered future once trading has ended.
       */
      underlying_settlement_price?: number | null;
    };
/**
 * Whether a contractual spread is included in overnight daily compounding.
 *
 * ISDA-standard RFR coupons normally compound only the overnight index and add
 * any spread as simple interest after compounding. `Include` represents the
 * less common contract where the spread enters every daily compound factor.
 */
export type OvernightSpreadCompounding = "exclude" | "include";
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
 * Funding leg specification for a CMS swap.
 */
export type FundingLeg =
  | {
      /**
       * Accrual fractions for each period.
       */
      accrual_fractions: number[];
      /**
       * Day count convention.
       */
      day_count: DayCount;
      /**
       * Payment dates for each period.
       */
      payment_dates: DateWire[];
      /**
       * Fixed coupon rate as a decimal annual rate (0.03 = 3%).
       */
      rate: DecimalWire;
      type: "fixed";
    }
  | {
      /**
       * Accrual fractions for each period.
       */
      accrual_fractions: number[];
      /**
       * Day count convention.
       */
      day_count: DayCount;
      /**
       * Forward curve for floating rate projection.
       */
      forward_curve_id: Id;
      /**
       * Payment dates for each period. Each is also treated as the period's
       * accrual-end date (no payment lag — see the variant docs).
       */
      payment_dates: DateWire[];
      /**
       * Additive spread over the floating index in basis points (10 = 10bp).
       */
      spread_bp: DecimalWire;
      type: "floating";
    };
/**
 * Classification of collateral for repos.
 */
export type CollateralType =
  | "general"
  | {
      special: {
        /**
         * Optional special rate adjustment in basis points (negative = lower rate)
         */
        rate_adjustment_bp?: number | null;
      };
    };
/**
 * Repo margin type.
 *
 * Different margin mechanisms offer varying levels of protection
 * and operational complexity.
 */
export type RepoMarginType = "none" | "mark_to_market" | "net_exposure" | "triparty";
/**
 * Type of repurchase agreement.
 */
export type RepoType = "term" | "open" | "overnight";
/**
 * Regional ISDA CDS schedule family.
 *
 * Exact documentation clauses stay on the instrument or quote. This enum
 * identifies the regional schedule family used to resolve calendars,
 * day-count, frequency, stub, and settlement from the convention registry.
 */
export type CdsConvention = "isda_na" | "isda_eu" | "isda_as" | "custom";
/**
 * CDS market standard documentation clauses.
 *
 * Represents the ISDA documentation clause used for CDS contracts. Different clauses
 * define different restructuring events and settlement procedures. Used as part of
 * [`CdsConventionKey`] to look up CDS conventions.
 */
export type CdsDocClause =
  "cr14" | "mr14" | "mm14" | "xr14" | "isda_na" | "isda_eu" | "isda_as" | "isda_au" | "isda_nz" | "custom";
/**
 * Valuation presentation and pricing policy for CDS marks.
 *
 * Each variant bundles a coherent set of choices (premium-leg accrual schedule,
 * Act/360 accrual day counting, accrual-on-default bias, clean/dirty NPV,
 * par-spread denominator). Mixing those choices via separate boolean
 * overrides is intentionally not supported — the variants here are the only
 * conventions traded in practice.
 */
export type CdsValuationConvention =
  "isda_dirty" | "bloomberg_cdsw_clean" | "bloomberg_cdsw_clean_full_premium" | "quant_lib_isda_parity";
/**
 * Pricing mode for CDS indices.
 */
export type IndexPricing = "single_curve" | "constituents";
/**
 * Accrual-start convention for the synthetic underlying CDS used by CDSO.
 */
export type ProtectionStartConvention = "spot" | "forward";
/**
 * Typed CDS-option strike: forward-spread or clean-price convention.
 *
 * Wire format: externally tagged JSON — `{"spread": "0.0325"}` or
 * `{"clean_price_pct": "107.0"}` — with no bare-decimal fallback.
 */
export type CdsOptionStrike =
  | {
      spread: DecimalWire;
    }
  | {
      clean_price_pct: DecimalWire;
    };
/**
 * Lookback option type.
 */
export type LookbackType = "fixed_strike" | "floating_strike";
/**
 * Corporate-action treatment of historical equity price observations.
 */
export type EquityPriceSeriesPolicy = "adjusted" | "raw";
/**
 * Methods for calculating realized variance from price series.
 */
export type RealizedVarMethod = "close_to_close" | "parkinson" | "garman_klass" | "rogers_satchell" | "yang_zhang";
/**
 * Official NDF fixing source/benchmark.
 *
 * NDF settlements reference official fixing rates published by central banks
 * or designated fixing bodies. Using the correct fixing source is critical
 * for proper settlement calculations.
 *
 * # Market Standards
 *
 * | Currency | Fixing Source | Publisher | Settlement |
 * |----------|---------------|-----------|------------|
 * | CNY | PBOC | People's Bank of China | USD T+2 |
 * | CNH | CNHFIX | Treasury Markets Association (HK) | USD T+2 |
 * | INR | RBI | Reserve Bank of India | USD T+2 |
 * | KRW | KFTC | Korea Financial Telecommunications | USD T+1 |
 * | BRL | PTAX | Banco Central do Brasil | USD T+2 |
 * | TWD | TAIFX | Taipei Forex Inc. | USD T+2 |
 * | PHP | PHP BVAL | Bankers Association of the Philippines | USD T+1 |
 * | IDR | JISDOR | Bank Indonesia | USD T+2 |
 * | MYR | BNM | Bank Negara Malaysia | USD T+2 |
 */
export type NdfFixingSource =
  "pboc" | "cnhfix" | "rbi" | "kftc" | "ptax" | "taifx" | "php_bval" | "jisdor" | "bnm" | "other";
/**
 * Quote convention for NDF contract rates.
 *
 * NDFs can be quoted in two conventions depending on the market:
 *
 * # BasePerSettlement (default)
 *
 * Rate is quoted as units of base currency per one unit of settlement currency.
 * Example: USD/CNY = 7.25 means 7.25 CNY per 1 USD.
 *
 * Settlement formula:
 * ```text
 * Settlement = Notional_base × (1/F_fixing - 1/F_contract)
 * ```
 *
 * This is the standard convention for most Asian NDF markets (CNY, KRW, INR, etc.)
 * where the restricted currency is the base and USD is the settlement currency.
 *
 * # SettlementPerBase
 *
 * Rate is quoted as units of settlement currency per one unit of base currency.
 * Example: CNY/USD = 0.138 means 0.138 USD per 1 CNY.
 *
 * Settlement formula:
 * ```text
 * Settlement = Notional_base × (F_fixing - F_contract)
 * ```
 *
 * This is less common but may be used in some markets or for consistency with
 * other FX instruments that quote in this direction.
 */
export type NdfQuoteConvention = "base_per_settlement" | "settlement_per_base";
/**
 * Pair/venue FX delta quoting convention selected by the contract.
 */
export type FxDeltaConventionKind = "spot" | "forward" | "premium_adjusted_spot" | "premium_adjusted_forward";
/**
 * Payout type for digital (binary) options.
 */
export type DigitalPayoutType = "cash_or_nothing" | "asset_or_nothing";
/**
 * Touch type: one-touch (pays if barrier is hit) or no-touch (pays if not hit).
 */
export type TouchType = "one_touch" | "no_touch";
/**
 * Standard commodity market conventions by product type.
 *
 * Each variant provides market-standard defaults for settlement days,
 * business day convention, and calendar. Use when constructing commodity
 * forwards, options, or swaps without explicitly specifying these parameters.
 *
 * # Market Standards Reference
 *
 * | Convention | Settlement | BDC | Calendar | Exchange |
 * |------------|------------|-----|----------|----------|
 * | WTI Crude | T+2 | Following | NYMEX | NYMEX/CME |
 * | Brent Crude | T+2 | Following | ICE | ICE |
 * | Natural Gas | T+2 | Following | NYMEX | NYMEX/CME |
 * | Gold | T+2 | Modified Following | COMEX | CME |
 * | Silver | T+2 | Modified Following | COMEX | CME |
 * | Copper | T+2 | Following | LME | LME |
 * | Corn/Wheat | T+2 | Following | CBOT | CME |
 * | Power | T+1 | Modified Following | NERC | Various |
 *
 * # Sources
 *
 * - CME Group rulebooks
 * - ICE Futures exchange rules
 * - LME trading procedures
 */
export type CommodityConvention =
  "wti_crude" | "brent_crude" | "natural_gas" | "gold" | "silver" | "copper" | "agricultural" | "power";
/**
 * Final-settlement price fixing rule for a linear commodity future: which
 * observations set the official settlement price and how they are averaged.
 * The cash/physical delivery method lives in `terms.settlement`.
 */
export type CommodityFutureFixing =
  | {
      /**
       * Date whose forward or realized price determines final settlement.
       */
      observation_date: DateWire;
      /**
       * Official observed price once the observation date has passed.
       */
      realized_price?: number | null;
      type: "single";
    }
  | {
      /**
       * Ordered, unique exchange observation dates.
       */
      fixing_dates: DateWire[];
      /**
       * Official prices already fixed, keyed by observation date.
       */
      past_fixings?: [unknown, unknown][];
      type: "arithmetic_average";
    };
/**
 * Cliquet payoff aggregation type.
 */
export type CliquetPayoffType = "additive" | "multiplicative";
/**
 * Specifies how the range bounds are interpreted.
 */
export type BoundsType = "absolute" | "relative_to_initial_spot";
/**
 * Snowball note variant.
 */
export type SnowballVariant = "snowball" | "inverse_floater";
/**
 * Settlement timing for manufactured discrete dividends on an equity TRS.
 */
export type TrsDividendSettlement = "on_dividend_date" | "at_period_end";
/**
 * Type of fund event.
 */
export type FundEventKind = "contribution" | "distribution" | "proceeds";
/**
 * Catch-up mode for GP profit sharing.
 */
export type CatchUpMode = "full" | "partial";
/**
 * Clawback settlement trigger.
 */
export type ClawbackSettle = "fund_end" | "periodic";
/**
 * Waterfall allocation style.
 */
export type WaterfallStyle = "european" | "american";
/**
 * Individual tranche in the waterfall.
 */
export type PeFundWaterfallTranche =
  | "return_of_capital"
  | {
      preferred_irr: {
        /**
         * LP preferred-return hurdle as an annual decimal IRR (`0.08` = 8%),
         * compounded on the spec's `day_count`.
         */
        hurdle_irr: number;
      };
    }
  | {
      catch_up: {
        /**
         * GP share of each catch-up dollar, in `[0, 1]`. In
         * [`CatchUpMode::Partial`] this caps the GP's take per dollar; in
         * [`CatchUpMode::Full`] the GP takes 100% of catch-up dollars and
         * this value is only the fallback target share when no promote tier
         * follows.
         */
        gp_share: number;
      };
    }
  | {
      promote_tier: {
        /**
         * GP share of each split dollar, in `[0, 1]`; must sum to 1 with
         * `lp_share`.
         */
        gp_share: number;
        /**
         * Annual decimal IRR hurdle (`0.12` = 12%) the LP must reach (at
         * 100% payout) before this tier's split activates.
         */
        hurdle_irr: number;
        /**
         * LP share of each split dollar, in `[0, 1]`; must sum to 1 with
         * `gp_share`.
         */
        lp_share: number;
      };
    };
/**
 * Broad property classification for reporting / tagging.
 */
export type RealEstatePropertyType =
  "office" | "multifamily" | "retail" | "industrial" | "hospitality" | "mixed_use" | "other";
/**
 * Valuation method for a real estate asset.
 */
export type RealEstateValuationMethod = "dcf" | "direct_cap";
/**
 * Terminal value calculation method for DCF.
 */
export type TerminalValueSpec =
  | {
      /**
       * Perpetual stable growth rate as an annual decimal (e.g., 0.02 for
       * 2%). Must be < WACC.
       */
      stable_growth_rate: number;
      type: "gordon_growth";
      [k: string]: unknown;
    }
  | {
      /**
       * Multiple to apply (e.g., 10.0 for 10x EBITDA)
       */
      multiple: number;
      /**
       * Terminal metric value (e.g., EBITDA)
       */
      terminal_metric: number;
      type: "exit_multiple";
      [k: string]: unknown;
    }
  | {
      /**
       * Half-life of the growth fade in years (e.g., 5.0).
       */
      half_life_years: number;
      /**
       * Initial high growth rate (e.g., 0.15 for 15%).
       */
      high_growth_rate: number;
      /**
       * Long-run stable growth rate (e.g., 0.03 for 3%). Must be < WACC.
       */
      stable_growth_rate: number;
      type: "h_model";
      [k: string]: unknown;
    };
export type RepoDayCountWire = "act_360" | "act_365f";
/**
 * Which notes an assumed call redeems.
 */
export type CallScope =
  | "deal"
  | {
      tranche: string;
    };
/**
 * How defaulted collateral enters the OC numerator until its recovery cash
 * arrives.
 */
export type DefaultedValuation =
  | "recovery"
  | {
      market_value: {
        /**
         * Percent of defaulted par carried (`40.0` = 40%).
         */
        pct: number;
        [k: string]: unknown;
      };
    };
/**
 * What a failing coverage test does with the interest it diverts.
 */
export type CoverageTestAction = "pay_down_senior" | "reinvest";
/**
 * Type of coverage test (simplified to OC/IC only)
 */
export type CoverageTestType = "oc" | "ic" | "borrowing_base";
/**
 * Where the template places a coverage test tier.
 */
export type CoveragePlacement =
  | {
      kind: "after_tranche";
      /**
       * Tranche whose interest tier the test follows.
       */
      tranche_id: string;
    }
  | {
      kind: "after_junior_fees";
    };
/**
 * Notional a hedge swap's flows are scaled to each period.
 */
export type SwapNotional =
  | "contractual"
  | {
      tranche_par: string;
    }
  | "pool_par";
/**
 * Where a hedge swap's net payments rank in the waterfall.
 */
export type SwapPriority = "senior_fee" | "junior_fee";
/**
 * How collateral losses reach the note balances.
 *
 * Realized net loss (`default × (1 − recovery)`) is always tracked for the
 * cumulative-loss triggers; this policy decides whether it also reduces the
 * notes' outstanding principal before legal final.
 *
 * | Policy | Market | Note balances | Loss realized |
 * |---|---|---|---|
 * | `WriteDown` | RMBS, CMBS | reduced junior-first at each default | at default |
 * | `ParPreserving` | CLO, CBO, ABS, cards | carried at par; OC tests and the residual absorb losses | unpaid principal at legal final / liquidation |
 *
 * Under `ParPreserving` a subordinated note keeps accruing its full coupon
 * ahead of the residual holder; the OC numerator carries each defaulted
 * asset at its modeled recovery value until the recovery cash arrives.
 */
export type LossAllocationPolicy = "write_down" | "par_preserving";
/**
 * When a collateral loss is booked: at default (expected net loss, the
 * INTEX/Moody's Analytics convention for corporate collateral) or when the
 * defaulted loan liquidates and its recovery settles (mortgage servicing
 * convention, where the realized loss is known only at liquidation).
 *
 * The timing drives every cumulative-loss quantity: note write-downs under
 * [`LossAllocationPolicy::WriteDown`], the step-down and early-amortization
 * loss triggers and the excess-spread trap. The OC tests carry defaulted
 * collateral at its recovery value from the default date under both.
 */
export type LossRecognition = "at_default" | "at_liquidation";
/**
 * Consequences when triggers are breached
 */
export type TriggerConsequence =
  "divert_cash_flow" | "trap_excess_spread" | "accelerate_amortization" | "stop_reinvestment";
/**
 * Tranche seniority levels
 */
export type TrancheSeniority = "senior" | "mezzanine" | "subordinated" | "equity";
/**
 * Collection account(s) a waterfall tier draws on.
 *
 * Interest and principal proceeds are separate accounts in the executor.
 * A tier normally draws on the account matching its [`PaymentType`]; a
 * `Fee` or `Interest` tier may instead be allowed to top up from principal
 * proceeds (the CLO principal-waterfall convention that senior fees and
 * senior note interest shortfalls are paid from principal before any note
 * is redeemed). Cash taken from principal this way is reported as
 * [`WaterfallDistribution::principal_used_for_interest`].
 */
export type FundingSource = "interest" | "principal" | "interest_then_principal";
/**
 * Payment type classification
 */
export type PaymentType = "fee" | "interest" | "principal" | "residual" | "coverage_test";
/**
 * How to calculate payment amount
 */
export type PaymentCalculation =
  | {
      fixed_amount: {
        /**
         * Amount.
         */
        amount: Money;
        /**
         * Rounding convention.
         */
        rounding?: RoundingConvention | null;
      };
    }
  | {
      percentage_of_collateral: {
        /**
         * Annualized.
         */
        annualized: boolean;
        /**
         * Day count convention for annualization.
         */
        day_count?: DayCount | null;
        /**
         * Rate.
         */
        rate: number;
        /**
         * Rounding convention.
         */
        rounding?: RoundingConvention | null;
      };
    }
  | {
      percentage_of_special_serviced: {
        /**
         * Annualized.
         */
        annualized: boolean;
        /**
         * Day count convention for annualization.
         */
        day_count?: DayCount | null;
        /**
         * Rate.
         */
        rate: number;
        /**
         * Rounding convention.
         */
        rounding?: RoundingConvention | null;
      };
    }
  | {
      tranche_interest: {
        /**
         * Rounding convention.
         */
        rounding?: RoundingConvention | null;
        /**
         * Tranche id.
         */
        tranche_id: string;
      };
    }
  | {
      tranche_principal: {
        /**
         * Rounding convention.
         */
        rounding?: RoundingConvention | null;
        /**
         * Balance the regular principal pass amortizes the tranche down to;
         * `None` pays it in full. A coverage-test cure diversion at an earlier
         * position pays toward zero and counts toward this target, so the
         * regular pass only completes the remaining distance to it.
         */
        target_balance?: Money | null;
        /**
         * Tranche id.
         */
        tranche_id: string;
      };
    }
  | "residual_cash"
  | {
      reserve_replenishment: {
        /**
         * Target reserve balance the account should be replenished to.
         */
        target_balance: Money;
      };
    }
  | {
      capped_tranche_interest: {
        /**
         * Cap on the annualized coupon rate (decimal, e.g. `0.03` = 3%).
         */
        cap_rate: number;
        /**
         * Rounding convention.
         */
        rounding?: RoundingConvention | null;
        /**
         * Tranche id.
         */
        tranche_id: string;
      };
    }
  | {
      net_wac_carryover: {
        /**
         * Carryover balance outstanding at the period's opening.
         */
        amount: Money;
        /**
         * Tranche id.
         */
        tranche_id: string;
      };
    }
  | {
      incentive_fee: {
        /**
         * Equity IRR hurdle as an annual decimal.
         */
        hurdle_irr: number;
        /**
         * Share of the residual paid once the hurdle is met, in `[0, 1]`.
         */
        share: number;
      };
    };
/**
 * Rounding convention for payments
 */
export type RoundingConvention = "nearest" | "floor" | "ceiling";
/**
 * Recipient of waterfall payments
 */
export type RecipientType =
  | {
      service_provider: string;
    }
  | {
      manager_fee: ManagementFeeType;
    }
  | {
      tranche: string;
    }
  | "equity"
  | {
      reserve_account: string;
    };
/**
 * Type of management fee
 */
export type ManagementFeeType = "senior" | "subordinated" | "incentive";
/**
 * Target balance of the deal reserve account, re-evaluated every period.
 */
export type ReserveTarget =
  | {
      fixed: Money;
    }
  | {
      fraction_of_current: number;
    }
  | {
      fraction_of_original: number;
    }
  | {
      /**
       * @minItems 2
       * @maxItems 2
       */
      max: [unknown, unknown];
    };
/**
 * How a shifting-interest schedule value is read.
 */
export type ShiftMode = "shift_of_subordinate" | "senior_share";
/**
 * A single step-down performance trigger.
 *
 * Each variant is a per-period *health check* the deal must pass (in addition
 * to seasoning past the step-down date) for principal to switch to pro-rata.
 * All configured triggers must pass simultaneously; while any is breached the
 * deal reverts to sequential, so the switch is re-evaluated every period.
 *
 * Conventions (all evaluated on *current* balances each period):
 * - cumulative loss is a fraction of the *original* pool balance;
 * - the OC ratio is `current pool balance ÷ rated (non-equity) note balance`;
 * - credit enhancement is the senior cushion `(pool − senior note) ÷ pool`,
 *   where the senior note is the *single* most-senior tranche by payment
 *   priority — for pari-passu senior classes (e.g. A-1/A-2) only the
 *   lowest-priority one is taken as the reference.
 *
 * `MaxDelinquency` reads the delinquent share of the pool, which is zero
 * unless the deal carries a `credit_model.delinquency` model.
 */
export type StepDownTrigger =
  | {
      max_cumulative_loss: number;
    }
  | {
      min_oc_ratio: number;
    }
  | {
      min_credit_enhancement: number;
    }
  | {
      max_delinquency: number;
    };
/**
 * Financing instruments supported by levered real-estate valuation and metrics.
 */
export type RealEstateFinancing =
  | {
      spec: Bond;
      type: "bond";
    }
  | {
      spec: TermLoan;
      type: "term_loan";
    }
  | {
      spec: RevolvingCredit;
      type: "revolving_credit";
    }
  | {
      spec: Repo;
      type: "repo";
    };
/**
 * Rule controlling when dynamic quantities may be explicitly recalculated.
 */
export type RebalanceRule =
  | {
      kind: "manual";
    }
  | {
      /**
       * Strictly increasing dates on which the new state becomes eligible.
       */
      dates: DateWire[];
      kind: "dates";
    }
  | {
      /**
       * Business-day adjustment applied to each generated date.
       */
      business_day_convention: BusinessDayConvention;
      /**
       * Registered holiday-calendar identifier.
       */
      calendar_id: string;
      /**
       * Optional final unadjusted schedule date.
       */
      end?: DateWire | null;
      /**
       * Rebalance cadence as a tenor (for example `1D`, `1W`, `1M`, `3M`).
       */
      frequency: Tenor;
      kind: "calendar";
      /**
       * Unadjusted schedule start.
       */
      start: DateWire;
    };
/**
 * Policy used to resolve signed leg quantities at initialization or rebalance.
 *
 * Each variant consumes the signed `score` on [`CompositeLegSpec`] as either
 * the quantity itself or a target score. Resolution happens only in
 * [`CompositeSpec::initialize`] / [`CompositeSpec::initialize_fixed`] or
 * [`CompositeInstrument::rebalance`].
 *
 * [`CompositeSpec::initialize`]: crate::instruments::composite::CompositeSpec::initialize
 * [`CompositeSpec::initialize_fixed`]: crate::instruments::composite::CompositeSpec::initialize_fixed
 * [`CompositeInstrument::rebalance`]: crate::instruments::composite::CompositeInstrument::rebalance
 *
 * # Formulas
 *
 * Let `w_i` be the leg score, `G` a positive reporting-currency gross
 * notional, `N_i` the absolute unit notional, `m_i` the unit metric, `s_i`
 * the (optionally neutralized) score, and `σ_i` annualized unit-P&L
 * volatility. With an anchor quantity `q_a`:
 *
 * ```text
 * FixedQuantity:     q_i = w_i
 * NotionalWeighted:  q_i = sign(w_i) · G · |w_i| / Σ|w| / N_i
 * MetricWeighted:    q_i = (s_i / s_a) · (q_a · m_a) / m_i
 * VolatilityWeighted:q_i = (w_i / w_a) · q_a · σ_a / σ_i
 * ```
 *
 * Neutralization rescales positive scores to sum to `+1` and negative scores
 * to sum to `-1` before metric weighting. User-defined expressions replace
 * these closed forms and must return a finite non-zero quantity per leg.
 *
 * # References
 *
 * - DV01-neutral and duration-weighted curve trades:
 *   `docs/REFERENCES.md#tuckman-serrat-fixed-income`
 */
export type WeightingMethod =
  | {
      kind: "fixed_quantity";
    }
  | {
      /**
       * Positive gross notional in the composite reporting currency.
       */
      gross_notional: Money;
      kind: "notional_weighted";
    }
  | {
      /**
       * Leg whose resolved quantity fixes the overall scale.
       */
      anchor_leg_id: Id;
      /**
       * Non-zero signed quantity assigned to the anchor leg.
       */
      anchor_quantity: number;
      kind: "metric_weighted";
      /**
       * Metric used as the weighting measure, such as `dv01` or `delta`.
       */
      metric: MetricId;
      /**
       * Whether positive and negative scores are normalized to `+1` and `-1`.
       */
      neutralize: boolean;
    }
  | {
      /**
       * Leg whose resolved quantity fixes the overall scale.
       */
      anchor_leg_id: Id;
      /**
       * Non-zero signed quantity assigned to the anchor leg.
       */
      anchor_quantity: number;
      /**
       * Positive periods-per-year factor used to annualize sample volatility.
       */
      annualization_factor: number;
      kind: "volatility_weighted";
      /**
       * Maximum number of most-recent P&L observations used.
       */
      lookback: number;
      /**
       * Minimum P&L observations required for every active leg.
       */
      min_observations: number;
    }
  | {
      /**
       * Positive periods-per-year factor used to annualize the
       * `leg.{id}.volatility` column (for example `252.0` for daily history).
       */
      annualization_factor: number;
      kind: "user_defined";
      /**
       * One scalar expression per leg, keyed by instrument identifier.
       *
       * Available columns: `as_of_days`, `leg.{id}.score`,
       * `leg.{id}.value`, `leg.{id}.fx_rate`, optional `leg.{id}.notional`,
       * `leg.{id}.metric.{metric}` for each required metric, and
       * `leg.{id}.volatility` when history has at least three observations
       * (annualized with `sqrt(annualization_factor)`). `leg.{id}.notional`
       * is the signed reporting-currency notional.
       */
      quantity_expressions: {
        [k: string]: Expr;
      };
      /**
       * Metrics populated into the expression context for every leg.
       */
      required_metrics: MetricId[];
    };
/**
 * Strongly-typed metric identifier.
 *
 * Provides compile-time validation, autocomplete support, and safe refactoring
 * when metric names change. Covers bond, IRS, deposit, and risk metrics.
 *
 * See unit tests and `examples/` for usage.
 */
export type MetricId = string;
/**
 * The core expression node types.
 *
 * Deserialization is strict (`deny_unknown_fields`): unknown fields inside
 * struct variants (`CsRef`, `BinOp`, `UnaryOp`, `IfThenElse`) are rejected.
 */
export type ExprNode =
  | {
      column: string;
    }
  | {
      cs_ref: {
        /**
         * Capital-structure component.
         */
        component: string;
        /**
         * Instrument id or `total`.
         */
        instrument_or_total: string;
      };
    }
  | {
      literal: number;
    }
  | {
      /**
       * @minItems 2
       * @maxItems 2
       */
      call: [unknown, unknown];
    }
  | {
      bin_op: {
        /**
         * Left operand
         */
        left: Expr;
        /**
         * Operator
         */
        op: BinOp;
        /**
         * Right operand
         */
        right: Expr;
      };
    }
  | {
      unary_op: {
        /**
         * Operator
         */
        op: UnaryOp;
        /**
         * Operand
         */
        operand: Expr;
      };
    }
  | {
      if_then_else: {
        /**
         * Condition expression
         */
        condition: Expr;
        /**
         * Else branch
         */
        else_expr: Expr;
        /**
         * Then branch
         */
        then_expr: Expr;
      };
    };
/**
 * Binary operators for expressions.
 */
export type BinOp = "add" | "sub" | "mul" | "div" | "mod" | "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "and" | "or";
/**
 * Unary operators for expressions.
 */
export type UnaryOp = "neg" | "not";
/**
 * Type of asset in the basket
 */
export type BasketAssetType = "equity" | "bond" | "etf" | "cash" | "commodity" | "derivative";
/**
 * Standard FX conversion strategies used to hint FX providers.
 *
 * The policy tells a provider *how* the rate will be applied so it can decide
 * between spot, forward, or averaged sources.
 */
export type FxConversionPolicy = "cashflow_date" | "period_end" | "period_average";
/**
 * Classifies captured cashflows by economic meaning.
 *
 * These tags are diagnostic metadata only. They do not change pricing logic by
 * themselves.
 */
export type CashflowType =
  | "principal"
  | "interest"
  | "commitment_fee"
  | "usage_fee"
  | "facility_fee"
  | "upfront_fee"
  | "recovery"
  | "mark_to_market"
  | "other";
/**
 * Rounding modes supported by the library.
 *
 * The variants mirror the most common conventions found in pricing engines.
 */
export type RoundingMode = "bankers" | "away_from_zero" | "toward_zero" | "floor" | "ceil";
/**
 * Model-specific typed valuation details.
 *
 * These details are for rich structured outputs that do not fit the scalar
 * `measures` map while still belonging in the standard valuation envelope.
 */
export type ValuationDetails =
  | {
      data: CompositeValuationDetails;
      type: "composite";
      [k: string]: unknown;
    }
  | {
      data: CreditDerivativeValuationDetails;
      type: "credit_derivative";
      [k: string]: unknown;
    }
  | {
      data: StochasticPricingResult;
      type: "structured_credit_stochastic";
      [k: string]: unknown;
    }
  | {
      data: FxValuationDetails;
      type: "fx";
      [k: string]: unknown;
    }
  | {
      data: MonteCarloValuationDetails;
      type: "monte_carlo";
      [k: string]: unknown;
    };
/**
 * Pricing model selection for the pricer registry.
 *
 * Determines which mathematical model is used to price an instrument.
 * Each model has different computational characteristics and accuracy
 * profiles for different instrument types.
 *
 * # Model Categories
 *
 * ## Analytical Models
 * - [`Discounting`](Self::Discounting): Simple present value discounting
 * - [`Black76`](Self::Black76): Black-76 formula for options
 * - [`Normal`](Self::Normal): Bachelier (normal) model for rate options
 *
 * ## Tree Models
 * - [`Tree`](Self::Tree): Binomial/trinomial lattice
 * - [`HullWhite1F`](Self::HullWhite1F): Hull-White one-factor short rate
 *
 * ## Credit Models
 * - [`HazardRate`](Self::HazardRate): Fractional-recovery hazard-rate pricing
 * - [`RatesCredit`](Self::RatesCredit): Joint short-rate and hazard-rate pricing
 *
 * ## Monte Carlo Models
 * - [`MonteCarloGBM`](Self::MonteCarloGBM): GBM simulation
 * - [`MonteCarloHeston`](Self::MonteCarloHeston): Heston stochastic vol
 * - [`MonteCarloThreeFactor`](Self::MonteCarloThreeFactor): revolver
 *   utilization, short-rate and credit-spread simulation
 *
 * ## Exotic Analytical
 * - [`BarrierBSContinuous`](Self::BarrierBSContinuous): Reiner-Rubinstein barriers
 * - [`AsianGeometricBS`](Self::AsianGeometricBS): Geometric Asian (exact)
 * - [`AsianTurnbullWakeman`](Self::AsianTurnbullWakeman): Arithmetic Asian (approx)
 */
export type ModelKey =
  | "discounting"
  | "tree"
  | "black76"
  | "hull_white_1f"
  | "hazard_rate"
  | "rates_credit"
  | "normal"
  | "monte_carlo_gbm"
  | "monte_carlo_heston"
  | "monte_carlo_hull_white_1f"
  | "monte_carlo_three_factor"
  | "barrier_bs_continuous"
  | "asian_geometric_bs"
  | "asian_turnbull_wakeman"
  | "lookback_bs_continuous"
  | "quanto_bs"
  | "fx_barrier_bs_continuous"
  | "heston_fourier"
  | "merton_mc"
  | "monte_carlo_schwartz_smith"
  | "static_replication"
  | "lmm_monte_carlo"
  | "structured_credit_stochastic"
  | "bond_future_clean_price_proxy"
  | "monte_carlo_rough_bergomi"
  | "monte_carlo_rough_heston"
  | "rough_heston_fourier"
  | "pde_crank_nicolson_1d"
  | "pde_adi_2d"
  | "bloomberg_cdso";
/**
 * Pricing mode selection.
 *
 * Choose based on horizon × dimensionality: `Tree` for SHORT-horizon
 * non-recombining stochastic deals (deterministic, low variance),
 * `MonteCarlo` for long-horizon or high-dimensional pools, `Hybrid` to
 * front-load tree precision and tail with MC.
 *
 * # Tree mode is bounded by construction — read this before selecting it
 *
 * Path-preserving tree pricing keeps `3^n` terminal nodes for `n`
 * periods, checked against `max_tree_paths` (default 100,000). `3^11 =
 * 177,147`, so **Tree hard-errors for any deal with more than ten periods
 * remaining** — which is essentially every real deal, since
 * `build_scenario_tree_config` sets `num_periods` to months-to-maturity.
 *
 * The default is [`StructuredCreditPricingMode::MonteCarlo`] — the mode that can price the
 * deals this module is built for at realistic horizons (the public
 * `price_stochastic` entry point also selects Monte Carlo). Tree remains
 * available and correct for genuinely short horizons; select it explicitly.
 *
 * Test coverage:
 * - **Tree**: `tests/instruments/structured_credit/unit/{stochastic_pricing_tests,stochastic_tranche_pv_tests}`, at horizons within the node bound.
 * - **MonteCarlo**: the same suites plus the convergence tests.
 * - **Hybrid**: structured-credit pricer integration tests.
 */
export type StructuredCreditPricingMode =
  | "tree"
  | {
      monte_carlo: {
        /**
         * Pair each estimator's path with its sign-flipped mirror.
         */
        antithetic: boolean;
        /**
         * Number of independent estimators. With `antithetic` each estimator
         * simulates a `(Z, -Z)` pair, so the engine prices `2 × num_paths`
         * scenario paths.
         */
        num_paths: number;
      };
    }
  | {
      hybrid: {
        /**
         * Monte Carlo continuation paths per tree prefix
         */
        num_paths: number;
        /**
         * Tree periods before switching to MC
         */
        tree_periods: number;
      };
    };
/**
 * Domain-specific trace entry types.
 *
 * Each variant captures relevant details for different types of computations:
 * - Calibration: iteration details, convergence status
 * - Pricing: cashflow-level PV breakdowns
 * - Waterfall: step-by-step payment allocations
 */
export type TraceEntry =
  | {
      /**
       * Whether convergence was achieved
       */
      converged: boolean;
      /**
       * Iteration number (0-based)
       */
      iteration: number;
      kind: "calibration_iteration";
      /**
       * Knot points that were updated
       */
      knots_updated: string[];
      /**
       * Objective function residual
       */
      residual: number;
    }
  | {
      /**
       * Cashflow amount (stored as f64 for JSON simplicity)
       */
      cashflow_amount: number;
      /**
       * Cashflow currency
       */
      cashflow_currency: string;
      /**
       * Discount curve ID used
       */
      curve_id: string;
      /**
       * Cashflow payment date (ISO8601)
       */
      date: DateWire;
      /**
       * Discount factor applied
       */
      discount_factor: number;
      kind: "cashflow_pv";
      /**
       * Present value of this cashflow
       */
      pv_amount: number;
      /**
       * PV currency
       */
      pv_currency: string;
    }
  | {
      /**
       * Cash inflow amount
       */
      cash_in_amount: number;
      /**
       * Cash inflow currency
       */
      cash_in_currency: string;
      /**
       * Cash outflow amount
       */
      cash_out_amount: number;
      /**
       * Cash outflow currency
       */
      cash_out_currency: string;
      kind: "waterfall_step";
      /**
       * Period index
       */
      period: number;
      /**
       * Shortfall amount if any
       */
      shortfall_amount?: number | null;
      /**
       * Shortfall currency
       */
      shortfall_currency?: string | null;
      /**
       * Step name/description
       */
      step_name: string;
    }
  | {
      /**
       * Step description
       */
      description: string;
      kind: "computation_step";
      /**
       * Arbitrary metadata (JSON object)
       */
      metadata?: {
        [k: string]: unknown;
      };
      /**
       * Step name
       */
      name: string;
    };
/**
 * Numeric schema revision for contracts whose sole supported revision is v1.
 */
export type SchemaVersion = number;
/**
 * Canonical schema marker for persisted instrument envelopes.
 */
export type InstrumentSchema = "finstack_quant.instrument/1";
/**
 * Stage of persisted artifact loading that produced a diagnostic.
 */
export type LoadPhase = "parse" | "version" | "structure" | "semantic" | "canonicalize" | "hash" | "build";
/**
 * Severity assigned to a persisted artifact diagnostic.
 */
export type Severity = "error" | "warning";
/**
 * Records how a captured dataset was selected from the full simulation.
 */
export type PathSamplingMethod =
  | "all"
  | {
      random_sample: {
        /**
         * Target number of paths to capture on average.
         */
        count: number;
        /**
         * Seed used by the deterministic sampling rule.
         */
        seed: number;
      };
    };
/**
 * Built-in function identifiers.
 */
export type Function =
  | "lag"
  | "lead"
  | "diff"
  | "pct_change"
  | "cum_sum"
  | "cum_prod"
  | "cum_min"
  | "cum_max"
  | "rolling_mean"
  | "rolling_sum"
  | "ewm_mean"
  | "std"
  | "var"
  | "median"
  | "rolling_std"
  | "rolling_var"
  | "rolling_median"
  | "shift"
  | "rank"
  | "quantile"
  | "rolling_min"
  | "rolling_max"
  | "rolling_count"
  | "ewm_std"
  | "ewm_var"
  | "sum"
  | "mean"
  | "annualize"
  | "annualize_rate"
  | "ttm"
  | "ytd"
  | "qtd"
  | "fiscal_ytd"
  | "coalesce"
  | "min"
  | "max"
  | "abs"
  | "sign"
  | "growth_rate"
  | "pow"
  | "round"
  | "floor"
  | "ceil"
  | "ln"
  | "exp"
  | "log10"
  | "sqrt"
  | "clamp"
  | "is_missing";
/**
 * Strongly-typed instrument classification for pricer dispatch.
 *
 * Each variant represents a distinct instrument type with its own pricing
 * logic and risk characteristics. Used by the pricing registry to route
 * instruments to appropriate pricer implementations.
 */
export type InstrumentType =
  | "bond"
  | "credit_default_swap"
  | "cds_index"
  | "cds_tranche"
  | "cds_option"
  | "interest_rate_swap"
  | "cap_floor"
  | "swaption"
  | "bermudan_swaption"
  | "basis_swap"
  | "basket"
  | "convertible_bond"
  | "deposit"
  | "equity_option"
  | "fx_option"
  | "fx_spot"
  | "fx_swap"
  | "xccy_swap"
  | "inflation_linked_bond"
  | "inflation_swap"
  | "yoy_inflation_swap"
  | "inflation_cap_floor"
  | "interest_rate_future"
  | "variance_swap"
  | "fx_variance_swap"
  | "equity"
  | "repo"
  | "forward_rate_agreement"
  | "structured_credit"
  | "private_markets_fund"
  | "revolving_credit"
  | "asian_option"
  | "barrier_option"
  | "lookback_option"
  | "quanto_option"
  | "autocallable"
  | "cms_option"
  | "cms_swap"
  | "cliquet_option"
  | "range_accrual"
  | "fx_barrier_option"
  | "term_loan"
  | "discounted_cash_flow"
  | "real_estate_asset"
  | "levered_real_estate_equity"
  | "trs_equity"
  | "trs_fixed_income_index"
  | "bond_future"
  | "commodity_forward"
  | "commodity_swap"
  | "commodity_option"
  | "commodity_asian_option"
  | "commodity_swaption"
  | "commodity_spread_option"
  | "volatility_index_future"
  | "fx_forward"
  | "ndf"
  | "agency_mbs_passthrough"
  | "agency_tba"
  | "dollar_roll"
  | "agency_cmo"
  | "fx_digital_option"
  | "fx_touch_option"
  | "tarn"
  | "cms_spread_option"
  | "callable_range_accrual"
  | "snowball"
  | "composite"
  | "commodity_future"
  | "fx_future"
  | "equity_future"
  | "equity_total_return_future"
  | "interest_rate_future_option"
  | "equity_future_option"
  | "fx_future_option"
  | "commodity_future_option"
  | "volatility_index_future_option"
  | "asset_backed_facility";
/**
 * Rule for deriving an interest-rate future's reference period from its expiry.
 */
export type IrFutureReferencePeriod =
  "forward_starting" | "imm_quarter_in_arrears" | "calendar_month_in_arrears" | "business_month_in_arrears";
/**
 * Readiness of the mapped valuation route.
 */
export type ListedCoverageStatus = "native" | "composed" | "partial";
/**
 * Supported exchange catalog.
 */
export type ListedExchange = "cme" | "eurex" | "montreal" | "sgx";
/**
 * High-level listed product form.
 */
export type ListedProductKind = "future" | "option_on_future" | "option";
/**
 * Risk factor categories for VaR calculation.
 *
 * Each risk factor represents a market variable that can shift and impact
 * portfolio valuations. Risk factors are bucketed at standard tenors/strikes
 * to enable historical simulation.
 */
export type RiskFactorType =
  | {
      /**
       * Curve identifier
       */
      curve_id: Id;
      /**
       * Tenor in years (e.g., 1.0, 5.0, 10.0)
       */
      tenor_years: number;
      type: "discount_rate";
      [k: string]: unknown;
    }
  | {
      /**
       * Curve identifier
       */
      curve_id: Id;
      /**
       * Tenor in years
       */
      tenor_years: number;
      type: "forward_rate";
      [k: string]: unknown;
    }
  | {
      /**
       * Curve identifier
       */
      curve_id: Id;
      /**
       * Tenor in years
       */
      tenor_years: number;
      type: "credit_spread";
      [k: string]: unknown;
    }
  | {
      /**
       * Equity ticker or identifier
       */
      ticker: string;
      type: "equity_spot";
      [k: string]: unknown;
    }
  | {
      /**
       * Base currency of the FX pair.
       */
      base: Currency;
      /**
       * Quote currency of the FX pair.
       */
      quote: Currency;
      type: "fx_spot";
      [k: string]: unknown;
    }
  | {
      /**
       * Expiry in years
       */
      expiry_years: number;
      /**
       * Strike level (absolute or moneyness)
       */
      strike: number;
      type: "implied_vol";
      /**
       * Volatility surface identifier
       */
      vol_surface_id: Id;
      [k: string]: unknown;
    };
/**
 * Unit family of a metric value.
 *
 * The classification follows the unit documented on each `MetricId`
 * constant. Currency-per-bump sensitivities (`dv01`, `cs01`, `vega`) are
 * reported as [`MetricUnit::Currency`]: the value is a currency amount and
 * the bump is part of the metric's definition, not of its unit.
 */
export type MetricUnit = "currency" | "decimal" | "basis_points" | "years" | "percent" | "dimensionless" | "unknown";
/**
 * Type of rate index for convention determination.
 *
 * Distinguishes between overnight risk-free rate (RFR) indices and term indices, which have
 * different conventions for compounding, payment frequencies, and reset lags.
 */
export type RateIndexKind = "overnight_rfr" | "term";

/**
 * Advance rate applied to one collateral class.
 *
 * `asset_class` is the wire name of the [`AssetType`](super::AssetType)
 * variant (`"first_lien_loan"`, `"new_auto_loan"`, ...) or `"*"` for every
 * class without a more specific entry; collateral with no matching entry is
 * ineligible.
 */
export interface AdvanceRate {
  /**
   * Collateral class the rate applies to (`AssetType` wire name or `"*"`).
   */
  asset_class: string;
  /**
   * Eligibility criteria the collateral must meet to count.
   */
  eligibility?: EligibilityRule;
  /**
   * Advance rate as a decimal share of eligible balance (`0.8` = 80%).
   */
  rate: number;
}
/**
 * Eligibility criteria for collateral under an advance rate.
 */
export interface EligibilityRule {
  /**
   * Defaulted collateral is ineligible (the default).
   */
  exclude_defaulted?: boolean;
  /**
   * Non-performing rows (an unresolved `liquidation` timeline) are
   * ineligible (the default); the re-performing share becomes eligible
   * once the timeline resolves.
   */
  exclude_non_performing?: boolean;
  /**
   * Delinquent balance more than this many days past due is ineligible
   * (delinquency buckets are 30 days each, so `60` keeps the 30- and
   * 60-day buckets and drops the rest). `None` keeps every delinquent
   * balance eligible.
   */
  max_days_past_due?: number | null;
  /**
   * Collateral maturing after this date is ineligible.
   */
  max_maturity?: DateWire | null;
}
/**
 * Available-funds cap (net-WAC cap) specification.
 */
export interface AfcSpec {
  /**
   * Ids of tranches whose interest coupon is capped at the collateral's
   * (net) weighted-average coupon.
   */
  capped_tranches: string[];
  /**
   * Net-WAC carryover: the interest the cap withholds from each capped
   * tranche accrues as a carryover balance (no interest on it) repaid from
   * excess interest through a `net_wac_carryover` tier ahead of the
   * incentive fee and the residual. Off by default: the capped-off coupon
   * is then simply never owed.
   */
  carryover?: boolean;
  /**
   * Senior fee load (annualized basis points) ranking ahead of the capped
   * interest — typically servicing plus trustee fees. Subtracted from the
   * gross collateral WAC to form the **net**-WAC cap. When `None` the cap is
   * the gross collateral WAC.
   */
  net_wac_fee_bp?: number | null;
}
/**
 * Agency CMO instrument.
 *
 * Represents a CMO deal backed by agency MBS collateral with multiple
 * tranches that receive cashflows according to waterfall rules.
 */
export interface AgencyCmo {
  /**
   * Agency program
   */
  agency: AgencyProgram;
  /**
   * Attributes for tagging and selection.
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Collateral pool (optional - for detailed cashflow projection)
   */
  collateral?: AgencyMbsPassthrough | null;
  /**
   * Collateral WAC (if no explicit collateral)
   */
  collateral_wac?: number | null;
  /**
   * Remaining collateral WAM in months (if no explicit collateral).
   */
  collateral_wam_months?: number | null;
  /**
   * Deal name (e.g., "FNR 2024-1")
   */
  deal_name: Id;
  /**
   * Discount curve identifier.
   */
  discount_curve_id: Id;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Issue date
   */
  issue_date: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Reference tranche ID for pricing (which tranche to value)
   */
  reference_tranche_id: string;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Waterfall configuration with tranches
   */
  waterfall: CmoWaterfall;
}
/**
 * User-defined tags and key-value metadata for classification.
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
 * Agency MBS passthrough instrument (pool or specified pool).
 *
 * Represents an agency mortgage-backed security where principal and interest
 * payments from the underlying mortgage pool are passed through to investors,
 * net of servicing and guarantee fees.
 *
 * # Cashflow Sign Convention
 *
 * All cashflows are from the holder's (investor's) perspective:
 * - Principal and interest received are positive
 * - The initial purchase price is handled at trade level
 *
 * # Payment Delay
 *
 * Agency MBS have standardized payment delays measured from the **start**
 * of the accrual period (first day of the month) to the payment date:
 * - FNMA / FHLMC (UMBS): ~55 days → payment on the 25th of M+1
 * - GNMA I: ~45 days → payment on the 15th of M+1
 * - GNMA II: ~50 days → payment on the 20th of M+1
 */
export interface AgencyMbsPassthrough {
  /**
   * Agency program (FNMA, FHLMC, GNMA).
   */
  agency: AgencyProgram;
  /**
   * Attributes for scenario selection and tagging.
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Net pass-through coupon paid to the investor, as an annual decimal
   * (`0.04` = 4%): `wac` less the servicing and guarantee fees.
   */
  coupon: number;
  /**
   * Current face amount (remaining principal balance).
   */
  current_face: Money;
  /**
   * Day count convention for accrual.
   */
  day_count: DayCount;
  /**
   * Discount curve identifier for pricing.
   */
  discount_curve_id: Id;
  /**
   * Annual agency guarantee fee (g-fee) in basis points (`25.0` = 0.25%).
   *
   * Defaults to `0.0` when omitted.
   */
  guarantee_fee_bp?: number;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Issue date of the pool.
   */
  issue_date: DateWire;
  /**
   * End date of the latest accrual period whose delayed P&I payment has
   * settled and is already reflected in `current_face`.
   *
   * When omitted, pricing infers the latest paid period from the agency
   * payment-delay rule and `as_of`.
   */
  last_paid_accrual_end?: DateWire | null;
  /**
   * Legal maturity date.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Original face amount (initial principal balance).
   */
  original_face: Money;
  /**
   * Pool identifier (CUSIP or internal pool ID).
   */
  pool_id: Id;
  /**
   * Pool type (generic or specified).
   */
  pool_type?: PoolType;
  /**
   * Prepayment model specification.
   */
  prepayment_spec: cashflows.PrepaymentModelSpec;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Annual servicing fee in basis points (`25.0` = 0.25%).
   *
   * Defaults to `0.0` when omitted.
   */
  servicing_fee_bp?: number;
  /**
   * Optional custom stated payment delay in days (overrides the agency
   * rule). A delay `D` pays on day `D − 30k` of the month `k = (D − 1)/30`
   * months after the accrual month, rolled Following on the `usny`
   * calendar (55 → 25th of the next month, 75 → 15th two months later).
   * Must be at least 1.
   */
  stated_delay_days?: number | null;
  /**
   * Weighted average coupon (gross rate on underlying mortgages).
   */
  wac: number;
  /**
   * Remaining weighted average maturity in months as of the valuation
   * date (current WAM, not the original term). Pool age (WALA) for
   * seasoning ramps is derived separately from `issue_date`.
   */
  wam_months: number;
}
/**
 * Currency-tagged monetary amount.
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
 * Instrument-owned market quote and model configuration overrides.
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
 * Metric-time pricing and finite-difference configuration.
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
 * Scenario-only price and spread shocks.
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
 * CMO waterfall configuration.
 */
export interface CmoWaterfall {
  /**
   * Whether to use pro-rata allocation within same priority
   */
  pro_rata_same_priority: boolean;
  /**
   * Tranches in the deal (ordered by priority for sequential)
   */
  tranches: CmoTranche[];
}
/**
 * CMO tranche definition.
 */
export interface CmoTranche {
  /**
   * Coupon rate (0.0 for PO)
   */
  coupon: number;
  /**
   * Current face amount
   */
  current_face: Money;
  /**
   * Tranche identifier (e.g., "A", "B", "IO")
   */
  id: string;
  /**
   * Original face amount
   */
  original_face: Money;
  /**
   * PAC collar (if PAC tranche)
   */
  pac_collar?: PacCollar | null;
  /**
   * Payment priority (1 = highest for sequential)
   */
  priority: number;
  /**
   * Tranche type
   */
  tranche_type: CmoTrancheType;
}
/**
 * PAC collar boundaries.
 */
export interface PacCollar {
  /**
   * Lower collar speed as a multiple of the standard PSA curve (`1.0` =
   * 100% PSA).
   */
  lower_speed_multiplier: number;
  /**
   * Upper collar speed as a multiple of the standard PSA curve (`3.0` =
   * 300% PSA).
   */
  upper_speed_multiplier: number;
}
/**
 * TBA (To-Be-Announced) forward trade.
 *
 * Represents a forward contract to buy or sell agency MBS at a specified
 * price for a future settlement date. The specific pools delivered are
 * not known at trade time.
 *
 * # Good Delivery Standards
 *
 * TBA trades must meet SIFMA good delivery guidelines:
 * - Pool must be from the specified agency program
 * - Pool coupon must match the TBA coupon
 * - Pool term must match the TBA term
 * - Variance rules for face amount (±0.01% of trade amount)
 *
 * # Pricing
 *
 * TBA value is calculated as the difference between the forward value
 * of assumed pool characteristics and the trade price, discounted to
 * the valuation date.
 */
export interface AgencyTba {
  /**
   * Agency program (FNMA, FHLMC, GNMA).
   */
  agency: AgencyProgram;
  /**
   * Optional assumed pool for valuation.
   * Its current and original faces are scaled together to the trade's
   * purchased current face. If absent, generic pool characteristics apply.
   */
  assumed_pool?: AgencyMbsPassthrough | null;
  /**
   * Attributes for tagging and selection.
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Pass-through coupon rate (e.g., 0.04 for 4%).
   */
  coupon: number;
  /**
   * Discount curve identifier.
   */
  discount_curve_id: Id;
  /**
   * Unique instrument identifier.
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
   * Current face amount purchased at settlement, in the trade currency.
   */
  notional: Money;
  /**
   * Current/original face ratio in `(0, 1]` for the assumed delivered pool.
   * Defaults to 1.0 for a generic pool. Original face is `notional / factor`;
   * the purchased current face remains `notional`. When an explicit pool is
   * supplied, its factor is used and any specified factor must agree.
   */
  pool_factor?: number | null;
  /**
   * Prepayment model of the generic assumed pool.
   *
   * Applies only when `assumed_pool` is absent (an explicit pool carries its
   * own model); `None` uses the embedded generic PSA assumption. Setting it
   * together with `assumed_pool` is rejected.
   */
  prepayment_spec?: cashflows.PrepaymentModelSpec | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * SIFMA settlement class override.
   *
   * When `None`, inferred from agency + term using
   * [`SifmaSettlementClass::from_agency_term`].
   */
  settlement_class?: SifmaSettlementClass | null;
  /**
   * Explicit settlement date override.
   *
   * When set, bypasses the SIFMA calendar lookup for
   * `settlement_year`/`settlement_month` in
   * [`AgencyTba::effective_settlement_date`]. Dollar rolls use this to keep leg
   * pricing consistent with explicit roll settlement dates.
   */
  settlement_date?: DateWire | null;
  /**
   * Settlement month (1-12).
   */
  settlement_month: number;
  /**
   * Settlement year.
   */
  settlement_year: number;
  /**
   * Original loan term.
   */
  term: TbaTerm;
  /**
   * Trade date.
   */
  trade_date?: DateWire | null;
  /**
   * Trade price (percentage of par, e.g., 98.5).
   */
  trade_price: number;
}
/**
 * Asian option instrument.
 *
 * Asian options depend on the average price over a period rather than
 * just the terminal price. Supports both call and put options with
 * arithmetic or geometric averaging.
 *
 * # Averaging Methods
 *
 * - **Arithmetic**: `A = (1/n) × Σ S(t_i)` - Market standard, approximated via Turnbull-Wakeman
 * - **Geometric**: `G = [Π S(t_i)]^(1/n)` - Closed-form solution available (Kemna-Vorst)
 *
 * # Fixing Dates and Business Day Conventions
 *
 * **Important**: The `fixing_dates` field expects dates that have already been adjusted
 * for business day conventions. In production use, callers should:
 *
 * 1. Generate the schedule of observation dates (e.g., monthly end-of-month)
 * 2. Apply the appropriate business day convention (typically Modified Following)
 * 3. Adjust for the relevant holiday calendar (based on underlying asset's market)
 *
 * Common conventions by market:
 * - **US Equity (SPX)**: NYSE calendar, Modified Following
 * - **FX Options**: Joint calendar of currency pair, Modified Following
 * - **Commodities**: Exchange-specific calendar
 *
 * The number of fixing dates directly affects the averaging calculation and pricing.
 * Typical configurations:
 * - Daily averaging: ~252 dates per year (trading days)
 * - Weekly averaging: ~52 dates per year
 * - Monthly averaging: 12 dates per year (typically month-end)
 *
 * # Pricing Models
 *
 * | Averaging | Model | Accuracy |
 * |-----------|-------|----------|
 * | Geometric | Kemna-Vorst (1990) | Exact closed-form |
 * | Arithmetic | Turnbull-Wakeman (1991) | ~1% vs Monte Carlo |
 * | Either | Monte Carlo | Configurable accuracy |
 */
export interface AsianOption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Averaging method (arithmetic or geometric)
   */
  averaging_method: AveragingMethod;
  /**
   * Currency of the strike, premium and present value.
   */
  currency: Currency;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Discount curve ID for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Optional dividend-yield scalar ID
   */
  div_yield_id?: Id | null;
  /**
   * Option expiry date
   */
  expiry: DateWire;
  /**
   * Dates on which underlying is observed for averaging.
   *
   * **Note**: These dates should be pre-adjusted for business day conventions.
   * The pricer uses these dates directly without further adjustment.
   * See struct-level documentation for business day convention guidance.
   */
  fixing_dates: DateWire[];
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
   * Option type (call or put)
   */
  option_type: OptionType;
  /**
   * Past fixings for seasoned options (date, observed price pairs).
   *
   * For seasoned options where some averaging observations have already occurred,
   * provide the historical fixings here. Only fixings that match dates in
   * `fixing_dates` and are on or before the valuation date are considered.
   */
  past_fixings?: [unknown, unknown][];
  /**
   * Number of underlying units the option is written on; PV and Greeks scale linearly with it.
   */
  quantity: number;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Spot price identifier
   */
  spot_id: Id;
  /**
   * Strike price
   */
  strike: number;
  /**
   * Underlying asset ticker symbol
   */
  underlying_ticker: string;
  /**
   * Volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * Committed asset-backed facility (warehouse line) against a collateral pool.
 *
 * The facility lends `drawn` of a `commitment` against `collateral`; the
 * borrowing base is the advance-rate-weighted eligible collateral after the
 * concentration limits. Each period the structured-credit engine runs a
 * synthetic two-class deal (the facility note and the residual): a
 * borrowing-base deficiency is a mandatory repayment ahead of the residual,
 * collateral principal recycles into new collateral while revolving and
 * repays the facility sequentially afterwards, the undrawn commitment
 * accrues `commitment_fee_bp`, and the residual keeps what is left. See
 * [`Self::synthesized_deal`] for the exact mapping.
 *
 * Rates are decimals (`rate`) or basis points (`*_bp`); `*_pct` fields are
 * percent values (`20.0` = 20%) and loss thresholds are decimal fractions
 * (`max_cumulative_loss`).
 */
export interface AssetBackedFacility {
  /**
   * Events that end revolving early.
   */
  amortization_events?: AmortizationEvent[];
  /**
   * Free-form attributes (tags and metadata).
   */
  attributes?: Attributes;
  /**
   * Advance rates, eligibility and concentration limits that define the
   * borrowing base.
   */
  borrowing_base_rules: BorrowingBaseRules;
  /**
   * Holiday calendar for the payment schedule (e.g. `"nyse"`); required
   * for pricing.
   */
  calendar_id?: Id | null;
  /**
   * Optional credit-card master-trust portfolio model. Asset and rep-line
   * pools only: the pool is the investor interest in the receivables, the
   * spec's payment rate, portfolio yield and charge-off rate replace the
   * prepayment, coupon and default assumptions. See [`CardPortfolioSpec`].
   */
  card?: CardPortfolioSpec | null;
  /**
   * Closing date; the first payment date is one `frequency` later.
   */
  closing_date: DateWire;
  /**
   * Collateral pool the facility lends against (asset rows, rep lines or
   * instrument collateral).
   */
  collateral: AssetPool;
  /**
   * Total commitment in the collateral currency.
   */
  commitment: Money;
  /**
   * Commitment fee on the undrawn commitment, in basis points per annum
   * (`50` = 0.50%). Defaults to zero.
   */
  commitment_fee_bp?: DecimalWire;
  /**
   * Optional correlation structure for stochastic modeling.
   */
  correlation_structure?: CorrelationStructure | null;
  /**
   * Accrual convention of the facility interest and commitment fee.
   */
  day_count?: DayCount;
  /**
   * Default model specification.
   */
  default_spec?: cashflows.DefaultModelSpec;
  /**
   * Optional roll-rate delinquency model with servicer advancing and loan
   * modification. Asset and rep-line pools only: the default model then
   * feeds the first delinquency bucket and only the roll out of the last
   * bucket charges off. See [`DelinquencyModel`].
   */
  delinquency?: DelinquencyModel | null;
  /**
   * Discount curve for the facility and residual cashflows.
   */
  discount_curve_id: Id;
  /**
   * Amount drawn at closing, in the collateral currency; at most the
   * commitment and strictly below the collateral balance.
   */
  drawn: Money;
  /**
   * Scheduled draws after closing, ascending by date; each funds on the
   * first payment date at or after its date.
   */
  draws?: DrawEvent[];
  /**
   * Transaction fees paid through the synthetic deal's waterfall ahead of
   * the facility's interest (trustee, servicing, ...); `None` for none.
   */
  fees?: DealFees | null;
  /**
   * Payment frequency of interest, fees and the borrowing-base test.
   */
  frequency: Tenor;
  /**
   * Stable facility identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Price the collateral realizes when the term-out ends and the
   * remaining collateral is liquidated to repay the facility, as a percent
   * of par (`None` = par).
   */
  liquidation_price_pct?: number | null;
  /**
   * Legal final maturity of the facility.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Prepayment model specification.
   */
  prepayment_spec?: cashflows.PrepaymentModelSpec;
  /**
   * Facility coupon: a fixed all-in rate (decimal) or a floating index
   * plus spread.
   */
  rate: RateSpec;
  /**
   * Re-advance the facility each revolving period up to the commitment
   * and the borrowing base.
   */
  readvance_to_borrowing_base?: boolean;
  /**
   * Recovery model specification.
   */
  recovery_spec?: cashflows.RecoveryModelSpec;
  /**
   * Scheduled end of the revolving period.
   */
  revolving_end: DateWire;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Optional stochastic default model specification.
   */
  stochastic_default_spec?: StochasticDefaultSpec | null;
  /**
   * Optional stochastic prepayment model specification.
   */
  stochastic_prepay_spec?: StochasticPrepaySpec | null;
  /**
   * Optional stochastic recovery model for `price_stochastic`:
   * `MarketCorrelated` recoveries move with the systematic factor (and
   * disperse per name), so a path with heavy defaults also recovers
   * less. `None` keeps recoveries constant at `recovery_spec.rate`.
   */
  stochastic_recovery_spec?: RecoverySpec | null;
  /**
   * Term-out window after revolving; `None` lets the collateral run to
   * `maturity`.
   */
  term_out?: TermOutSpec | null;
}
/**
 * Advance rates and concentration limits that define a borrowing base.
 */
export interface BorrowingBaseRules {
  /**
   * Advance rate per collateral class (first match wins, `"*"` last).
   */
  advance_rates: AdvanceRate[];
  /**
   * Concentration limits applied to the eligible collateral before the
   * advance rates, in order.
   */
  concentration_limits?: ConcentrationLimit[];
}
/**
 * Cap on the eligible collateral any one obligor, industry or asset class
 * may contribute; balance above the cap is excluded from the borrowing base.
 */
export interface ConcentrationLimit {
  /**
   * Maximum share of the eligible collateral, in percent (`20.0` = 20%).
   */
  max_pct: number;
  /**
   * Dimension the cap is measured on.
   */
  scope: ConcentrationScope;
}
/**
 * Portfolio assumptions for a credit-card master trust.
 */
export interface CardPortfolioSpec {
  /**
   * Annual charge-off rate as a decimal of the receivables balance;
   * replaces the default model unless an asset carries `mdr_override`.
   */
  charge_off_rate: number;
  /**
   * Investor allocation of trust collections fixed at the end of the
   * revolving period, as a decimal in `(0, 1]`. `None` freezes the
   * allocation at the investor's floating share
   * `investor_interest / (investor_interest + seller_interest)`.
   */
  fixed_allocation_decimal?: number | null;
  /**
   * Share of the receivables balance paid down each month, as a decimal
   * (`0.15` = 15% monthly payment rate). Drives principal collections in
   * place of the prepayment model.
   */
  monthly_payment_rate: number;
  /**
   * Annual portfolio yield (finance charges, interchange and fees) as a
   * decimal of the receivables balance; replaces the assets' coupons.
   */
  portfolio_yield: number;
  /**
   * Seller's interest in the trust receivables at the deal's valuation
   * date, in the pool currency. The pool is the investor interest; the
   * trust receivables are the two together. `None` means no seller
   * interest.
   */
  seller_interest?: Money | null;
}
/**
 * Main asset pool structure
 */
export interface AssetPool {
  /**
   * Underlying assets
   */
  assets: PoolAsset[];
  /**
   * AssetPool-level accounts
   * Collection account balance (collected but not yet distributed)
   */
  collection_account: Money;
  /**
   * Performance tracking
   * Cumulative defaults to date
   */
  cumulative_defaults: Money;
  /**
   * Cumulative prepayments (voluntary early repayment)
   */
  cumulative_prepayments: Money;
  /**
   * Cumulative recoveries on defaulted assets
   */
  cumulative_recoveries: Money;
  /**
   * Cumulative scheduled amortization (level-pay principal for amortizing assets).
   *
   * Part of the original-balance denominator in
   * [`current_loss_percentage`](super::StructuredCredit::current_loss_percentage),
   * so it must be stated: omitting it understates the denominator and
   * overstates the reported loss rate.
   */
  cumulative_scheduled_amortization: Money;
  /**
   * Base currency for every asset and pool-level account.
   */
  currency: Currency;
  /**
   * Deal type classification
   */
  deal_type: DealType;
  /**
   * Excess spread account (accumulated excess interest)
   */
  excess_spread_account: Money;
  /**
   * AssetPool identifier
   */
  id: Id;
  /**
   * Real instruments held as collateral (bonds, term loans, revolvers).
   * Must be used with an empty `assets` vector and no `rep_lines`; the
   * simulation engine drives period flows from the instruments' own
   * schedules and materializes them into asset rows for every balance
   * consumer.
   */
  instruments?: InstrumentCollateral | null;
  /**
   * Original (cut-off) pool balance the deal's cumulative-loss triggers,
   * clean-up call factor and cumulative-loss/timing default curves are
   * stated against. `None` reconstructs it as the current balance plus
   * `cumulative_defaults`, `cumulative_prepayments` and
   * `cumulative_scheduled_amortization` (see
   * [`Self::original_balance_or_reconstructed`]); supply it when those
   * tallies are incomplete. Must be at least the current total balance.
   */
  original_balance?: Money | null;
  /**
   * Reinvestment management
   * Reinvestment period configuration (if applicable)
   */
  reinvestment_period?: ReinvestmentPeriod | null;
  /**
   * Aggregated representative lines (optional optimization)
   * Must be used with an empty `assets` vector; normalized into the same engine.
   */
  rep_lines?: RepLine[] | null;
  /**
   * Reserve account balance (for credit enhancement)
   */
  reserve_account: Money;
  /**
   * Annual simple interest rate (decimal, ACT/360 on the opening balance
   * per legal period) earned by `reserve_account`. Defaults to `0.0`.
   */
  reserve_account_rate?: number;
  /**
   * Where the interest earned on `reserve_account` is paid.
   */
  reserve_interest_destination?: ReserveInterestDestination;
  /**
   * Target reserve balance that revolver repayments replenish toward
   * before counting as principal collections. `None` disables
   * replenishment from repayments.
   */
  reserve_target?: Money | null;
}
/**
 * Individual asset held in a structured-credit collateral pool.
 *
 * Monetary fields use the asset's native currency. Rates are annual decimal
 * rates unless a field explicitly says basis points.
 */
export interface PoolAsset {
  /**
   * Date on which the pool acquired the asset, if known.
   */
  acquisition_date?: DateWire | null;
  /**
   * Amortization schedule length in months from origination
   * (`acquisition_date`, else the deal closing) for level-pay assets, e.g.
   * `360` for a 30-year schedule on a 10-year loan: the level payment is
   * sized over `term − age` and the unamortized balance pays as a balloon
   * at maturity. `None` amortizes fully by maturity. Ignored when
   * `contractual_payment` is set.
   */
  amortization_term_months?: number | null;
  /**
   * Economic asset classification used by pool-level assumptions.
   */
  asset_type: AssetType;
  /**
   * Current outstanding principal balance in the asset currency.
   */
  balance: Money;
  /**
   * Balloon maturity behavior (commercial mortgages): the share that does
   * not refinance at maturity is extended instead of paid.
   */
  balloon?: BalloonSpec | null;
  /**
   * Total commitment for revolving or delayed-draw collateral; `balance` is
   * the drawn part. `None` for fully funded assets.
   */
  commitment?: Money | null;
  /**
   * Contractual periodic payment for level-pay assets. Required for exact
   * seasoned-loan amortization; when absent it is inferred once from the
   * current state and the amortization term (or the remaining contractual
   * periods to maturity).
   */
  contractual_payment?: Money | null;
  /**
   * Day-count convention used for coupon and accrual calculations.
   */
  day_count: DayCount;
  /**
   * Economic default date for the outstanding recovery claim; required for defaulted assets.
   */
  default_date?: DateWire | null;
  /**
   * Whether the asset has defaulted (optional on the wire, default `false`).
   */
  defaulted?: boolean;
  /**
   * Delinquent balance per bucket (30, 60, 90 days past due, ...) at the
   * valuation date, in the pool's currency. Requires
   * `credit_model.delinquency` with the same number of buckets; the sum
   * must not exceed `balance`. `None` for a fully current asset.
   */
  delinquency_buckets?: Money[] | null;
  /**
   * Rates forward-curve identifier for floating-rate assets, such as SOFR-3M.
   */
  forward_curve_id?: string | null;
  /**
   * Stable identifier used to match the asset to diagnostics and scenarios.
   */
  id: Id;
  /**
   * Floor on the floating index in basis points (100 = 1%), applied before
   * `spread_bp` is added, matching `FloatingRateSpec::index_floor_bp`;
   * `None` for no floor. Ignored on fixed-rate rows.
   */
  index_floor_bp?: number | null;
  /**
   * Optional industry classification used by concentration checks.
   */
  industry?: string | null;
  /**
   * Interest-only period in months from origination: no scheduled
   * principal while the loan is younger than this, then the level payment
   * over the remaining schedule. `None` for no interest-only period.
   */
  io_months?: number | null;
  /**
   * Non-performing loan resolution: the loan pays nothing until the
   * resolution date, where a share liquidates (a default whose recovery
   * is the net proceeds) and the rest re-performs. The timeline replaces
   * the default flag: `defaulted` stays `false` and
   * `recovery_amount`/`default_date` stay unset.
   */
  liquidation?: LiquidationSpec | null;
  /**
   * Market price as percent of par (`60.0` = 60% of par). Read by the
   * excess-CCC coverage rule; an asset without a price is carried at par.
   */
  market_price_pct?: number | null;
  /**
   * Contractual maturity date of the asset.
   */
  maturity: DateWire;
  /**
   * Optional decimal Monthly Default Rate override.
   */
  mdr_override?: number | null;
  /**
   * Net operating income per annum of the property securing the loan,
   * in the loan's currency. The only NOI input of the CMBS pool
   * debt-service coverage ratio.
   */
  noi?: Money | null;
  /**
   * Optional obligor identifier used for single-name concentration limits.
   */
  obligor_id?: string | null;
  /**
   * Origination date of the loan or receivable, if known: the anchor for
   * the seasoning-dependent prepayment/default curves (PSA, SDA, ABS,
   * vector) and the amortization schedule. Falls back to
   * `acquisition_date`, then to the deal closing date (new collateral).
   */
  origination_date?: DateWire | null;
  /**
   * Prepayment penalty the borrower pays on voluntary prepayment; the
   * premium is collected as interest.
   */
  prepayment_penalty?: PrepaymentPenalty | null;
  /**
   * Acquisition price in the asset currency, used for trading gain/loss.
   */
  purchase_price?: Money | null;
  /**
   * Current all-in coupon as an annual decimal rate.
   */
  rate: number;
  /**
   * Optional agency credit rating of the obligor or asset; drives WARF,
   * the CCC bucket and `CoverageRules.rating_haircuts`.
   */
  rating?: CreditRating | null;
  /**
   * Realized or modeled recovery amount when the asset is defaulted.
   */
  recovery_amount?: Money | null;
  /**
   * Per-asset recovery fraction in [0, 1]; overrides the deal recovery model.
   */
  recovery_rate?: number | null;
  /**
   * Optional decimal Single Monthly Mortality override.
   */
  smm_override?: number | null;
  /**
   * Special-servicing state: an appraisal reduction cuts the interest
   * advanced on the loan, and the special servicing fee accrues on it.
   */
  special_servicing?: SpecialServicingSpec | null;
  /**
   * Spread over the reference index in basis points for floating-rate assets.
   * Weighted-average-spread calculations use this field rather than the
   * all-in coupon because the index component is not a credit spread.
   */
  spread_bp?: number | null;
}
/**
 * Balloon terms of a commercial mortgage at its maturity.
 *
 * The performing balance at maturity splits three ways: `loss_prob` defaults
 * (a workout that recovers `100 − severity_pct` percent after
 * `workout_months`), `extension_prob` is extended `extension_months` at
 * `extension_rate` (the loan's coupon when `None`), and the rest pays as the
 * balloon. An extended level-pay balance keeps its schedule scaled by the
 * extended fraction; a fixed `extension_rate` on a floating loan replaces the
 * index and spread (an all-in modification rate).
 */
export interface BalloonSpec {
  /**
   * Months the extended share is extended.
   */
  extension_months: number;
  /**
   * Share of the balloon balance that fails to refinance and is extended,
   * as a decimal in `[0, 1]`.
   */
  extension_prob: number;
  /**
   * Annual coupon of the extended balance as a decimal; `None` keeps the
   * loan's coupon (index plus spread for a floating loan). A value is an
   * all-in fixed rate that replaces the index and spread.
   */
  extension_rate?: number | null;
  /**
   * Share of the balloon balance that defaults at maturity, as a decimal
   * in `[0, 1]`; `extension_prob + loss_prob ≤ 1`. Defaults to `0.0`.
   */
  loss_prob?: number;
  /**
   * Loss severity on the defaulted share in percent of its balance
   * (`40.0` = a 60% recovery). Defaults to `0.0`.
   */
  severity_pct?: number;
  /**
   * Months from maturity until the workout recovery is received; `0`
   * uses the deal's recovery lag. Defaults to `0`.
   */
  workout_months?: number;
}
/**
 * Resolution timeline of a non-performing loan.
 *
 * The loan pays no interest or principal until the first payment date at
 * or after `months_to_resolution` months from its origination (the asset's
 * `acquisition_date`, else the deal closing). On that date the share
 * `1 − reperformance_prob` of the balance liquidates: it is booked as a
 * default whose recovery is `(proceeds_pct − carry_cost_pct)%` of the
 * liquidated balance, released on the resolution date itself (the workout
 * is the timeline). The share `reperformance_prob` re-performs: it becomes a
 * performing loan on its original amortization terms at `modified_rate`
 * (the loan's coupon when `None`) from the following period on, with the
 * level payment recast on the re-performing balance.
 */
export interface LiquidationSpec {
  /**
   * Carry costs over the workout (taxes, insurance, legal, servicing
   * advances) as a percent of the liquidated balance, deducted from the
   * proceeds.
   */
  carry_cost_pct?: number;
  /**
   * Annual coupon the re-performing share pays as a decimal; `None` keeps
   * the loan's coupon.
   */
  modified_rate?: number | null;
  /**
   * Months from the loan's origination (`acquisition_date`, else the deal
   * closing) to the resolution date.
   */
  months_to_resolution: number;
  /**
   * Gross liquidation proceeds as a percent of the liquidated balance.
   */
  proceeds_pct: number;
  /**
   * Share of the balance that re-performs at resolution instead of
   * liquidating, as a decimal in `[0, 1]`.
   */
  reperformance_prob?: number;
}
/**
 * One step of a step-down prepayment penalty: `pct` applies to
 * prepayments on or before `through`.
 */
export interface PenaltyStep {
  /**
   * Premium in percent of the prepaid balance (`3.0` = 3%).
   */
  pct: number;
  /**
   * Last date this step applies.
   */
  through: DateWire;
}
/**
 * Special-servicing state of a commercial mortgage.
 *
 * The appraisal reduction cuts the interest advanced on the loan
 * (an appraisal subordinate entitlement reduction, ASER): the trust
 * collects interest on `1 − appraisal_reduction_pct / 100` of the balance
 * and the shortfall falls on the most junior classes first through the
 * sequential interest waterfall. The special servicing fee accrues only on
 * specially serviced balances.
 */
export interface SpecialServicingSpec {
  /**
   * Appraisal reduction in percent of the loan balance (`40.0` = 40%).
   */
  appraisal_reduction_pct: number;
}
/**
 * Real instruments held as pool collateral.
 *
 * Exactly one collateral representation may be populated on an
 * [`super::AssetPool`]: `assets`, `rep_lines` or `instruments`.
 */
export interface InstrumentCollateral {
  /**
   * Bonds in any form: fixed, floating, step-up, amortizing, custom
   * cashflows, with call/put schedules and return floors.
   */
  bonds?: Bond[];
  /**
   * Deal-level call exercise rule.
   */
  call_exercise?: CallExercisePolicy;
  /**
   * Per-instrument exercise overrides.
   */
  overrides?: InstrumentExerciseOverride[];
  /**
   * Deal-level put exercise rule.
   */
  put_exercise?: PutExercisePolicy;
  /**
   * Revolving credit facilities.
   */
  revolvers?: RevolvingCredit[];
  /**
   * Term loans, including delayed-draw commitments, PIK and call schedules.
   */
  term_loans?: TermLoan[];
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
 * Per-instrument override of the deal-level exercise policies.
 */
export interface InstrumentExerciseOverride {
  /**
   * Call policy for this instrument; `None` keeps the deal-level policy.
   */
  call?: CallExercisePolicy | null;
  /**
   * Identifier of the bond or term loan the override applies to.
   */
  id: Id;
  /**
   * Put policy for this instrument; `None` keeps the deal-level policy.
   */
  put?: PutExercisePolicy | null;
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
 * Deal-level reinvestment period and rules.
 *
 * While the period is active (`is_active` and the payment date is on or
 * before `end_date`), principal proceeds are recycled into collateral
 * instead of repaying the notes. Every note is held flat except those listed
 * in `amortizing_tranches`, which are paid down first; only the principal
 * left after their paydown is reinvested.
 */
export interface ReinvestmentPeriod {
  /**
   * Notes paid down from principal proceeds during the period, in the
   * principal waterfall's order; every other note is held flat. Must name
   * non-equity tranches of the deal.
   */
  amortizing_tranches?: string[];
  /**
   * Terms of the replacement collateral. `None` clones the surviving pool
   * pro rata; `Some` books each period's purchases as a synthetic
   * `REINVEST-{n}` first-lien row with these terms.
   */
  assumptions?: ReinvestmentAssumptions | null;
  /**
   * Eligibility criteria for purchases.
   */
  criteria: ReinvestmentCriteria;
  /**
   * Inclusive end date of the reinvestment period.
   */
  end: DateWire;
  /**
   * Whether reinvestment is currently active.
   */
  is_active: boolean;
}
/**
 * Terms of the collateral bought with reinvested principal.
 *
 * Each purchase creates a bullet first-lien row maturing `maturity_months`
 * after the purchase date, accruing ACT/360 at `forward_curve_id` plus `spread_bp`
 * (or at `spread_bp` as a fixed coupon when no index is given), bought at
 * `price_pct` percent of par. A discount price builds par.
 */
export interface ReinvestmentAssumptions {
  /**
   * Minimum all-in annual coupon in basis points (a floor on the resolved
   * index plus spread, 100 = 1%), if any.
   */
  all_in_floor_bp?: number | null;
  /**
   * Floating-rate index id (a forward curve in the market context), or
   * `None` for a fixed coupon.
   */
  forward_curve_id?: string | null;
  /**
   * Months from the purchase date to the bullet maturity.
   */
  maturity_months: number;
  /**
   * Purchase price as percent of par (`99.0` buys `100/99` of par per unit
   * of cash); must not exceed `ReinvestmentCriteria::max_price_pct`.
   */
  price_pct: number;
  /**
   * Spread over the index in basis points (the whole coupon in basis
   * points when `forward_curve_id` is `None`).
   */
  spread_bp: number;
}
/**
 * Criteria for reinvestment during revolving period
 */
export interface ReinvestmentCriteria {
  /**
   * Maximum purchase price in percent of par (`100.0` = par).
   */
  max_price_pct: number;
  /**
   * Minimum annual decimal current yield: replacement coupon divided by
   * purchase-price fraction. Surviving assets below it are skipped when
   * the pool is replicated pro rata; a synthetic purchase below it is not
   * made.
   */
  min_yield: number;
}
/**
 * Representative line for aggregated pool modeling
 */
export interface RepLine {
  /**
   * Amortization schedule length in months from origination (see
   * `PoolAsset::amortization_term_months`); with `seasoning_months` the
   * line amortizes over `term − seasoning`.
   */
  amortization_term_months?: number | null;
  /**
   * Contractual asset classification; determines level-pay amortization.
   */
  asset_type: AssetType;
  /**
   * Aggregated balance
   */
  balance: Money;
  /**
   * Optional CDR override for this line
   */
  cdr?: number | null;
  /**
   * Contractual periodic payment for level-pay lines (see
   * `PoolAsset::contractual_payment`).
   */
  contractual_payment?: Money | null;
  /**
   * Optional CPR override for this line
   */
  cpr?: number | null;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Rates forward-curve identifier (if floating)
   */
  forward_curve_id?: string | null;
  /**
   * Unique identifier for the rep line
   */
  id: string;
  /**
   * Floor on the floating index in basis points (100 = 1%), applied before
   * `spread_bp`; `None` for no floor.
   */
  index_floor_bp?: number | null;
  /**
   * Interest-only months from origination (see `PoolAsset::io_months`).
   */
  io_months?: number | null;
  /**
   * Weighted average maturity date
   */
  maturity: DateWire;
  /**
   * Weighted average coupon
   */
  rate: number;
  /**
   * Optional recovery rate override
   */
  recovery_rate?: number | null;
  /**
   * Weighted average seasoning in months
   */
  seasoning_months: number;
  /**
   * Weighted average spread (for floating rate)
   */
  spread_bp?: number | null;
}
/**
 * Roll-rate delinquency model with optional servicer advancing and loan
 * modification.
 *
 * `roll_rates[b]` and `cure_rates[b]` are monthly transition probabilities
 * for the balance in bucket `b` (bucket 0 = 30 days past due). The balance
 * that neither rolls nor cures stays in its bucket. The roll out of the last
 * bucket is the charge-off, so a 30/60/90 model with charge-off at 120 days
 * has three entries.
 */
export interface DelinquencyModel {
  /**
   * Servicer advancing of the interest and scheduled principal that
   * delinquent balances miss. Defaults to no advancing.
   */
  advancing?: AdvancingPolicy;
  /**
   * Monthly probability, per bucket, that the bucket's balance returns to
   * current. Decimals in `[0, 1]`, same length as `roll_rates`, with
   * `roll + cure ≤ 1` for every bucket.
   */
  cure_rates: number[];
  /**
   * Loan modification program applied to delinquent balances every
   * month; `None` for no modifications.
   */
  modification?: ModificationSpec | null;
  /**
   * Monthly probability, per bucket, that the bucket's balance rolls to
   * the next bucket; the last entry rolls to charge-off. Decimals in
   * `[0, 1]`; the length is the number of buckets.
   */
  roll_rates: number[];
}
/**
 * Loan modification program: every month a share of each delinquency
 * bucket is modified back to current with a lower coupon and, for
 * level-pay collateral, a recast payment over an extended term.
 *
 * On a rep line the modified share blends into the line's coupon and level
 * payment; the line's maturity is unchanged, so principal the extended term
 * pushes past it is paid as a balloon on that date.
 */
export interface ModificationSpec {
  /**
   * Coupon reduction granted to modified balances, in basis points.
   */
  rate_reduction_bp: number;
  /**
   * Share of each bucket's balance modified per month, as a decimal in
   * `[0, 1]`.
   */
  share_of_delinquent: number;
  /**
   * Term extension granted to modified level-pay balances, in months.
   */
  term_extension_months: number;
}
/**
 * Fee structure for structured credit deals
 */
export interface DealFees {
  /**
   * Manager incentive fee paid from the residual once equity has earned
   * its hurdle IRR; `None` for deals without one.
   */
  incentive_fee?: IncentiveFeeSpec | null;
  /**
   * Special servicer liquidation fee as a percent of the liquidation
   * proceeds of defaulted loans (`1.0` = 1%), taken before the recovery
   * reaches the waterfall; `None` for no liquidation fee.
   */
  liquidation_fee_pct?: number | null;
  /**
   * Master servicer fee (for CMBS/RMBS, basis points)
   */
  master_servicer_fee_bp?: number | null;
  /**
   * Senior management fee (basis points per annum on collateral)
   */
  senior_mgmt_fee_bp: number;
  /**
   * Servicing fee (basis points per annum)
   */
  servicing_fee_bp: number;
  /**
   * Special servicer fee (for CMBS, basis points per annum on the
   * specially serviced balance: loans flagged at closing plus every loan
   * that has since defaulted or taken a balloon loss).
   */
  special_servicer_fee_bp?: number | null;
  /**
   * Subordinated management fee (basis points per annum), paid after every
   * note coupon.
   */
  subordinated_mgmt_fee_bp: number;
  /**
   * Fixed trustee fee per annum; the waterfall divides it by the payment
   * periods per year.
   */
  trustee_fee: Money;
  /**
   * Special servicer workout fee as a percent of the principal and
   * interest collected on specially serviced loans (`1.0` = 1%), taken
   * off the top of those collections; `None` for no workout fee.
   */
  workout_fee_pct?: number | null;
}
/**
 * Manager incentive fee: once the equity IRR to date (invested capital at
 * closing against every distribution to date, including the residual on the
 * current payment date) reaches `hurdle_irr`, the manager takes `share`
 * of the residual interest proceeds ahead of equity.
 */
export interface IncentiveFeeSpec {
  /**
   * Equity IRR hurdle as an annual decimal (`0.12` = 12%).
   */
  hurdle_irr: number;
  /**
   * Share of the residual paid to the manager once the hurdle is met, as
   * a decimal fraction in `[0, 1]`.
   */
  share: number;
}
/**
 * Term-out after revolving: collateral principal repays the facility
 * sequentially for `months` after the (possibly accelerated) revolving end,
 * and whatever collateral is left is then liquidated at the facility's
 * `liquidation_price_pct` to repay the balance (the residual takes the
 * rest).
 */
export interface TermOutSpec {
  /**
   * Months from the revolving end to the facility's final repayment date.
   */
  months: number;
}
/**
 * Autocallable structured product instrument.
 */
export interface Autocallable {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Autocall barrier levels (as ratios of initial spot, e.g., 1.0 = 100%).
   *
   * Each barrier corresponds to the observation date at the same index.
   * If spot ≥ barrier × initial_spot on the observation date, the product autocalls.
   */
  autocall_barriers: number[];
  /**
   * Cap level for final payoff (maximum return)
   */
  cap_level: number;
  /**
   * Coupon barriers corresponding to observation dates.
   */
  coupon_barriers: number[];
  /**
   * Coupon return amounts for each observation date.
   */
  coupons: number[];
  /**
   * Day count convention for interest calculations
   */
  day_count: DayCount;
  /**
   * Discount curve ID for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Optional dividend-yield scalar ID.
   *
   * `Some(id)`: lookup MUST succeed (a missing or non-unitless scalar
   * returns an error). `None`: no implicit default; treated as zero
   * continuous dividend yield. Set explicitly for index underlyings.
   */
  div_yield_id?: Id | null;
  /**
   * Explicit terminal expiry date for the structure.
   */
  expiry: DateWire;
  /**
   * Final barrier level for final payoff determination
   */
  final_barrier: number;
  /**
   * Type of final payoff (capital protection, participation, knock-in put)
   */
  final_payoff_type: FinalPayoffType;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Initial (strike-set) underlying level S_0 used as the reference for
   * barrier and payoff ratios.
   *
   * `None` (the default) uses the spot at the valuation date, which is only
   * correct for a new trade priced on its strike-set date. **Required for
   * seasoned trades** (any observation date on or before `as_of`): pricing
   * errors if past observation dates exist without it.
   */
  initial_level?: number | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Memory ("Phoenix") coupon feature.
   *
   * Missed coupons accrue until a later coupon barrier is met.
   */
  memory_coupons?: boolean;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount
   */
  notional: Money;
  /**
   * Observation dates for autocall and coupon checks.
   *
   * Barriers are monitored **discretely** at these exact dates only.
   * The Monte Carlo time grid is constructed to include these dates precisely.
   */
  observation_dates: DateWire[];
  /**
   * Observed underlying fixings for seasoned trades (date, level pairs).
   *
   * For a mid-life autocallable, every observation date on or before the
   * valuation date must have a matching fixing here; pricing errors
   * otherwise. Past fixings are evaluated deterministically (autocall,
   * missed memory coupons, discrete knock-in monitoring) and only the
   * remaining future observation dates are simulated.
   */
  past_fixings?: [unknown, unknown][];
  /**
   * Explicit path model selection; required to acknowledge model risk.
   */
  path_model: EquityPathModel;
  /**
   * Contractual payment dates corresponding to observation dates.
   */
  payment_dates: DateWire[];
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Spot price identifier for underlying asset
   */
  spot_id: Id;
  /**
   * Underlying asset ticker symbol
   */
  underlying_ticker: string;
  /**
   * Volatility surface ID for option pricing
   */
  vol_surface_id: Id;
}
/**
 * Barrier option with a total trade rebate and explicit contractual monitoring.
 * Continuous monitoring uses closed-form pricing by default. Discrete monitoring
 * observes only the supplied dates and uses Monte Carlo by default.
 */
export interface BarrierOption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Barrier level: the absolute underlying price that triggers the
   * knock-in/out, in the same quote units as `strike`.
   */
  barrier: number;
  /**
   * Barrier type (up/down, in/out)
   */
  barrier_type: BarrierType;
  /**
   * Currency of the strike, premium and present value.
   */
  currency: Currency;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Discount curve ID for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Optional dividend-yield scalar ID
   */
  div_yield_id?: Id | null;
  /**
   * Option expiry date
   */
  expiry: DateWire;
  /**
   * Terminal underlying fixing observed at expiry, in the same quote units
   * as `strike`.
   *
   * Required when valuing after expiry so the realized intrinsic value is
   * invariant to later market spot updates. At expiry, the current market
   * spot is used when this field is absent.
   */
  expiry_fixing?: number | null;
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
   * Contractual monitoring: continuous or an explicit, strictly increasing
   * set of observation dates no later than expiry. Discrete pricing does not
   * interpolate hits between those dates.
   */
  monitoring?: Monitoring;
  /**
   * Observed barrier state for expired options.
   *
   * Historical barrier monitoring must be supplied explicitly for expired
   * options because terminal spot alone does not reveal whether the barrier
   * was breached intralife and then reversed.
   */
  observed_barrier_breached?: boolean | null;
  /**
   * Option type (call or put)
   */
  option_type: OptionType;
  /**
   * Number of underlying units the option is written on; PV and Greeks scale linearly with it.
   */
  quantity: number;
  /**
   * Total contractual trade rebate in the payoff currency, independent of
   * notional. Knock-outs pay on a hit according to `rebate_timing`;
   * knock-ins pay at expiry only if no hit occurred.
   */
  rebate?: Money | null;
  /**
   * Timing of the knock-out rebate payment.
   *
   * `at_hit` (default, market standard) pays the rebate the moment a
   * knock-out barrier is breached; `at_expiry` defers payment to expiry.
   * Knock-in rebates always pay at expiry (a no-hit is only known then),
   * so this setting does not affect them. The analytical pricer values
   * at-hit rebates via the discounted first-passage closed form. Monte
   * Carlo applies at-hit when `rebate_timing == AtHit` via
   * `with_rebate_at_hit`; the crate primitive defaults to at-expiry.
   */
  rebate_timing?: PayoutTiming;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Spot price identifier
   */
  spot_id: Id;
  /**
   * Strike price
   */
  strike: number;
  /**
   * Underlying asset ticker symbol
   */
  underlying_ticker: string;
  /**
   * Volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * Basis swap instrument that exchanges two floating rate payments with different tenors.
 *
 * A basis swap allows parties to exchange floating rate payments based on different
 * reference rates (e.g., 3M SOFR vs 6M SOFR) plus an optional spread on one leg.
 * The primary leg typically receives the spread, while the reference leg pays flat.
 *
 * Each leg owns its own dates, discount curve, calendar, and stub conventions,
 * following the IRS leg-centric pattern.
 *
 * # Cross-Currency (XCCY) Basis Swaps
 *
 * **Important**: This implementation supports **single-currency** basis swaps only.
 * For cross-currency basis swaps, use `XccySwap` instead.
 */
export interface BasisSwap {
  /**
   * Allow calendar-day fallback when the calendar cannot be resolved.
   *
   * When `false` (default), missing calendars are treated as an input error to
   * avoid silently misaligning schedule and payment-lag conventions.
   */
  allow_calendar_fallback?: boolean;
  /**
   * Allow both legs to reference the same forward curve.
   *
   * When `false` (default), having identical forward curves on both legs produces
   * a validation error, as this is almost always a configuration mistake (NPV would
   * equal spread × annuity by construction). Set to `true` only for testing or
   * deliberate same-index spread trades.
   */
  allow_same_curve?: boolean;
  /**
   * Attributes for instrument selection and tagging.
   */
  attributes: Attributes;
  /**
   * Unique identifier for this instrument.
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
   * Notional amount for both legs.
   */
  notional: Money;
  /**
   * Primary leg that typically receives the spread.
   */
  primary_leg: FloatLegSpec;
  /**
   * Reference leg that typically pays flat.
   */
  reference_leg: FloatLegSpec;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
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
 * Simplified basket instrument focused on pricing essentials.
 *
 * This basket represents a collection of financial instruments or market data references
 * that can be valued as a portfolio. It focuses purely on pricing functionality without
 * ETF-specific operational features like creation/redemption mechanics.
 */
export interface Basket {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Basket constituents (the actual holdings)
   */
  constituents: BasketConstituent[];
  /**
   * Discount curve identifier for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Total expense ratio (as decimal, e.g., 0.0025 = 0.25%)
   * This affects pricing through expense drag calculations
   */
  expense_ratio: number;
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
   * Position notional used to scale basket NAV to portfolio PV.
   */
  notional: Money;
  /**
   * Pricing configuration
   */
  pricing_config: BasketPricingConfig;
  /**
   * Reporting currency of the basket: NAV and PV are stated in it and every
   * constituent in another currency is FX-converted into it.
   */
  reporting_currency: Currency;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
}
/**
 * Individual constituent in a basket
 */
export interface BasketConstituent {
  /**
   * Unique identifier for the constituent
   */
  id: string;
  /**
   * Reference to the underlying asset
   */
  reference: ConstituentReference;
  /**
   * Optional ticker symbol for reporting
   */
  ticker?: string | null;
  /**
   * Number of units for physical replication (optional)
   */
  units?: number | null;
  /**
   * Weight in the basket (as a fraction, e.g., 0.05 = 5%)
   */
  weight: number;
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
 * Inflation-Linked Bond instrument
 */
export interface InflationLinkedBond {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Base CPI/index value at issue
   */
  base_cpi: number;
  /**
   * Base date for index (may differ from issue date)
   */
  base_date: DateWire;
  /**
   * Business day convention
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Holiday calendar identifier
   */
  calendar_id?: Id | null;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Deflation protection
   */
  deflation_protection: DeflationProtection;
  /**
   * Discount curve identifier. This **must** be a NOMINAL curve (e.g.
   * "USD-OIS"): the cashflow schedule contains inflation-projected nominal
   * amounts (real amount × projected index ratio), so discounting on a real
   * curve would double-count inflation. The real curve is never used for PV;
   * it enters only through real-yield style metrics computed from real flows.
   */
  discount_curve_id: Id;
  /**
   * Coupon frequency
   */
  frequency: Tenor;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Indexation method
   */
  indexation_method: IndexationMethod;
  /**
   * Inflation index identifier
   */
  inflation_index_id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Issue date
   */
  issue_date: DateWire;
  /**
   * Inflation lag
   */
  lag: InflationLag;
  /**
   * Maturity date
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount (in real terms)
   */
  notional: Money;
  /**
   * Real coupon rate (as decimal)
   */
  real_coupon: DecimalWire;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Stub convention
   */
  stub?: StubKind;
}
/**
 * Dollar roll - simultaneous sale and purchase of TBAs for different months.
 *
 * A dollar roll involves:
 * 1. Selling TBA for near-month settlement
 * 2. Buying TBA for far-month settlement
 *
 * The price difference between the two legs represents the "drop" and
 * implies a financing rate.
 *
 * # Financing and Carry
 *
 * Dollar rolls are used for:
 * - **Financing**: Implied repo rate is often cheaper than repo
 * - **Carry trades**: Profit from drop vs. expected prepayment
 * - **Roll specialness**: When roll drops exceed fair value
 */
export interface DollarRoll {
  /**
   * Agency program.
   */
  agency: AgencyProgram;
  /**
   * Attributes for tagging and selection.
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Back-month price (buy price).
   */
  back_price: number;
  /**
   * Explicit back-month settlement date override.
   *
   * When set, bypasses the SIFMA calendar lookup for the back leg.
   */
  back_settlement_date?: DateWire | null;
  /**
   * Back-month settlement month (1-12).
   */
  back_settlement_month: number;
  /**
   * Back-month settlement year.
   */
  back_settlement_year: number;
  /**
   * Pass-through coupon rate.
   */
  coupon: number;
  /**
   * Discount curve identifier.
   */
  discount_curve_id: Id;
  /**
   * Front-month price (sell price).
   */
  front_price: number;
  /**
   * Explicit front-month settlement date override.
   *
   * When set, bypasses the SIFMA calendar lookup for the front leg.
   */
  front_settlement_date?: DateWire | null;
  /**
   * Front-month settlement month (1-12).
   */
  front_settlement_month: number;
  /**
   * Front-month settlement year.
   */
  front_settlement_year: number;
  /**
   * Unique instrument identifier.
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
   * Trade notional (par amount).
   */
  notional: Money;
  /**
   * Prepayment model of the generic pool both legs deliver.
   *
   * `None` uses the embedded generic PSA assumption of the TBA legs.
   */
  prepayment_spec?: cashflows.PrepaymentModelSpec | null;
  /**
   * Optional repo/financing curve identifier (carry-only).
   *
   * Discount curve used by roll specialness to calculate the simple
   * financing rate `(DF(front)/DF(back) − 1)/τ` over the front/back
   * settlement interval, with `τ` on ACT/360.
   * The implied financing rate itself is determined by prices and carry.
   * See the carry calculations (see [`crate::instruments::fixed_income::dollar_roll::carry`]
   * module). Does **not** affect
   * the mark-to-market PV, which always discounts both legs at
   * `discount_curve_id`.
   *
   * When `None`, the discount curve rate is used as the reference
   * financing rate for carry analytics.
   */
  repo_curve_id?: Id | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * SIFMA settlement class override.
   *
   * When `None`, inferred from agency + term.
   */
  settlement_class?: SifmaSettlementClass | null;
  /**
   * Original loan term.
   */
  term: TbaTerm;
  /**
   * Trade date.
   */
  trade_date?: DateWire | null;
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
 * Cross-currency floating-for-floating swap.
 *
 * Each leg owns its own dates, stub conventions, and calendar. The parent struct
 * only holds the instrument identity, notional exchange mode, and reporting currency.
 */
export interface XccySwap {
  /**
   * Allow a weekends-only calendar when either leg's `leg.calendar_id` is
   * missing or cannot be resolved.
   *
   * When `false` (default), missing calendars are treated as input errors.
   */
  allow_calendar_fallback?: boolean;
  /**
   * Attributes for instrument selection and tagging.
   */
  attributes: Attributes;
  /**
   * Unique identifier for this instrument.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * First leg.
   */
  leg1: XccySwapLeg;
  /**
   * Second leg.
   */
  leg2: XccySwapLeg;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Whether and when principal is exchanged.
   */
  notional_exchange?: NotionalExchange;
  /**
   * PV reporting currency (output currency of `value`/`npv`).
   */
  reporting_currency: Currency;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
}
/**
 * One floating leg of an XCCY swap.
 *
 * The leg's schedule, curves, spread, lags and compounding are a canonical
 * [`FloatLegSpec`]; the leg currency is `notional`'s currency. A reset lag of
 * `0` fixes on the accrual start; negative reset lags are rejected.
 */
export interface XccySwapLeg {
  /**
   * Floating-leg terms (curves, dates, frequency, spread in bp, lags,
   * calendars, compounding).
   */
  leg: FloatLegSpec;
  /**
   * Leg notional in the leg currency.
   */
  notional: Money;
  /**
   * Pay/receive direction for this leg.
   */
  side: PayReceive;
}
/**
 * Zero-coupon Inflation Swap instrument.
 *
 * Represents a zero-coupon inflation swap where one party pays a fixed real rate
 * and the other receives the cumulative inflation over the swap's life. At maturity:
 *
 * ```text
 * Inflation leg = Notional × [CPI(T_mat - Lag) / CPI(T_start - Lag) - 1]
 * Fixed leg     = Notional × [(1 + fixed_rate)^τ - 1]
 * ```
 *
 * # Market Conventions
 *
 * - **Lag**: Standard 3-month lag for US CPI/EUR HICP; 2-month for UK RPI
 * - **Day Count**: Standard fixed legs compound annually with 1/1 accrual per period
 * - **Business Day**: Payment dates adjusted per calendar; index observation typically unadjusted
 *
 * # Validation
 *
 * Call [`validate()`](Self::validate) to check structural invariants before pricing.
 */
export interface InflationSwap {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Explicit Base CPI (reference index level at start with lag applied).
   * If not provided, it will be looked up/calculated from start date.
   */
  base_cpi?: number | null;
  /**
   * Business day convention for payment date adjustment.
   * Defaults to `Following` if not specified.
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Holiday calendar identifier for payment date adjustment.
   * If not specified, payment dates are used unadjusted.
   */
  calendar_id?: Id | null;
  /**
   * Day count for accrual calculation (fixed leg compounding)
   */
  day_count: DayCount;
  /**
   * Discount curve identifier (quote currency)
   */
  discount_curve_id: Id;
  /**
   * Fixed real rate (as decimal)
   */
  fixed_rate: DecimalWire;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Inflation index identifier (e.g., US-CPI-U)
   */
  inflation_index_id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Contractual monthly reference-index interpolation; takes precedence over index metadata;
   * without either source the default is monthly step interpolation.
   */
  interpolation?: InflationInterpolation | null;
  /**
   * Contractual CPI observation lag; when `None` the index lag, then the
   * curve's indexation lag, applies.
   */
  lag?: InflationLag | null;
  /**
   * Maturity date
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional in quote currency
   */
  notional: Money;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Trade side
   */
  side: PayReceive;
  /**
   * Start date of indexation
   */
  start_date: DateWire;
}
/**
 * Year-on-year (YoY) Inflation Swap instrument.
 *
 * Pays periodic inflation rates (CPI ratios over each period) versus a fixed rate.
 */
export interface YoYInflationSwap {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Observed contractual reference CPI at start, with lag and interpolation
   * already applied. If absent, start CPI must resolve from market observations.
   */
  base_cpi?: number | null;
  /**
   * Business day convention for payment date adjustment.
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Holiday calendar identifier for payment date adjustment.
   */
  calendar_id?: Id | null;
  /**
   * Day count for fixed leg accrual calculation
   */
  day_count: DayCount;
  /**
   * Discount curve identifier (quote currency)
   */
  discount_curve_id: Id;
  /**
   * Fixed rate (decimal)
   */
  fixed_rate: DecimalWire;
  /**
   * Payment frequency
   */
  frequency: Tenor;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Inflation index identifier (e.g., US-CPI-U)
   */
  inflation_index_id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Contractual monthly reference-index interpolation; takes precedence over index metadata;
   * without either source the default is monthly step interpolation.
   */
  interpolation?: InflationInterpolation | null;
  /**
   * Contractual CPI observation lag; when `None` the index lag, then the
   * curve's indexation lag, applies.
   */
  lag?: InflationLag | null;
  /**
   * Maturity date
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional in quote currency
   */
  notional: Money;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Trade side
   */
  side: PayReceive;
  /**
   * Start date of the first accrual period
   */
  start_date: DateWire;
}
/**
 * YoY inflation cap/floor instrument.
 */
export interface InflationCapFloor {
  /**
   * Attributes for scenario selection and tagging.
   */
  attributes: Attributes;
  /**
   * Business day convention for schedule and payments.
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional holiday calendar identifier.
   */
  calendar_id?: Id | null;
  /**
   * Day count convention for accrual and option time.
   */
  day_count: DayCount;
  /**
   * Discount curve identifier.
   */
  discount_curve_id: Id;
  /**
   * Payment frequency (ignored for caplet/floorlet).
   */
  frequency: Tenor;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Inflation index/curve identifier (e.g., US-CPI-U).
   */
  inflation_index_id: Id;
  /**
   * Correlation between the inflation index and the nominal short rate,
   * used in the YoY convexity/timing adjustment. `None` ⇒ treated as 0
   * (the timing term vanishes; the pure inflation-vol Jensen convexity
   * `σ_I²·τ` is still applied).
   */
  inflation_nominal_correlation?: number | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Contractual monthly CPI interpolation; takes precedence over index metadata.
   * Defaults to monthly step interpolation when neither source supplies it.
   */
  interpolation?: InflationInterpolation | null;
  /**
   * Contractual CPI observation lag; when `None` the index lag, then the
   * curve's indexation lag, applies.
   */
  lag?: InflationLag | null;
  /**
   * End date of the final inflation period.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Nominal short-rate volatility `σ_n` (annualized, absolute), used in the
   * YoY timing term. `None` ⇒ the `ρ·σ_n` timing term is dropped.
   */
  nominal_rate_volatility?: number | null;
  /**
   * Notional amount in quote currency.
   */
  notional: Money;
  /**
   * Cap/floor type (cap, floor, caplet, floorlet). Caplet and floorlet price a
   * single period.
   */
  rate_option_type: RateOptionType;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Start date of the first inflation period.
   */
  start_date: DateWire;
  /**
   * Strike (annualized, decimal).
   */
  strike: DecimalWire;
  /**
   * Schedule stub convention.
   */
  stub?: StubKind;
  /**
   * Volatility surface identifier.
   */
  vol_surface_id: Id;
}
/**
 * Forward Rate Agreement instrument.
 *
 * A FRA is a forward contract on an interest rate. The holder receives
 * the difference between the realized rate and the fixed rate, paid at
 * the start of the interest period (FRA convention).
 *
 * # Direction Convention
 *
 * - `side = PayReceive::Receive`: Receive fixed rate, pay floating rate.
 *   When forward rate > fixed rate, PV is negative (you're paying more than receiving).
 * - `side = PayReceive::Pay`: Pay fixed rate, receive floating rate.
 *   When forward rate > fixed rate, PV is positive (you're receiving more than paying).
 *
 * # Side field
 *
 * Use `side` to indicate the fixed leg direction. If omitted in JSON,
 * deserialization defaults to `Pay`.
 */
export interface ForwardRateAgreement {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Day count convention for interest accrual
   */
  day_count: DayCount;
  /**
   * Discount curve identifier
   */
  discount_curve_id: Id;
  /**
   * Fixed rate (decimal, e.g., 0.05 for 5%)
   */
  fixed_rate: DecimalWire;
  /**
   * Optional business day convention for fixing date adjustment (default: ModifiedFollowing)
   */
  fixing_business_day_convention?: BusinessDayConvention | null;
  /**
   * Optional fixing calendar identifier for business day adjustment
   */
  fixing_calendar_id?: Id | null;
  /**
   * Rate fixing date. If `None`, inferred from `start_date - reset_lag_days` business days.
   */
  fixing_date?: DateWire | null;
  /**
   * Forward curve identifier
   */
  forward_curve_id: Id;
  /**
   * Unique identifier
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Interest period end date
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
   * Optional observed fixing (locked rate) when known
   */
  observed_fixing?: number | null;
  /**
   * Reset lag in business days (fixing to value date)
   */
  reset_lag_days: number;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Direction of the FRA: Pay means paying the fixed rate (receiving floating),
   * Receive means receiving the fixed rate (paying floating).
   */
  side?: PayReceive;
  /**
   * Interest period start date
   */
  start_date: DateWire;
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
 * Bermudan swaption with multiple exercise dates.
 *
 * A Bermudan swaption gives the holder the right to enter into an interest rate
 * swap at any of a set of predetermined exercise dates. This is the most common
 * type of exotic swaption in the market, used extensively for:
 *
 * - Callable bond hedging
 * - Mortgage prepayment risk management
 * - Structured product hedging
 *
 * # Pricing Methods
 *
 * Bermudan swaptions require numerical methods for pricing:
 * - **Hull-White Tree** (default): Industry standard, calibrated to swaption
 *   volatility. Official PV and risk use this path unless a caller selects
 *   [`crate::pricer::ModelKey::MonteCarloHullWhite1F`].
 * - **LSMC**: Longstaff-Schwartz Monte Carlo, opt-in for path-dependent
 *   validation and model comparison.
 */
export interface BermudanSwaption {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Co-terminal or non-co-terminal exercise
   */
  bermudan_type: BermudanType;
  /**
   * Bermudan exercise schedule
   */
  exercise_schedule: BermudanSchedule;
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
   * Option type (payer = Call, receiver = Put)
   */
  option_type: OptionType;
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
   * Volatility surface ID for calibration
   */
  vol_surface_id: Id;
}
/**
 * Bermudan exercise schedule specification.
 *
 * Defines the exercise dates and constraints for a Bermudan swaption.
 * Exercise dates are typically aligned with swap coupon dates.
 */
export interface BermudanSchedule {
  /**
   * Exercise dates (must be sorted, typically on swap coupon dates)
   */
  exercise_dates: DateWire[];
  /**
   * Lockout period end (no exercise before this date)
   */
  lockout_end?: DateWire | null;
  /**
   * Notice period in business days before exercise
   */
  notice_days: number;
}
/**
 * Interest Rate Future instrument.
 *
 * Position size, multiplier, entry price and lifecycle dates live in the
 * shared [`ListedFutureTerms`]. `terms.multiplier` is the settlement-currency
 * value of one full price point per contract (`tick_value / tick_size`, e.g.
 * $2,500 for CME SR3), `terms.last_trading_date` is the last trading day and
 * `terms.settlement_date` is the date after which the position carries no
 * value (the last trading day for term-rate contracts, the reference-period
 * end for in-arrears overnight contracts).
 */
export interface InterestRateFuture {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Reference-rate method defined by the exchange contract: `simple` (one
   * term fixing, e.g. EURIBOR / 3M Term), `simple_average` (calendar-day
   * weighted arithmetic average of overnight fixings, e.g. CME 1M SOFR /
   * Fed Funds) or `compounded_in_arrears` with zero lookback (daily
   * compounded overnight fixings, e.g. CME 3M SOFR). Other variants are
   * rejected by validation.
   */
  compounding: FloatingLegCompounding;
  /**
   * Contract specifications
   */
  contract_specs: FutureContractSpecs;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Discount curve identifier
   */
  discount_curve_id: Id;
  /**
   * Optional overnight fixing calendar identifier.
   *
   * Required only when the contract currency has no registered standard
   * overnight calendar. Otherwise the currency standard is used.
   */
  fixing_calendar_id?: Id | null;
  /**
   * Underlying rate fixing date.
   *
   * Defaults to `terms.last_trading_date` when omitted.
   */
  fixing_date?: DateWire | null;
  /**
   * Forward curve identifier
   */
  forward_curve_id: Id;
  /**
   * Unique identifier
   */
  id: Id;
  /**
   * Optional rate-index identity keying the historical fixing series.
   *
   * When omitted, historical fixings use `forward_curve_id`. Fixing series
   * are looked up strictly as `FIXING:{id}` without alias fallback.
   */
  index_id?: Id | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Rate period end date.
   *
   * Defaults to `period_start + contract_specs.delivery_months` months when omitted.
   */
  period_end?: DateWire | null;
  /**
   * Rate period start date.
   *
   * Defaults to 2 calendar days after fixing date when omitted.
   */
  period_start?: DateWire | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Standard listed position, multiplier, price and lifecycle terms.
   */
  terms: ListedFutureTerms;
  /**
   * Optional volatility surface identifier for convexity adjustment
   */
  vol_surface_id?: Id | null;
}
/**
 * Contract specifications for interest rate futures.
 *
 * Encapsulates exchange-defined contract parameters and optional convexity
 * adjustment for pricing.
 */
export interface FutureContractSpecs {
  /**
   * Optional pre-computed convexity adjustment (in rate terms).
   *
   * # Usage
   *
   * - `Some(0.0)`: Explicitly disable model-based adjustment (strict mode)
   * - `Some(x)`: Use fixed adjustment of `x` (e.g., from broker quote)
   * - `None`: Compute adjustment from volatility surface (requires `vol_surface_id`)
   *
   * # Market Practice
   *
   * For calibration, use `Some(0.0)` and let the curve fitting process
   * implicitly absorb the convexity. For pricing with a pre-built curve,
   * either:
   * - Use a fixed adjustment from broker/vendor data
   * - Provide a volatility surface for model-based calculation
   */
  convexity_adjustment?: number | null;
  /**
   * Number of delivery months (e.g., 3 for quarterly contracts)
   */
  delivery_months: number;
  /**
   * Face value of one contract in currency units (e.g., $1,000,000 for
   * Eurodollar/SOFR futures)
   */
  face_value: number;
  /**
   * Tick size in price points (e.g., 0.0025 = 0.25bp for SOFR futures)
   */
  tick_size: number;
}
/**
 * Standardized position, multiplier, and lifecycle terms for a listed future.
 */
export interface ListedFutureTerms {
  /**
   * Number of exchange contracts. Fractional values are permitted for
   * portfolio aggregation but must be finite and strictly positive.
   */
  contracts: number;
  /**
   * Currency in which variation margin is paid.
   */
  currency: Currency;
  /**
   * Trade fill price in the same price-point units as the market mark.
   */
  entry_price: number;
  /**
   * Final date on which the contract trades.
   */
  last_trading_date: DateWire;
  /**
   * Settlement-currency value of one full price point per contract.
   */
  multiplier: number;
  /**
   * Long or short position direction.
   */
  position: Position;
  /**
   * Optional live exchange mark, in price points.
   */
  quoted_price?: number | null;
  /**
   * Cash or physical final settlement convention.
   */
  settlement?: ListedFutureSettlement;
  /**
   * Date on which final cash settlement is completed.
   */
  settlement_date: DateWire;
  /**
   * Optional official final settlement price, in price points.
   */
  settlement_price?: number | null;
}
/**
 * Option on an arbitrary interest-rate futures price.
 *
 * One instrument covers exchange-listed contracts and bilateral/OTC trades.
 * All economics are caller-supplied; no exchange symbol or contract definition
 * is embedded in the type.
 */
export interface InterestRateFutureOption {
  /**
   * Attributes for selection and reporting.
   */
  attributes?: Attributes;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs: the flat option volatility in
   * `market_quotes.implied_volatility` (required while live) and optional
   * tree-step overrides.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Complete listed or bilateral option-on-future terms.
   */
  terms: FutureOptionTerms;
}
/**
 * Shared contractual and pricing terms for an asset-owned option on a future.
 */
export interface FutureOptionTerms {
  /**
   * Number of option contracts.
   */
  contracts: number;
  /**
   * Premium, variation-margin, and settlement currency.
   */
  currency: Currency;
  /**
   * Day-count convention for option time and discount-rate inference.
   */
  day_count: DayCount;
  /**
   * Settlement-currency discount curve.
   */
  discount_curve_id: Id;
  /**
   * Recorded early-exercise or expiry observation. Required from expiry onward.
   */
  exercise?: FutureOptionExercise | null;
  /**
   * European or American exercise convention.
   */
  exercise_style: ExerciseStyle;
  /**
   * Contractual option expiry.
   */
  expiry: DateWire;
  /**
   * Current futures mark in the contract's price units.
   */
  futures_price: number;
  /**
   * Black-76 or normal quotation model.
   */
  model: FutureOptionModel;
  /**
   * Settlement-currency value of one underlying futures price point per contract.
   */
  multiplier: number;
  /**
   * Option trade or settlement reference price in option price points.
   *
   * Required for [`FutureOptionPremiumStyle::FuturesStyle`]. Set this to the
   * trade price for cumulative P&L since inception or to the preceding
   * official settlement price for one-day variation margin.
   */
  option_reference_price?: number | null;
  /**
   * Call or put payoff.
   */
  option_type: OptionType;
  /**
   * Long-holder or short-writer position.
   */
  position?: Position;
  /**
   * Up-front premium or futures-style variation-margin convention.
   */
  premium_style: FutureOptionPremiumStyle;
  /**
   * Cash payment or delivery of an underlying future.
   */
  settlement: FutureOptionSettlement;
  /**
   * Option strike in the same futures-price points.
   */
  strike: number;
  /**
   * Underlying futures identifier or descriptive label.
   */
  underlying: string;
  /**
   * Optional change in the underlying futures price, in price points, for
   * a one-basis-point increase in its mapped rate or yield risk factor.
   *
   * Rate-futures-option wrappers use this caller-supplied transform to
   * report DV01 without inferring economics from an exchange symbol.
   */
  underlying_price_change_per_bp?: number | null;
}
/**
 * Exercise or assignment observation for an option on a future.
 */
export interface FutureOptionExercise {
  /**
   * Exercise date. European options require this to equal contractual expiry.
   */
  date: DateWire;
  /**
   * Official underlying futures price used to determine exercise.
   */
  futures_price: number;
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
 * CMS (Constant Maturity Swap) swap instrument.
 *
 * One leg pays a CMS rate (the par swap rate for a reference tenor, e.g., 10Y)
 * observed on each fixing date, and the other leg pays a fixed or floating rate.
 *
 * The CMS rate requires a convexity adjustment because the CMS rate is not a
 * martingale under the payment measure. The adjustment depends on the correlation
 * between the CMS rate and the numeraire (annuity).
 *
 * # Reference
 *
 * Hagan, P. S. (2003). "Convexity Conundrums: Pricing CMS Swaps, Caps, and Floors."
 * *Wilmott Magazine*, March, 38-44. `docs/REFERENCES.md#hagan-2003-cms-convexity`
 */
export interface CmsSwap {
  /**
   * Accrual fractions for each CMS period.
   */
  accrual_fractions: number[];
  /**
   * Attributes for scenario selection and grouping.
   */
  attributes?: Attributes;
  /**
   * Optional cap on the CMS rate (decimal).
   */
  cms_cap?: number | null;
  /**
   * Optional floor on the CMS rate (decimal).
   */
  cms_floor?: number | null;
  /**
   * Additive spread over the CMS rate in basis points (10 = 10bp).
   */
  cms_spread_bp?: DecimalWire;
  /**
   * Tenor of the CMS reference swap (e.g. 10Y); must be month- or year-based.
   */
  cms_tenor: Tenor;
  /**
   * Day count convention for CMS leg accrual.
   */
  day_count: DayCount;
  /**
   * Discount curve ID for present value calculations.
   */
  discount_curve_id: Id;
  /**
   * Fixing dates for CMS rate observations.
   */
  fixing_dates: DateWire[];
  /**
   * Forward/projection curve ID for CMS rate projection.
   */
  forward_curve_id: Id;
  /**
   * Funding leg definition (fixed or floating).
   */
  funding_leg: FundingLeg;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Rate-index convention-registry key of the underlying swap (e.g. `USD-SOFR-OIS`).
   */
  index_id?: Id | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount.
   */
  notional: Money;
  /**
   * Payment dates for the CMS leg.
   */
  payment_dates: DateWire[];
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Pay direction: `Pay` means pay CMS leg, receive funding leg.
   */
  side: PayReceive;
  /**
   * Day count of the underlying swap fixed leg (overrides convention).
   */
  swap_fixed_day_count?: DayCount | null;
  /**
   * Fixed leg frequency of the underlying swap (overrides convention).
   */
  swap_fixed_frequency?: Tenor | null;
  /**
   * Day count of the underlying swap floating leg (overrides convention).
   */
  swap_float_day_count?: DayCount | null;
  /**
   * Floating leg frequency of the underlying swap (overrides convention).
   */
  swap_float_frequency?: Tenor | null;
  /**
   * Volatility surface ID for CMS convexity adjustment.
   */
  vol_surface_id: Id;
}
/**
 * CMS option instrument (cap/floor on CMS rates).
 */
export interface CmsOption {
  /**
   * Accrual fractions for each period
   */
  accrual_fractions: number[];
  /**
   * Attributes for scenario selection and grouping
   */
  attributes?: Attributes;
  /**
   * Tenor of the CMS reference swap (e.g. 10Y); must be month- or year-based.
   */
  cms_tenor: Tenor;
  /**
   * Day count convention for the option accrual
   */
  day_count: DayCount;
  /**
   * Discount curve ID for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Observation/fixing dates for CMS rate
   */
  fixing_dates: DateWire[];
  /**
   * Forward/projection curve ID for CMS rate projection
   */
  forward_curve_id: Id;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Rate-index convention-registry key of the underlying swap (e.g. `USD-SOFR-OIS`).
   *
   * When set, provides default values for `swap_fixed_frequency`, `swap_float_frequency`,
   * `swap_fixed_day_count`, and `swap_float_day_count`. Individual fields still
   * override the convention when explicitly set.
   */
  index_id?: Id | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount
   */
  notional: Money;
  /**
   * Option type (call or put on CMS rate)
   */
  option_type: OptionType;
  /**
   * Payment dates for each period (usually fixing date + lag or period end)
   */
  payment_dates: DateWire[];
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Strike (fixed rate for CMS option)
   */
  strike: DecimalWire;
  /**
   * Day count convention of the underlying swap fixed leg (overrides convention if set)
   */
  swap_fixed_day_count?: DayCount | null;
  /**
   * Fixed leg frequency of the underlying swap (overrides convention if set)
   */
  swap_fixed_frequency?: Tenor | null;
  /**
   * Optional day count convention of the underlying swap floating leg
   */
  swap_float_day_count?: DayCount | null;
  /**
   * Floating leg frequency of the underlying swap (overrides convention if set)
   */
  swap_float_frequency?: Tenor | null;
  /**
   * Volatility surface ID for CMS rates
   */
  vol_surface_id: Id;
}
/**
 * Simple deposit instrument with optional quoted rate.
 *
 * Represents a single-period deposit where principal is exchanged
 * at start and principal plus interest at maturity.
 *
 * # Market Convention Fields
 *
 * The instrument supports optional settlement convention fields for proper
 * business-day adjusted cashflow generation:
 *
 * - `business_day_convention`: Business day convention for date adjustment (default: ModifiedFollowing)
 * - `calendar_id`: Holiday calendar identifier for business day logic (e.g., "nyse", "target")
 *
 * `start_date` is always the accrual start (spot) date. Callers holding a
 * trade date compute the spot date before building (see
 * [`Deposit::from_conventions`]). When `calendar_id` is set, `start_date` and
 * `maturity` are adjusted by the business day convention.
 */
export interface Deposit {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Business day convention for date adjustments.
   *
   * Used to adjust the effective start/end dates to valid business days.
   * Default: `ModifiedFollowing` (standard money market convention).
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional holiday calendar identifier for business day logic.
   *
   * Examples: "nyse", "target", "london", "tokyo".
   * When set, enables calendar-aware spot date and accrual date adjustments.
   */
  calendar_id?: Id | null;
  /**
   * Day count convention for interest accrual.
   */
  day_count: DayCount;
  /**
   * Discount curve id used for valuation and par extraction.
   */
  discount_curve_id: Id;
  /**
   * Optional contractual simple rate r (annualised decimal, 0.045 = 4.5%) for the deposit.
   *
   * Note: `cashflow_schedule()` requires `fixed_rate` to be set. Leaving it as `None`
   * is only appropriate if the caller never requests cashflow generation/PV from
   * this instrument (e.g., constructing placeholders).
   */
  fixed_rate?: DecimalWire | null;
  /**
   * Unique identifier for the deposit.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Maturity date of the deposit period.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Principal amount of the deposit.
   */
  notional: Money;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Start date of the deposit period.
   */
  start_date: DateWire;
}
/**
 * Repurchase Agreement instrument.
 */
export interface Repo {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Business day convention
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional calendar for business day adjustments
   */
  calendar_id?: Id | null;
  /**
   * Cash amount being lent/borrowed
   */
  cash_amount: Money;
  /**
   * Collateral specification
   */
  collateral: CollateralSpec;
  /**
   * Day count convention for interest calculations
   */
  day_count: DayCount;
  /**
   * Discount curve identifier for valuation
   */
  discount_curve_id: Id;
  /**
   * Haircut percentage (as decimal, e.g., 0.02 = 2%)
   */
  haircut: number;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Optional margin specification for mark-to-market margining.
   *
   * When present, enables margin call generation, collateral valuation,
   * and margin interest calculations. See [`RepoMarginSpec`] for details.
   */
  margin_spec?: RepoMarginSpec | null;
  /**
   * Maturity date of the repo
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Repo rate (annual, as decimal)
   */
  repo_rate: DecimalWire;
  /**
   * Type of repo
   */
  repo_type: RepoType;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Start date of the repo
   */
  start_date: DateWire;
  /**
   * Whether this is a tri-party repo
   */
  triparty: boolean;
}
/**
 * Specification of collateral backing a repo.
 */
export interface CollateralSpec {
  /**
   * Type of collateral (general vs special)
   */
  collateral_type: CollateralType;
  /**
   * Identifier of the collateral security (for special collateral, the
   * special security itself).
   */
  instrument_id: Id;
  /**
   * Market value identifier in MarketContext (e.g., "BOND_ABC_PRICE")
   */
  market_value_id: Id;
  /**
   * Quantity/face value of collateral
   */
  quantity: number;
}
/**
 * GMRA 2011 compliant repo margin specification.
 *
 * Defines margin maintenance parameters for repurchase agreements
 * following GMRA 2011 standards.
 *
 * # GMRA 2011 References
 *
 * - Paragraph 4: Margin Maintenance
 * - Paragraph 5: Income Payments
 * - Paragraph 8: Substitution
 * - Annex I: Margin Ratio and Haircut
 */
export interface RepoMarginSpec {
  /**
   * Tenor of margin valuation and calls.
   */
  call_frequency: MarginTenor;
  /**
   * Eligible collateral for substitution (if allowed).
   */
  eligible_substitutes?: EligibleCollateralSchedule | null;
  /**
   * Percentage deviation that triggers a margin call.
   *
   * E.g., 0.01 = 1% deviation from margin ratio triggers a call.
   * If the current ratio falls below `margin_ratio * (1 - threshold)`,
   * a margin call is generated.
   */
  margin_call_threshold: number;
  /**
   * Margin interest rate (if applicable).
   *
   * Typically tied to overnight rates (Fed Funds, SONIA, ESTR).
   */
  margin_interest_rate?: number | null;
  /**
   * Margin ratio (e.g., 1.02 = 102% collateralization required).
   *
   * GMRA typically expresses this as the ratio of Market Value
   * of Securities to Purchase Price.
   */
  margin_ratio: number;
  /**
   * Type of margin mechanism.
   */
  margin_type: RepoMarginType;
  /**
   * Whether margin interest is paid on cash margin transfers.
   *
   * Under GMRA, the parties may agree to pay interest on
   * cash margin transfers.
   */
  pays_margin_interest: boolean;
  /**
   * Settlement lag for margin transfers (business days).
   *
   * GMRA standard is typically same-day (0) or next-day (1).
   */
  settlement_lag: number;
  /**
   * Whether collateral substitution is permitted.
   *
   * GMRA Paragraph 8 governs substitution rights.
   */
  substitution_allowed: boolean;
}
/**
 * Credit Default Swap instrument.
 *
 * # Market Standards & Citations (Week 5)
 *
 * ## ISDA Standards
 *
 * This implementation follows the **ISDA 2014 Credit Derivatives Definitions**:
 * - **Section 1.1:** General Terms and Credit Events
 * - **Section 3.2:** Fixed Payments (Premium Leg)
 * - **Section 3.3:** Floating Payments (Protection Leg)
 * - **Section 7.1:** Settlement Terms
 *
 * ## ISDA CDS Standard Model
 *
 * The pricing engine implements the **ISDA CDS Standard Model (2009)**:
 * - Quarterly premium payments (20th of Mar/Jun/Sep/Dec - IMM dates)
 * - ACT/360 day count
 * - Modified Following business day convention
 * - Accrual-on-default included in premium leg
 * - Settlement: T+3 (North America), T+1 (Europe post-2009)
 *
 * ## Integration Method
 *
 * Protection and accrual-on-default legs use one piecewise-analytical
 * integration engine. Hazard- and discount-curve knots are mandatory
 * boundaries; configurable intra-knot substeps control the additional
 * resolution between those boundaries.
 *
 * ## References
 *
 * - ISDA 2014 Credit Derivatives Definitions `docs/REFERENCES.md#isda-2014-credit-definitions`
 * - "Modelling Single-name and Multi-name Credit Derivatives" by O'Kane (2008) `docs/REFERENCES.md#o-kane-2008`
 * - ISDA CDS Standard Model Implementation (Markit, 2009) `docs/REFERENCES.md#isda-cds-standard-model`
 * - Bloomberg CDSW / *The Bloomberg CDS Model* (DOCS 2057273)
 *   `docs/REFERENCES.md#bloomberg-cds-model`
 *
 * See unit tests and `examples/` for usage.
 */
export interface CreditDefaultSwap {
  /**
   * Additional attributes
   */
  attributes?: Attributes;
  /**
   * ISDA convention
   */
  convention: CdsConvention;
  /**
   * ISDA documentation clause for restructuring credit events.
   *
   * Controls which restructuring events trigger protection payments and
   * the maximum deliverable obligation maturity upon restructuring:
   *
   * - **Cr14** (Full Restructuring): All restructuring events trigger; no maturity cap.
   * - **Mr14** (Modified Restructuring): Restructuring triggers with 30-month maturity cap.
   * - **Mm14** (Modified-Modified Restructuring): Restructuring triggers with 60-month cap.
   * - **Xr14** (No Restructuring): Restructuring does not trigger protection.
   *
   * If `None`, the effective clause is derived from the CDS convention:
   * - `IsdaNa` / `IsdaAs` -> `Xr14` (no restructuring, North American / Asian standard)
   * - `IsdaEu` -> `Mm14` (modified-modified restructuring, European standard)
   *
   * See [`doc_clause_effective`](Self::doc_clause_effective) for resolution logic.
   */
  doc_clause?: CdsDocClause | null;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Optional OTC margin specification for VM/IM.
   *
   * For cleared CDS (e.g., via ICE Clear Credit), use
   * `OtcMarginSpec::cleared("ICE", Currency::USD)`. For bilateral
   * CDS, use `OtcMarginSpec::bilateral_simm(...)`; an explicit
   * `SimmCreditClassification` is required so CS01 is routed to the correct CQ sector
   * bucket or the credit non-qualifying risk class.
   */
  margin_spec?: OtcMarginSpec | null;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount
   */
  notional: Money;
  /**
   * Premium leg specification
   */
  premium_leg: PremiumLegSpec;
  /**
   * Optional protection effective date for forward-starting CDS.
   *
   * When `Some(date)`, protection begins on the specified date rather than
   * the premium leg start date. This allows a CDS where premium accrues
   * from the original start date but credit protection only kicks in later.
   *
   * Must satisfy: `premium.start <= protection_effective_date <= premium.end`.
   *
   * When `None`, protection starts on the premium leg start date (standard CDS).
   */
  protection_effective_date?: DateWire | null;
  /**
   * Protection leg specification
   */
  protection_leg: ProtectionLegSpec;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Buyer/seller perspective
   */
  side: PayReceive;
  /**
   * Upfront payment (Date, Money).
   *
   * The amount is defined as a payment from Protection Buyer to Protection Seller.
   * - If positive: Buyer pays Seller.
   * - If negative: Seller pays Buyer.
   *
   * @minItems 2
   * @maxItems 2
   */
  upfront?: [unknown, unknown] | null;
  /**
   * Valuation presentation convention.
   *
   * Defaults to Bloomberg CDSW clean. Set `IsdaDirty` only when
   * reproducing academic ISDA Standard Model literature.
   */
  valuation_convention?: CdsValuationConvention;
}
/**
 * Specification for CDS premium legs
 */
export interface PremiumLegSpec {
  /**
   * Business day convention
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Holiday calendar identifier
   */
  calendar_id?: Id | null;
  /**
   * Contractual running coupon in basis points (e.g., `100` = 1% per
   * annum, the standard CDX.NA.IG coupon; `500` for high yield).
   */
  coupon_bp: DecimalWire;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Discount curve identifier
   */
  discount_curve_id: Id;
  /**
   * End date of protection
   */
  end: DateWire;
  /**
   * Payment frequency
   */
  frequency: Tenor;
  /**
   * Premium roll-date grid. `cds_imm` selects the standard quarterly CDS
   * roll grid (20 March, June, September and December), including a final
   * maturity stub, and requires quarterly `frequency` and `short_front`
   * `stub`. `none` generates a bespoke schedule from the supplied
   * `frequency` and `stub`. The equity-futures `imm` grid is rejected.
   */
  roll_rule: RollRule;
  /**
   * Start date of protection
   */
  start: DateWire;
  /**
   * Stub convention
   */
  stub?: StubKind;
}
/**
 * Specification for CDS protection legs
 */
export interface ProtectionLegSpec {
  /**
   * Hazard curve identifier for default probabilities
   */
  credit_curve_id: Id;
  /**
   * Recovery rate as a decimal fraction in `[0.0, 1.0)`
   */
  recovery_rate: number;
  /**
   * Settlement delay in business days
   */
  settlement_delay: number;
}
/**
 * CDS Index instrument definition
 */
export interface CdsIndex {
  /**
   * Attributes for tagging and selection
   */
  attributes?: Attributes;
  /**
   * Optional list of constituents when using `IndexPricing::Constituents`
   */
  constituents: CdsIndexConstituent[];
  /**
   * Regional ISDA convention
   */
  convention: CdsConvention;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Index factor (fraction of surviving notional since series inception)
   */
  index_factor: number;
  /**
   * Index name, e.g., "CDX.NA.IG", "CDX.NA.HY", "iTraxx Europe"
   */
  index_name: string;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Optional OTC margin specification for VM/IM.
   *
   * CDS indices are typically cleared through ICE Clear Credit. Use
   * `OtcMarginSpec::cleared("ICE", Currency::USD)` for standard cleared
   * indices. Bilateral SIMM indices must attach an explicit
   * `SimmCreditClassification` rather than infer qualifying status from the
   * index name.
   */
  margin_spec?: OtcMarginSpec | null;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount of the index
   */
  notional: Money;
  /**
   * Number of reference entities in the index pool.
   *
   * Required for portfolio-level analytics (e.g. jump-to-default) when
   * `constituents` is empty (`SingleCurve` mode), since the per-name
   * risk number is `notional / N · LGD`. Index pool sizes drift with
   * series — iTraxx Crossover has been 75 names only since Series 9,
   * and CDX.NA.HY membership varies — so this is supplied explicitly
   * rather than inferred from `index_name`. Standard presets populate
   * it via `CdsIndex::from_preset`; set it directly with
   * `with_num_constituents` for custom indices.
   */
  num_constituents?: number | null;
  /**
   * Premium leg specification (coupon schedule and discounting)
   */
  premium_leg: PremiumLegSpec;
  /**
   * Pricing aggregation mode
   */
  pricing: IndexPricing;
  /**
   * Protection leg specification (credit curve and settlement)
   */
  protection_leg: ProtectionLegSpec;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Series number (e.g., 42)
   */
  series: number;
  /**
   * Protection buyer/seller perspective
   */
  side: PayReceive;
  /**
   * Contractual upfront payment `(payment date, amount)` on the index
   * notional. A positive amount is paid by the protection buyer to the
   * seller; a negative amount by the seller to the buyer. It is
   * discounted from its payment date and dropped once that date is before
   * the valuation date.
   *
   * @minItems 2
   * @maxItems 2
   */
  upfront?: [unknown, unknown] | null;
  /**
   * Version number within series
   */
  version: number;
}
/**
 * Constituent in a CDS index with weight and credit parameters.
 */
export interface CdsIndexConstituent {
  /**
   * Credit configuration for the issuer (includes hazard curve id and recovery)
   */
  credit: CreditParams;
  /**
   * Whether the constituent has defaulted. Defaulted names are excluded from the
   * premium leg but their settled protection payment is already reflected in `index_factor`.
   * Per O'Kane (2008) Ch. 7: "On default, the protection payment is settled and the
   * name is removed from the index. The index factor adjusts to reflect the reduced notional."
   */
  defaulted?: boolean;
  /**
   * Weight of the issuer in the index notional (e.g., 1/125.0 for CDX IG)
   */
  weight: number;
}
/**
 * Credit parameters for CDS instruments
 */
export interface CreditParams {
  /**
   * Credit curve identifier
   */
  credit_curve_id: Id;
  /**
   * Recovery rate (0.0 to 1.0)
   */
  recovery_rate: number;
  /**
   * Reference entity (issuer being protected)
   */
  reference_entity: string;
}
/**
 * CDS Tranche instrument definition (boilerplate)
 */
export interface CdsTranche {
  /**
   * Attachment point in percent (e.g., 0.0 for equity)
   */
  attach_pct: number;
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Business day convention
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional holiday calendar id
   */
  calendar_id?: Id | null;
  /**
   * Running coupon in basis points (e.g., 100 = 1.00%)
   */
  coupon_bp: number;
  /**
   * Credit index identifier for survival/loss modeling (placeholder)
   */
  credit_index_id: Id;
  /**
   * Day count (typically Act/360)
   */
  day_count: DayCount;
  /**
   * Detachment point in percent (e.g., 3.0 for 0-3% tranche)
   */
  detach_pct: number;
  /**
   * Discount curve identifier (by quote currency)
   */
  discount_curve_id: Id;
  /**
   * Payment frequency (typically quarterly)
   */
  frequency: Tenor;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Index name (e.g., "CDX.NA.IG", "CDX.NA.HY", "iTraxx EUR")
   */
  index_name: string;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Maturity date of the tranche
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount of the tranche
   */
  notional: Money;
  /**
   * Realized (settled) loss on the reference pool as a decimal fraction of
   * the original portfolio notional, in `[0, 1]`.
   */
  realized_loss: number;
  /**
   * Coupon roll-date grid. `cds_imm` selects the standard CDS roll dates
   * (20th of Mar, Jun, Sep, Dec); `none` (the default) generates a bespoke
   * schedule from `frequency` and `stub`. The equity-futures `imm` grid is
   * rejected. Use [`Self::standard`] for the IMM constructor.
   */
  roll_rule?: RollRule;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Series number (e.g., 37)
   */
  series: number;
  /**
   * Tranche side, as on CDS and CdsIndex: `pay` buys protection (pays the
   * running premium), `receive` sells protection (receives the premium).
   */
  side: PayReceive;
  /**
   * Optional effective date for schedule anchoring (if None, uses as_of date)
   */
  start_date?: DateWire | null;
  /**
   * Stub convention for a bespoke (`roll_rule = none`) coupon schedule.
   * Defaults to `short_front`.
   */
  stub?: StubKind;
  /**
   * Optional upfront payment (date, amount). Positive means paid by protection buyer.
   *
   * @minItems 2
   * @maxItems 2
   */
  upfront?: [unknown, unknown] | null;
}
/**
 * Credit option instrument (option on CDS spread or clean index price)
 *
 * The public pricing surface supports European options with cash or
 * physical settlement. Before expiry, cash- and physical-settled options
 * carry the same cash-equivalent model NPV and route through the same
 * quadrature; the clean payoff excludes accrued because the same underlying
 * accrued appears on both sides before exercise and cancels. A physical
 * exercise cashflow at settlement is dirty (includes accrued at exercise
 * settlement), and this pricer does not create or deliver a live underlying
 * CDS position — valuation at or after a physical exercise boundary fails
 * explicitly. Non-European exercise is rejected at pricing time so
 * deserialized instruments cannot silently fall through to an unsupported
 * engine.
 */
export interface CdsOption {
  /**
   * Additional attributes
   */
  attributes?: Attributes;
  /**
   * Contractual running coupon `c` of the underlying CDS, in basis points
   * (e.g., `100` for the standard CDX.NA.IG coupon, `500` for the
   * standard CDX.NA.HY coupon). When `None`, the synthetic underlying
   * CDS uses `strike` as its running coupon — the appropriate single-name
   * SNAC default where the trade is struck at the par spread. For CDS
   * index options where the index has a fixed standard coupon different
   * from the option strike, set this explicitly so the strike-adjustment
   * term `H(K) = ξN(c − K)A(K)` (DOCS 2055833 Eq. 2.4) is populated.
   */
  coupon_bp?: DecimalWire | null;
  /**
   * Credit curve identifier
   */
  credit_curve_id: Id;
  /**
   * Discount curve identifier
   */
  discount_curve_id: Id;
  /**
   * Payment date of the exercise proceeds, defaulting to legal expiry.
   * Must be on or after expiry and before CDS maturity. Discounting uses
   * this date; spread variance ends at legal expiry.
   */
  exercise_settlement_date?: DateWire | null;
  /**
   * Exercise style
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
   * Current index factor `f` at valuation: the surviving fraction of the
   * original index notional, in `(0, 1]`. Defaults to `1.0` (no settled
   * defaults) and scales the notional only when `underlying_is_index`.
   * See [`Self::strike_index_factor`] for the original factor `f0`
   * attached to a clean-price strike.
   */
  index_factor?: number;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Whether the option knocks out if the underlying defaults before
   * exercise. This is contract-specific; new instruments default to
   * no-knockout and legacy single-name books can opt in explicitly.
   */
  knockout?: boolean;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount
   */
  notional: Money;
  /**
   * Option type (Call = right to buy protection, Put = right to sell protection)
   */
  option_type: OptionType;
  /**
   * Payment date of the option premium, returned by the settlement-date
   * accessor. This does not change variance time or front-end protection;
   * the option value excludes the separately agreed trade premium.
   */
  premium_settlement_date?: DateWire | null;
  /**
   * Convention used to select the synthetic underlying CDS accrual start
   * when `underlying_start_date` is not explicitly supplied.
   */
  protection_start_convention?: ProtectionStartConvention;
  /**
   * Realized (settled) cumulative index loss from option inception to
   * valuation date, as a decimal fraction of the original index notional
   * in `[0, 1]`. Defaults to `0.0`.
   *
   * Bloomberg CDSO treats index options as no-knockout. Settled losses
   * after option inception are therefore deterministic payoff adjustments
   * at exercise (DOCS 2055833 Eq. 2.5 and DOCS 2151513). Single-name
   * options knock out instead and must leave this at `0.0`.
   */
  realized_loss?: number;
  /**
   * Recovery rate assumption
   */
  recovery_rate: number;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Settlement type
   */
  settlement: SettlementType;
  /**
   * Typed option strike: a decimal forward spread (`{"spread": "0.0325"}`)
   * or a clean price in percentage points (`{"clean_price_pct": "107.0"}`).
   */
  strike: CdsOptionStrike;
  /**
   * Original index factor `f0` attached to the option strike.
   *
   * Distinct from [`Self::index_factor`], which is the current factor
   * `f` at valuation: defaults settled between option strike and
   * valuation reduce `f` below `f0`. A clean-price strike quotes the
   * price on `f0` notional, so its deterministic payoff term scales by
   * `f0 / f`; the strike factor is therefore required for clean-price
   * strikes (`CdsOptionStrike::CleanPricePct`) and must not be inferred
   * from the current factor after a default. Rejected for spread
   * strikes, whose payoff does not reference it.
   */
  strike_index_factor?: number | null;
  /**
   * Convention used by the underlying CDS contract.
   *
   * This controls the CDS schedule, settlement lag, business day convention,
   * and other market-standard mechanics used when deriving forward spread and
   * risky annuity for the option's underlying.
   */
  underlying_convention?: CdsConvention;
  /**
   * If true, the underlying is a CDS index; else single-name CDS.
   *
   * The Bloomberg CDSO model treats the two cases differently in the
   * no-knockout calibration `F_0 = E[V_te]` (DOCS 2055833 §1.2): index
   * options trade no-knockout and the calibration target includes the
   * `(1−R)·(1−q_te)` FEP-equivalent contribution; single-name options
   * knock out on default and skip it.
   */
  underlying_is_index?: boolean;
  /**
   * Underlying CDS maturity date
   */
  underlying_maturity: DateWire;
  /**
   * Underlying CDS accrual-effective date used for forward spread and risky
   * annuity. Bloomberg CDSO can quote a standard CDS effective date before
   * option expiry; in that case premium accrues from this date while
   * protection starts at expiry.
   */
  underlying_start_date?: DateWire | null;
  /**
   * Volatility surface identifier
   */
  vol_surface_id: Id;
}
/**
 * Simple equity (spot) instrument.
 *
 * Represents a spot equity position that can be priced using market data.
 * The price can come from direct market quotes or be computed from
 * underlying fundamentals.
 *
 * See unit tests and `examples/` for usage.
 */
export interface Equity {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Currency in which the equity is quoted
   */
  currency: Currency;
  /**
   * Discount curve ID for pricing
   */
  discount_curve_id: Id;
  /**
   * Optional discrete cash dividends `(ex_date, amount)` for single-name forwards.
   */
  discrete_dividends?: [unknown, unknown][];
  /**
   * Market-scalar id of the unitless continuous dividend yield (decimal,
   * 0.02 = 2%). `None` means a zero dividend yield.
   */
  div_yield_id?: Id | null;
  /**
   * Unique identifier for the equity
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
   * Optional number of shares held (defaults to 1 if not specified).
   */
  quantity?: number | null;
  /**
   * Optional quoted spot price per share in `currency`. When set it wins
   * over the `spot_id` market lookup.
   */
  quoted_spot?: number | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Market-scalar id (`MarketContext::get_price`) of the spot price per
   * share. Required unless `quoted_spot` is set; no id is ever derived from
   * the ticker or instrument id.
   */
  spot_id?: Id | null;
  /**
   * Ticker symbol (e.g., "AAPL", "MSFT")
   */
  ticker: string;
}
/**
 * Equity option instrument
 */
export interface EquityOption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Currency of the strike, premium and present value.
   */
  currency: Currency;
  /**
   * Model year fraction for volatility, dividend carry and exercise times; defaults to ACT/365F. Discount factors retain the curve's own date convention.
   */
  day_count?: DayCount;
  /**
   * Discount curve ID for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Optional discrete dividend schedule for more accurate pricing.
   *
   * Each entry is (ex-date, dividend_amount). When provided, the escrowed
   * dividend model is used: the spot price is adjusted by subtracting the
   * PV of future dividends before option pricing.
   *
   * # Escrowed Dividend Model
   *
   * The adjusted spot is:
   * ```text
   * S* = S - Σ D_i × DF(t_i)
   * ```
   * where D_i is each dividend amount and DF(t_i) is the discount factor
   * to the ex-date. The adjusted spot S* is then used in Black-Scholes
   * with zero dividend yield.
   *
   * # Reference
   *
   * - Haug, Haug, Lewis (2003). "Back to Basics: a new approach to the
   *   discrete dividend problem"
   */
  discrete_dividends?: [unknown, unknown][];
  /**
   * Optional continuous dividend yield identifier.
   *
   * The dividend yield should be a unitless scalar representing the annualized
   * continuous dividend yield (e.g., 0.02 for 2%). This is used in the BSM model
   * as the `q` parameter: `d1 = (ln(S/K) + (r - q + σ²/2)T) / (σ√T)`.
   *
   * # Semantics by value
   *
   * - **`Some(id)`** — the lookup MUST succeed. A missing market scalar
   *   (or a non-unitless type) returns a hard error rather than silently
   *   defaulting to zero, preventing market-data configuration errors
   *   from quietly distorting P&L.
   * - **`None`** — there is *no implicit default curve*. The pricer treats
   *   the underlying as having **zero continuous dividend yield**. This is
   *   correct for non-dividend-paying single stocks; for index options
   *   (typically ~2% yield) callers should set `div_yield_id` explicitly.
   *
   * If `discrete_dividends` is non-empty, an escrowed-dividend adjustment
   * is applied to spot and `q` is set to 0 internally regardless of
   * `div_yield_id`.
   */
  div_yield_id?: Id | null;
  /**
   * Observed exercise or expiry state.
   *
   * Required from expiry onward. It fixes cash-settled intrinsic value or
   * identifies a physical-delivery obligation through its settlement date.
   */
  exercise?: EquityOptionExercise | null;
  /**
   * Exercise schedule for Bermudan options.
   *
   * Required when `exercise_style` is `Bermudan`. Each date represents a time
   * at which early exercise is permitted. Dates before as_of or after expiry
   * are filtered out automatically.
   */
  exercise_dates?: DateWire[] | null;
  /**
   * Exercise style (European or American)
   */
  exercise_style?: ExerciseStyle;
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
   * Option type (call or put)
   */
  option_type: OptionType;
  /**
   * Number of underlying units the option is written on; PV and Greeks scale linearly with it.
   */
  quantity: number;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Settlement type (physical or cash)
   */
  settlement?: SettlementType;
  /**
   * Equity spot price identifier
   */
  spot_id: Id;
  /**
   * Strike price
   */
  strike: number;
  /**
   * Underlying equity ticker symbol
   */
  underlying_ticker: string;
  /**
   * Equity volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * Observed exercise or expiry state for an equity option.
 *
 * From `date` onward the option pricer uses this fixed lifecycle state rather
 * than re-running the live option model. Cash settlement fixes the intrinsic
 * payoff from `spot`; physical settlement retains the marked delivery
 * obligation until `settlement_date`.
 */
export interface EquityOptionExercise {
  /**
   * Exercise date, or the expiry observation date for an unexercised option.
   */
  date: DateWire;
  /**
   * Whether the option was exercised or automatically assigned.
   */
  exercised: boolean;
  /**
   * Contractual cash-payment or physical-delivery date.
   */
  settlement_date: DateWire;
  /**
   * Observed underlying level used to determine the fixed cash payoff.
   */
  spot: number;
}
/**
 * Lookback option instrument.
 *
 * # Monitoring Convention
 *
 * This instrument uses **continuous monitoring** for analytical pricing. Real-world
 * lookback options are typically monitored discretely (daily closes). The continuous
 * formulas provide an upper bound; for accurate discrete pricing, use Monte Carlo.
 *
 * See module-level documentation for details on discrete monitoring adjustments.
 *
 * # Observed Extrema
 *
 * For seasoned options (where some monitoring has already occurred), provide:
 * - `observed_min`: Minimum spot observed so far (for floating calls / fixed puts)
 * - `observed_max`: Maximum spot observed so far (for floating puts / fixed calls)
 *
 * If not provided, the current spot is used as the starting extremum.
 */
export interface LookbackOption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Currency of the strike, premium and present value.
   */
  currency: Currency;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Discount curve ID for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Optional dividend-yield scalar ID
   */
  div_yield_id?: Id | null;
  /**
   * Option expiry date
   */
  expiry: DateWire;
  /**
   * Terminal underlying fixing observed at expiry, in the same quote units
   * as `strike`.
   *
   * Required when valuing after expiry so the realized payoff cannot move
   * with a later market spot snapshot. At expiry itself, the current market
   * spot is treated as the terminal fixing when this field is absent.
   */
  expiry_fixing?: number | null;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Lookback type (fixed or floating strike)
   */
  lookback_type: LookbackType;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Contractual monitoring of the path extremum.
   *
   * `continuous` (default) prices with the Goldman-Sosin-Gatto closed form.
   * `discrete` observes the extremum only on the strictly increasing
   * `observation_dates` (no later than expiry) and prices by Monte Carlo.
   */
  monitoring?: Monitoring;
  /**
   * Observed maximum underlying level since inception, in the same quote
   * units as `strike` (required for Floating Put / Fixed Call once seasoned).
   */
  observed_max?: number | null;
  /**
   * Observed minimum underlying level since inception, in the same quote
   * units as `strike` (required for Floating Call / Fixed Put once seasoned).
   */
  observed_min?: number | null;
  /**
   * Option type (call or put)
   */
  option_type: OptionType;
  /**
   * Number of underlying units the option is written on; PV and Greeks scale linearly with it.
   */
  quantity: number;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Spot price identifier
   */
  spot_id: Id;
  /**
   * Strike price (None for floating strike lookbacks)
   */
  strike?: number | null;
  /**
   * Underlying asset ticker symbol
   */
  underlying_ticker: string;
  /**
   * Volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * Variance swap instrument.
 *
 * A variance swap is a forward contract on realized variance with payoff:
 * ```text
 * Payoff = Notional_variance × (σ²_realized - K²_variance)
 * ```
 *
 * # Notional Conventions
 *
 * Variance swaps use **variance notional** (in variance units), not vega notional.
 * The notional scales variance expressed in decimal-squared units.
 *
 * ## Variance Notional vs Vega Notional
 *
 * Traders often quote positions in **vega notional** (sensitivity to 1% vol change).
 * The conversion between notional types is:
 *
 * ```text
 * Notional_variance = Notional_vega / (2 × σ_strike × 0.01)
 * ```
 *
 * where `σ_strike` is the strike volatility (square root of strike variance).
 *
 * ### Example
 *
 * For a variance swap with strike vol of 20% (strike variance = 0.04):
 * - Vega notional of $100,000 → Variance notional = $100,000 / (2 × 0.20 × 0.01) = $25,000,000
 * - If realized variance is 0.05 (22.4% vol) vs strike 0.04:
 *   - Payoff = $25,000,000 × (0.05 - 0.04) = $250,000
 *
 * # Realized Variance Calculation
 *
 * Uses **log returns** (not simple returns) per market standard:
 * ```text
 * σ²_realized = (252/N) × Σ [ln(S_i / S_{i-1})]²
 * ```
 *
 * where 252 is the standard trading days per year for equity markets.
 *
 * # References
 *
 * - Demeterfi, K. et al. (1999). "More Than You Ever Wanted to Know About Volatility Swaps." `docs/REFERENCES.md#demeterfi-1999-volatility-swaps`
 * - Carr, P. & Madan, D. (1998). "Towards a Theory of Volatility Trading."
 */
export interface VarianceSwap {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Series ID for close prices. Defaults to `spot_id` when absent.
   */
  close_series_id?: string | null;
  /**
   * Day count convention for time calculations
   */
  day_count: DayCount;
  /**
   * Discount curve identifier
   */
  discount_curve_id: Id;
  /**
   * Optional unitless continuous dividend-yield scalar id (decimal,
   * 0.02 = 2%). `None` means a zero dividend yield.
   */
  div_yield_id?: Id | null;
  /**
   * Series ID for high prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
   */
  high_series_id?: string | null;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Series ID for low prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
   */
  low_series_id?: string | null;
  /**
   * Contractual end of the observation period.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Variance notional (in variance units)
   */
  notional: Money;
  /**
   * Business-day convention applied to observation dates.
   */
  observation_business_day_convention?: BusinessDayConvention;
  /**
   * Exchange/fixing calendar used for every realized-variance observation.
   */
  observation_calendar_id: Id;
  /**
   * Preserve month-end rolls for month/year observation frequencies.
   */
  observation_end_of_month?: boolean;
  /**
   * Observation frequency
   */
  observation_frequency: Tenor;
  /**
   * Series ID for open prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
   */
  open_series_id?: string | null;
  /**
   * Corporate-action policy applied to all historical price series.
   *
   * The selected policy is declarative: input series must already conform to
   * it. `Adjusted` is appropriate for single-stock variance where splits
   * must not create artificial returns; `Raw` is explicit for official index
   * levels or contracts whose calculation agent retains raw observations.
   */
  price_series_policy: EquityPriceSeriesPolicy;
  /**
   * Method for calculating realized variance (defaults to CloseToClose)
   */
  realized_var_method?: RealizedVarMethod;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Optional cash-settlement date. Defaults to the adjusted final observation date.
   */
  settlement_date?: DateWire | null;
  /**
   * Pay/receive variance
   */
  side: PayReceive;
  /**
   * Market-scalar id (`MarketContext::get_price`) of the underlying spot
   * level. It is also the default close-price series id.
   */
  spot_id: Id;
  /**
   * Start date of observation period
   */
  start_date: DateWire;
  /**
   * Strike variance (annualized)
   */
  strike_variance: number;
  /**
   * Trading days per year used to annualise daily realized variance.
   *
   * A contract term of the variance-swap confirmation: daily observations
   * are annualised by `trading_days_per_year / observation_frequency.count()`.
   * Must be finite and positive. Defaults to
   * [`crate::constants::TRADING_DAYS_PER_YEAR`] (252).
   */
  trading_days_per_year?: number;
  /**
   * Underlying symbol (equity/index). A label only; market data is read
   * through `spot_id`, `vol_surface_id` and `div_yield_id`.
   */
  underlying_ticker: string;
  /**
   * Volatility surface used to replicate the unobserved variance.
   */
  vol_surface_id: Id;
}
/**
 * Volatility Index Future instrument.
 *
 * Represents a futures contract on a volatility index such as VIX, VXN,
 * or VSTOXX. These contracts provide exposure to expected future volatility.
 * Position size, multiplier, entry price and lifecycle dates live in the
 * shared [`ListedFutureTerms`]; `terms.settlement_price` is the official
 * Special Opening Quotation (SOQ) in index points.
 */
export interface VolatilityIndexFuture {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Discount curve identifier. **Unused in PV**: the future is daily
   * margined so the mark-to-market is undiscounted, and no Dv01 is
   * registered. Retained for market-data identification/scenario plumbing.
   */
  discount_curve_id: Id;
  /**
   * Unique identifier.
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
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Standard listed position and lifecycle terms. `terms.multiplier` is the
   * settlement-currency value of one index point ($1,000 for CBOE VIX),
   * `terms.entry_price` is the trade price in index points and
   * `terms.settlement_date` is the SOQ date on which the final settlement
   * price is fixed.
   */
  terms: ListedFutureTerms;
  /**
   * Volatility index forward curve identifier.
   */
  vol_index_curve_id: Id;
}
/**
 * Exchange-listed option on a volatility-index futures contract.
 */
export interface VolatilityIndexFutureOption {
  /**
   * Attributes for selection and reporting.
   */
  attributes?: Attributes;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs: the flat option volatility in
   * `market_quotes.implied_volatility` (required while live) and optional
   * tree-step overrides.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Caller-supplied option-on-future pricing and settlement terms.
   */
  terms: FutureOptionTerms;
}
/**
 * FX Spot instrument (1 unit of `base` priced in `quote`).
 *
 * Represents the spot exchange rate between two currencies following
 * standard market quoting conventions (base/quote or CCY1/CCY2).
 *
 * # Quote Convention
 *
 * The rate is interpreted as: **1 unit of base = rate units of quote**
 *
 * For example, if `base = EUR`, `quote = USD`, and `quoted_spot = 1.10`:
 * - 1 EUR = 1.10 USD
 * - This is the "EUR/USD" rate
 *
 * # Settlement
 *
 * When `settlement_days` is `None`, the pair-aware default is T+1 for
 * USD↔CAD and USD↔TRY and T+2 otherwise (including EUR/USD). An explicit
 * `settlement_days` overrides that default; `settlement_date` overrides both.
 *
 * See module-level documentation for comprehensive FX quoting conventions.
 */
export interface FxSpot {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Optional base currency calendar for joint calendar settlement adjustment.
   *
   * Per market convention, FX settlement uses the joint calendar of both currencies.
   * A date is a good business day only if it's valid in both calendars.
   */
  base_calendar_id?: string | null;
  /**
   * Base currency (the currency being priced)
   */
  base_currency: Currency;
  /**
   * Business day convention to apply when adjusting settlement (default: ModifiedFollowing)
   *
   * Note: Default changed from `Following` to `ModifiedFollowing` in v0.8.0 to align
   * with ISDA standard FX settlement conventions.
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional quote-currency discount curve for PV-ing the settlement
   * cashflow. When absent the settlement amount is reported undiscounted
   * (a 1–2 day effect for standard spot lags).
   */
  domestic_discount_curve_id?: Id | null;
  /**
   * Unique identifier for the FX pair
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
   * Notional amount in base currency.
   */
  notional: Money;
  /**
   * Optional quote currency calendar for joint calendar settlement adjustment.
   *
   * Per market convention, FX settlement uses the joint calendar of both currencies.
   * A date is a good business day only if it's valid in both calendars.
   */
  quote_calendar_id?: string | null;
  /**
   * Quote currency (the currency used for pricing)
   */
  quote_currency: Currency;
  /**
   * Optional quoted FX spot rate (quote per base); `None` reads the FxMatrix.
   */
  quoted_spot?: number | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Optional explicit settlement (value) date
   */
  settlement_date?: DateWire | null;
  /**
   * Optional T+N settlement lag in business days when `settlement_date` is not provided.
   *
   * `None` uses the pair-aware default: T+1 for USD↔CAD and USD↔TRY, T+2 otherwise.
   */
  settlement_days?: number | null;
}
/**
 * FX Swap instrument definition
 */
export interface FxSwap {
  /**
   * Attributes for scenario selection and tagging.
   */
  attributes: Attributes;
  /**
   * Optional base currency calendar for spot/settlement adjustment metadata.
   */
  base_calendar_id?: string | null;
  /**
   * Base currency (foreign).
   */
  base_currency: Currency;
  /**
   * Domestic discount curve id (quote currency).
   */
  domestic_discount_curve_id: Id;
  /**
   * Far leg settlement date (forward leg).
   */
  far_date: DateWire;
  /**
   * Optional far leg FX rate (quote per base). If None, source from forwards.
   */
  far_rate?: number | null;
  /**
   * Foreign discount curve id (base currency).
   */
  foreign_discount_curve_id: Id;
  /**
   * Unique instrument identifier.
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
   * Near leg settlement date (spot leg).
   */
  near_date: DateWire;
  /**
   * Optional near leg FX rate (quote per base). If None, source from market.
   */
  near_rate?: number | null;
  /**
   * Notional amount in base currency (exchanged on near, reversed on far).
   */
  notional: Money;
  /**
   * Optional quote currency calendar for spot/settlement adjustment metadata.
   */
  quote_calendar_id?: string | null;
  /**
   * Quote currency (domestic).
   */
  quote_currency: Currency;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
}
/**
 * FX forward (outright forward) instrument.
 *
 * Represents a commitment to exchange one currency for another at a specified
 * future date at a predetermined rate. The position is long base currency
 * (foreign) and short quote currency (domestic).
 *
 * # Pricing
 *
 * Forward value is calculated using covered interest rate parity:
 * ```text
 * F_market = S × DF_foreign(T) / DF_domestic(T)
 * PV = notional × (F_market - F_contract) × DF_domestic(T)
 * ```
 * where:
 * - S = spot FX rate (from FxMatrix or quoted_spot)
 * - DF_foreign(T) = discount factor in base currency to maturity
 * - DF_domestic(T) = discount factor in quote currency to maturity
 * - F_contract = contract_rate (if provided, else F_market for at-market forward)
 */
export interface FxForward {
  /**
   * Attributes for scenario selection and tagging.
   */
  attributes: Attributes;
  /**
   * Optional base currency calendar for business day adjustment.
   */
  base_calendar_id?: string | null;
  /**
   * Base currency (foreign currency, numerator of the pair).
   */
  base_currency: Currency;
  /**
   * Contract forward rate (quote per base). If None, valued at-market.
   */
  contract_rate?: number | null;
  /**
   * Domestic (quote currency) discount curve ID.
   */
  domestic_discount_curve_id: Id;
  /**
   * Foreign (base currency) discount curve ID.
   */
  foreign_discount_curve_id: Id;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Maturity/settlement date.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount in base currency.
   */
  notional: Money;
  /**
   * Optional quote currency calendar for business day adjustment.
   */
  quote_calendar_id?: string | null;
  /**
   * Quote currency (domestic currency, denominator of the pair, PV currency).
   */
  quote_currency: Currency;
  /**
   * Optional spot rate override (quote per base). If None, source from FxMatrix.
   */
  quoted_spot?: number | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
}
/**
 * Non-Deliverable Forward (NDF) instrument.
 *
 * Represents a cash-settled forward contract on a restricted currency pair.
 * The position is long base currency (restricted) and short settlement currency.
 *
 * # Quote Convention
 *
 * NDFs support two quote conventions via the `quote_convention` field:
 *
 * - **BasePerSettlement** (default): Rate quoted as base per settlement (e.g., 7.25 CNY/USD)
 * - **SettlementPerBase**: Rate quoted as settlement per base (e.g., 0.138 USD/CNY)
 *
 * See [`NdfQuoteConvention`] for details on the settlement formulas.
 *
 * # Pricing
 *
 * ## Pre-Fixing (observed_fixing = None)
 * Forward rate uses the explicit override or covered interest rate parity
 * with both currency curves.
 *
 * ## Post-Fixing (observed_fixing = Some)
 * Uses the observed fixing rate for settlement calculation.
 *
 * The settlement formula depends on `quote_convention`:
 *
 * **BasePerSettlement:**
 * ```text
 * Settlement = Notional_base × (1/F_fixing - 1/F_contract)
 * PV = Settlement × DF_settlement(T)
 * ```
 *
 * **SettlementPerBase:**
 * ```text
 * Settlement = Notional_base × (F_fixing - F_contract)
 * PV = Settlement × DF_settlement(T)
 * ```
 */
export interface Ndf {
  /**
   * Attributes for scenario selection and tagging.
   */
  attributes: Attributes;
  /**
   * Optional base currency calendar.
   */
  base_calendar_id?: string | null;
  /**
   * Base currency (restricted/non-deliverable currency, numerator).
   */
  base_currency: Currency;
  /**
   * Contract forward rate. Interpretation depends on `quote_convention`.
   * `None` values the NDF at-market.
   */
  contract_rate?: number | null;
  /**
   * Settlement currency discount curve ID.
   */
  domestic_discount_curve_id: Id;
  /**
   * Fixing date (rate observation date, typically T-2 before maturity).
   */
  fixing_date: DateWire;
  /**
   * Official fixing source/benchmark enum for type-safe specification.
   */
  fixing_source?: NdfFixingSource | null;
  /**
   * Optional foreign (base) currency discount curve ID.
   */
  foreign_discount_curve_id?: Id | null;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Maturity/settlement date.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount in base currency.
   */
  notional: Money;
  /**
   * Observed fixing rate. Interpretation depends on `quote_convention`.
   */
  observed_fixing?: number | null;
  /**
   * Quote convention for contract_rate and observed_fixing.
   */
  quote_convention: NdfQuoteConvention;
  /**
   * Explicit pre-fixing forward rate in `quote_convention` units.
   */
  quoted_forward?: number | null;
  /**
   * Optional spot rate override for forward rate calculation.
   */
  quoted_spot?: number | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Optional settlement currency calendar.
   */
  settlement_calendar_id?: string | null;
  /**
   * Settlement currency (freely convertible, typically USD, denominator and PV currency).
   */
  settlement_currency: Currency;
}
/**
 * European FX option with same-day cash settlement at expiry.
 *
 * Physical delivery is intentionally not represented: exercised payoff is a
 * quote-currency cash amount on `expiry`.
 */
export interface FxOption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Base currency (foreign currency)
   */
  base_currency: Currency;
  /**
   * Day count convention
   */
  day_count?: DayCount;
  /**
   * Pair/venue delta convention and premium currency.
   */
  delta_convention: FxDeltaConvention;
  /**
   * Domestic currency discount curve ID
   */
  domestic_discount_curve_id: Id;
  /**
   * Option expiry date
   */
  expiry: DateWire;
  /**
   * Foreign currency discount curve ID
   */
  foreign_discount_curve_id: Id;
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
   * Notional amount in base currency
   */
  notional: Money;
  /**
   * Option type (call or put on base currency)
   */
  option_type: OptionType;
  /**
   * Quote currency (domestic currency)
   */
  quote_currency: Currency;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Strike exchange rate (quote per base).
   *
   * **Note on ATM convention**: Professional FX markets define ATM as the
   * Delta-Neutral Straddle (DNS) strike, not the forward rate. See module
   * documentation for details. If constructing an "ATM" option, compute
   * the forward rate or DNS strike externally.
   */
  strike: number;
  /**
   * FX volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * Explicit venue and premium-currency convention for FX delta reporting.
 */
export interface FxDeltaConvention {
  /**
   * Delta convention quoted by the venue.
   */
  kind: FxDeltaConventionKind;
  /**
   * Currency in which the option premium is paid.
   */
  premium_currency: Currency;
  /**
   * Non-empty market venue or quoting-source identifier.
   */
  venue: string;
}
/**
 * FX digital (binary) option instrument.
 *
 * Pays a fixed cash amount if the option expires in-the-money.
 * Two payout types:
 * - Cash-or-nothing: pays a fixed amount in the payout currency
 * - Asset-or-nothing: pays the spot rate (one unit of foreign currency)
 *
 * # Pricing
 *
 * Uses Garman-Kohlhagen adapted formulas:
 *
 * **Cash-or-nothing call**: `PV = e^{-r_d T} × N(d2) × payout_amount`
 * **Cash-or-nothing put**: `PV = e^{-r_d T} × N(-d2) × payout_amount`
 * **Asset-or-nothing call**: `PV = S × e^{-r_f T} × N(d1) × notional`
 * **Asset-or-nothing put**: `PV = S × e^{-r_f T} × N(-d1) × notional`
 *
 * # References
 *
 * - Reiner, E., & Rubinstein, M. (1991). "Unscrambling the Binary Code."
 *   *Risk Magazine*, 4(9), 75-83. `docs/REFERENCES.md#reiner-rubinstein-1991`
 * - Wystup, U. (2006). *FX Options and Structured Products*. Wiley. `docs/REFERENCES.md#wystup-fx-options`
 */
export interface FxDigitalOption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Base currency (foreign currency)
   */
  base_currency: Currency;
  /**
   * Day count convention (defaults to ACT/365F).
   */
  day_count?: DayCount;
  /**
   * Domestic currency discount curve ID
   */
  domestic_discount_curve_id: Id;
  /**
   * Option expiry date
   */
  expiry: DateWire;
  /**
   * Foreign currency discount curve ID
   */
  foreign_discount_curve_id: Id;
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
   * Notional amount in base currency
   */
  notional: Money;
  /**
   * Option type (call or put on base currency)
   */
  option_type: OptionType;
  /**
   * Fixed payout amount (used for cash-or-nothing; for asset-or-nothing this
   * is the notional of foreign currency delivered)
   */
  payout_amount: Money;
  /**
   * Payout type (cash-or-nothing or asset-or-nothing)
   */
  payout_type: DigitalPayoutType;
  /**
   * Quote currency (domestic currency)
   */
  quote_currency: Currency;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Strike exchange rate (quote per base)
   */
  strike: number;
  /**
   * FX volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * FX touch option (American binary option).
 *
 * Touch options pay a fixed amount if the spot rate touches a barrier
 * level at any time before expiry:
 * - One-touch: pays if barrier is touched
 * - No-touch: pays if barrier is NOT touched
 *
 * # Pricing
 *
 * Uses closed-form pricing for continuous monitoring (Rubinstein & Reiner 1991):
 *
 * **Down-and-in one-touch (S > H, pay at expiry)**:
 * ```text
 * P = e^{-r_d T} × [(S/H)^{-(μ+λ)} × N(η·z) + (S/H)^{-(μ-λ)} × N(η·z')]
 * ```
 *
 * where:
 * - μ = (r_d - r_f - σ²/2) / σ²
 * - λ = sqrt(μ² + 2r_d/σ²)
 * - z = ln(H/S)/(σ√T) + λσ√T
 * - z' = ln(H/S)/(σ√T) - λσ√T
 * - η = +1 for down barrier, -1 for up barrier
 *
 * **No-touch**: P_no_touch = e^{-r_d T} × payout - P_one_touch
 *
 * # References
 *
 * - Rubinstein, M., & Reiner, E. (1991). "Unscrambling the Binary Code."
 *   *Risk Magazine*, 4(9), 75-83. `docs/REFERENCES.md#reiner-rubinstein-1991`
 * - Wystup, U. (2006). *FX Options and Structured Products*. Wiley. `docs/REFERENCES.md#wystup-fx-options`
 */
export interface FxTouchOption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Barrier level (exchange rate that triggers the touch)
   */
  barrier: number;
  /**
   * Barrier direction (up or down)
   */
  barrier_direction: BarrierDirection;
  /**
   * Base currency (foreign currency)
   */
  base_currency: Currency;
  /**
   * Day count convention (defaults to ACT/365F).
   */
  day_count?: DayCount;
  /**
   * Domestic currency discount curve ID
   */
  domestic_discount_curve_id: Id;
  /**
   * Option expiry date
   */
  expiry: DateWire;
  /**
   * Foreign currency discount curve ID
   */
  foreign_discount_curve_id: Id;
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
   * First date on which barrier monitoring is active. When set, a live
   * valuation after this date requires `observed_barrier_breached`.
   */
  monitoring_start_date?: DateWire | null;
  /**
   * Observed barrier event state for expired valuations.
   *
   * `Some(true)` means the barrier was touched during the option life,
   * `Some(false)` means it was observed not to have touched, and `None`
   * means the historical touch state is unavailable.
   *
   * This is only required once the option has expired. Without it, a
   * touched-and-reverted path cannot be distinguished from an untouched path
   * using the terminal spot alone.
   */
  observed_barrier_breached?: boolean | null;
  /**
   * Fixed payout amount
   */
  payout_amount: Money;
  /**
   * Payout timing (at hit or at expiry)
   */
  payout_timing: PayoutTiming;
  /**
   * Quote currency (domestic currency)
   */
  quote_currency: Currency;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Touch type (one-touch or no-touch)
   */
  touch_type: TouchType;
  /**
   * FX volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * FX barrier option instrument.
 */
export interface FxBarrierOption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Barrier level (exchange rate that triggers knock-in/out, dimensionless)
   */
  barrier: number;
  /**
   * Barrier type (up/down, in/out)
   */
  barrier_type: BarrierType;
  /**
   * Base currency (the currency being priced, formerly foreign_currency)
   */
  base_currency: Currency;
  /**
   * Day count convention (defaults to ACT/365F, consistent with FxOption)
   */
  day_count?: DayCount;
  /**
   * Domestic discount curve ID
   */
  domestic_discount_curve_id: Id;
  /**
   * Option expiry date
   */
  expiry: DateWire;
  /**
   * Foreign discount curve ID
   */
  foreign_discount_curve_id: Id;
  /**
   * Optional FX spot scalar identifier.
   *
   * If omitted, pricing falls back to `FxMatrix(base_currency, quote_currency)`.
   */
  fx_spot_id?: Id | null;
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
   * Contractual barrier-monitoring convention.
   *
   * Continuous monitoring uses the analytical Reiner-Rubinstein pricer by
   * default. Discrete monitoring requires explicit observation dates and
   * uses Monte Carlo without interpolating barrier hits between those dates.
   */
  monitoring?: Monitoring;
  /**
   * First date on which barrier monitoring is active. When set, a live
   * valuation after this date requires `observed_barrier_breached`.
   */
  monitoring_start_date?: DateWire | null;
  /**
   * Notional amount in foreign currency
   */
  notional: Money;
  /**
   * Observed barrier state for expired options.
   *
   * Historical monitoring must be supplied explicitly for expired contracts.
   */
  observed_barrier_breached?: boolean | null;
  /**
   * Option type (call or put on foreign currency)
   */
  option_type: OptionType;
  /**
   * Quote currency (the pricing/settlement currency, formerly domestic_currency)
   */
  quote_currency: Currency;
  /**
   * Total contractual trade rebate, paid in the quote (settlement)
   * currency and independent of notional. Knock-outs pay on a hit according
   * to `rebate_timing`; knock-ins pay at expiry only if no hit occurred.
   */
  rebate?: Money | null;
  /**
   * Timing of the knock-out rebate payment.
   *
   * `at_hit` (default, market standard) pays the rebate the moment a
   * knock-out barrier is breached; `at_expiry` defers payment to expiry.
   * Knock-in rebates always pay at expiry, so this setting does not affect
   * them. The analytical pricer values at-hit rebates via the discounted
   * first-passage closed form. Monte Carlo applies at-hit when
   * `rebate_timing == AtHit` via `with_rebate_at_hit`; the crate primitive
   * defaults to at-expiry.
   */
  rebate_timing?: PayoutTiming;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Strike exchange rate (quote per base, dimensionless)
   */
  strike: number;
  /**
   * FX volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * FX variance swap instrument.
 *
 * Payoff: Notional * (Realized Variance - Strike Variance)
 */
export interface FxVarianceSwap {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Base-currency calendar used in the joint observation calendar.
   */
  base_calendar_id: Id;
  /**
   * Base currency (foreign)
   */
  base_currency: Currency;
  /**
   * Series ID for close prices. Defaults to `spot_id` (or currency-pair string) when absent.
   */
  close_series_id?: string | null;
  /**
   * Day count convention for time calculations (defaults to ACT/365F).
   */
  day_count?: DayCount;
  /**
   * Domestic currency discount curve ID
   */
  domestic_discount_curve_id: Id;
  /**
   * Foreign currency discount curve ID
   */
  foreign_discount_curve_id: Id;
  /**
   * Series ID for high prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
   * Defaults to `spot_id` (or currency-pair string) when absent.
   */
  high_series_id?: string | null;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Series ID for low prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
   * Defaults to `spot_id` (or currency-pair string) when absent.
   */
  low_series_id?: string | null;
  /**
   * Contractual end of the observation period.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Variance notional (in quote currency units)
   */
  notional: Money;
  /**
   * Business-day convention applied to observation dates.
   */
  observation_business_day_convention?: BusinessDayConvention;
  /**
   * Preserve month-end rolls for month/year observation frequencies.
   */
  observation_end_of_month?: boolean;
  /**
   * Observation frequency
   */
  observation_frequency: Tenor;
  /**
   * Series ID for open prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
   * Defaults to `spot_id` (or currency-pair string) when absent.
   */
  open_series_id?: string | null;
  /**
   * Quote-currency calendar used in the joint observation calendar.
   */
  quote_calendar_id: Id;
  /**
   * Quote currency (domestic)
   */
  quote_currency: Currency;
  /**
   * Method for calculating realized variance (defaults to CloseToClose)
   */
  realized_var_method?: RealizedVarMethod;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Optional cash-settlement date. Defaults to the adjusted final observation date.
   */
  settlement_date?: DateWire | null;
  /**
   * Pay/receive variance
   */
  side: PayReceive;
  /**
   * Optional spot identifier used to look up historical series.
   */
  spot_id?: Id | null;
  /**
   * Start date of observation period
   */
  start_date: DateWire;
  /**
   * Strike variance (annualized)
   */
  strike_variance: number;
  /**
   * Trading days per year used to annualise daily realized variance.
   *
   * A contract term of the variance-swap confirmation: daily observations
   * are annualised by `trading_days_per_year / observation_frequency.count()`.
   * Must be finite and positive. Defaults to
   * [`crate::constants::TRADING_DAYS_PER_YEAR`] (252).
   */
  trading_days_per_year?: number;
  /**
   * FX volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * Quanto option instrument.
 *
 * Quanto options have payoffs that depend on an underlying asset in one currency
 * but are settled in another currency, creating FX exposure.
 */
export interface QuantoOption {
  /**
   * Attributes for scenario selection and grouping.
   */
  attributes: Attributes;
  /**
   * Base currency (equity denomination).
   */
  base_currency: Currency;
  /**
   * Correlation between equity price and FX rate.
   */
  correlation: number;
  /**
   * Day count convention (defaults to ACT/365F).
   */
  day_count?: DayCount;
  /**
   * Optional dividend-yield scalar ID.
   */
  div_yield_id?: Id | null;
  /**
   * Discount curve ID (domestic currency).
   */
  domestic_discount_curve_id: Id;
  /**
   * Option expiry date.
   */
  expiry: DateWire;
  /**
   * Discount curve ID (foreign currency).
   */
  foreign_discount_curve_id: Id;
  /**
   * Optional FX rate identifier.
   */
  fx_spot_id?: Id | null;
  /**
   * Optional FX volatility surface ID.
   */
  fx_vol_surface_id?: Id | null;
  /**
   * Unique instrument identifier.
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
   * Strike-equivalent domestic reference notional.
   */
  notional: Money;
  /**
   * Option type (call or put).
   */
  option_type: OptionType;
  /**
   * Fixed payoff FX conversion rate from base-currency payoff into quote currency.
   */
  payoff_fx_rate?: number | null;
  /**
   * Number of underlying units covered by the option payoff.
   */
  quantity?: number | null;
  /**
   * Quote currency (payment/settlement currency).
   */
  quote_currency: Currency;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Equity spot price identifier.
   */
  spot_id: Id;
  /**
   * Per-unit strike of the equity underlying, in `base_currency` price units.
   */
  strike: number;
  /**
   * Underlying equity ticker symbol.
   */
  underlying_ticker: string;
  /**
   * Equity volatility surface ID.
   */
  vol_surface_id: Id;
}
/**
 * Commodity option (option on commodity forward or spot).
 *
 * # Pricing
 *
 * - **European options**: Black-76 model using the forward price from the `PriceCurve`
 * - **American options**: Binomial tree (Leisen-Reimer) with cost-of-carry derived from
 *   the forward/spot relationship
 *
 * # American Option Assumptions
 *
 * For American exercise, the model requires a spot price to build the binomial tree.
 * If `spot_id` is provided, it uses that spot price. Otherwise, the forward
 * price is used as a proxy for spot, which may underestimate early exercise value.
 * The convenience yield (cost-of-carry) is implied from the forward/spot ratio:
 * `q = r - ln(F/S)/T`
 *
 * Bermudan exercise uses the configured exercise schedule and a binomial
 * tree. The schedule must contain at least one strictly future date when the
 * option is priced before expiry.
 *
 * # Forward Price Retrieval
 *
 * Forward prices are retrieved from a `PriceCurve` (not a `ForwardCurve`).
 * The curve must be added via `MarketContext::insert_price_curve()`.
 * If `quoted_forward` is provided, it overrides the curve lookup.
 */
export interface CommodityOption {
  /**
   * Attributes for tagging and selection.
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Commodity type (e.g., "Energy", "Metal", "Agricultural")
   */
  commodity_type: string;
  /**
   * Optional market convention for this commodity.
   *
   * When set, provides default premium settlement days and calendar.
   */
  convention?: CommodityConvention | null;
  /**
   * Base currency for pricing
   */
  currency: Currency;
  /**
   * Day count convention for time to expiry.
   */
  day_count?: DayCount;
  /**
   * Discount curve ID for present value.
   */
  discount_curve_id: Id;
  /**
   * Optional Bermudan exercise schedule.
   *
   * Required when `exercise_style == ExerciseStyle::Bermudan`.
   */
  exercise_dates?: DateWire[] | null;
  /**
   * Exercise style (European or American).
   */
  exercise_style?: ExerciseStyle;
  /**
   * Option expiry date.
   */
  expiry: DateWire;
  /**
   * Forward/futures curve ID for price interpolation.
   */
  forward_curve_id: Id;
  /**
   * Unique instrument identifier.
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
   * Contract multiplier (typically 1.0 for OTC options).
   */
  multiplier: PositiveF64Wire;
  /**
   * Option type (call or put).
   */
  option_type: OptionType;
  /**
   * Premium settlement lag in business days after trade date.
   *
   * Standard: T+1 for most exchange-traded options, T+2 for OTC.
   * If not set and `convention` is provided, uses convention default.
   * Otherwise defaults to 1 (T+1).
   */
  premium_settlement_days?: number | null;
  /**
   * Contract quantity in units.
   */
  quantity: PositiveF64Wire;
  /**
   * Optional quoted forward price (overrides curve lookup).
   */
  quoted_forward?: number | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Settlement type (physical or cash).
   *
   * Defaults to cash settlement when omitted in serialized payloads.
   */
  settlement?: SettlementType;
  /**
   * Optional spot price ID (for spot-based pricing and American options).
   */
  spot_id?: Id | null;
  /**
   * Strike price per unit.
   */
  strike: PositiveF64Wire;
  /**
   * Commodity symbol label (e.g., "CL", "GC", "NG"); never a market-data key.
   */
  underlying_ticker: string;
  /**
   * Unit of measurement (e.g., "BBL", "OZ", "MT", "MMBTU")
   */
  unit: string;
  /**
   * Volatility surface ID for implied vol.
   */
  vol_surface_id: Id;
}
/**
 * Commodity Asian option: option on the arithmetic or geometric average of
 * commodity prices.
 *
 * This is the dominant option type in commodity markets. The average is
 * typically computed over commodity forward/futures prices for specific
 * delivery periods.
 *
 * # Pricing Models
 *
 * | Averaging | Model | Accuracy |
 * |-----------|-------|----------|
 * | Geometric | Kemna-Vorst (1990) with forwards | Exact closed-form |
 * | Arithmetic | Turnbull-Wakeman (1991) with forwards | ~1% vs Monte Carlo |
 *
 * # Forward-Based Averaging
 *
 * For each future fixing date `t_i`, the forward price `F(t_i)` is read from
 * the price curve. The average forward is:
 * ```text
 * F_avg = (Σ_realized + Σ F(t_i)) / n
 * ```
 * where the sum includes both realized fixings and projected forwards.
 */
export interface CommodityAsianOption {
  /**
   * Attributes for scenario selection and grouping.
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Averaging method (arithmetic or geometric).
   */
  averaging_method: AveragingMethod;
  /**
   * Commodity type (e.g., "Energy", "Metal", "Agricultural")
   */
  commodity_type: string;
  /**
   * Base currency for pricing
   */
  currency: Currency;
  /**
   * Day count convention.
   */
  day_count?: DayCount;
  /**
   * Discount curve ID for present value calculations.
   */
  discount_curve_id: Id;
  /**
   * Option expiry/settlement date for the payoff.
   */
  expiry: DateWire;
  /**
   * Dates on which the commodity price is observed for averaging.
   *
   * **Note**: These dates should be pre-adjusted for business day conventions.
   */
  fixing_dates: DateWire[];
  /**
   * Forward/futures price curve ID.
   */
  forward_curve_id: Id;
  /**
   * Unique instrument identifier.
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
   * Option type (call or put).
   */
  option_type: OptionType;
  /**
   * Already observed fixings for seasoned options (ex-date, price pairs).
   */
  past_fixings?: [unknown, unknown][];
  /**
   * Contract quantity in commodity units.
   */
  quantity: PositiveF64Wire;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Strike price per unit.
   */
  strike: PositiveF64Wire;
  /**
   * Commodity symbol label (e.g., "CL", "GC", "NG"); never a market-data key.
   */
  underlying_ticker: string;
  /**
   * Unit of measurement (e.g., "BBL", "OZ", "MT", "MMBTU")
   */
  unit: string;
  /**
   * Volatility surface ID for implied vol.
   */
  vol_surface_id: Id;
}
/**
 * Commodity forward or futures contract.
 *
 * Represents a commitment to buy or sell a commodity at a specified future
 * date at a predetermined price. Can be physically settled (delivery) or
 * cash settled (price difference).
 *
 * # Pricing
 *
 * Forward value is calculated as:
 * ```text
 * NPV = sign(position) × (F - K) × Q × M × DF(T)
 * ```
 * where:
 * - sign = +1.0 for Long, -1.0 for Short
 * - F = Forward price from price curve (or quoted_forward if provided)
 * - K = Contract price (entry price). If None, treated as at-market (K = F)
 * - Q = Quantity
 * - M = Contract multiplier
 * - DF(T) = Discount factor to settlement date
 *
 * # At-Market vs Off-Market
 *
 * - **At-market**: `contract_price = None` → NPV ≈ 0 (like entering a new futures position)
 * - **Off-market**: `contract_price = Some(K)` → NPV reflects mark-to-market vs K
 */
export interface CommodityForward {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Commodity type (e.g., "Energy", "Metal", "Agricultural")
   */
  commodity_type: string;
  /**
   * Optional contract month (e.g., "2025M03" for March 2025).
   */
  contract_month?: string | null;
  /**
   * Contract price (entry/trade price K).
   *
   * If `None`, the forward is treated as **at-market** (K = F), meaning
   * NPV ≈ 0 at inception (like a newly opened futures position).
   *
   * If `Some(K)`, the forward is **off-market** and NPV reflects the
   * mark-to-market difference: sign × (F - K) × Q × M × DF.
   */
  contract_price?: number | null;
  /**
   * Optional market convention for this commodity.
   *
   * When set, provides default settlement days and calendar if not
   * explicitly specified. See `CommodityConvention` for available options.
   */
  convention?: CommodityConvention | null;
  /**
   * Base currency for pricing
   */
  currency: Currency;
  /**
   * Discount curve ID.
   */
  discount_curve_id: Id;
  /**
   * Optional exchange identifier (e.g., "NYMEX", "ICE").
   */
  exchange?: string | null;
  /**
   * Forward/futures price curve ID for price interpolation.
   *
   * Should reference a `PriceCurve` in the `MarketContext`.
   */
  forward_curve_id: Id;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Settlement/delivery date.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Contract multiplier (typically 1.0 for OTC forwards, defaults to 1.0).
   */
  multiplier?: PositiveF64Wire;
  /**
   * Position direction (long or short).
   *
   * - Long: buyer of the commodity at settlement
   * - Short: seller of the commodity at settlement
   */
  position?: Position;
  /**
   * Contract quantity in units.
   */
  quantity: PositiveF64Wire;
  /**
   * Optional quoted forward price (overrides curve lookup for F).
   *
   * This is a market price override, not the contract entry price.
   * Use `contract_price` for the trade entry price K.
   */
  quoted_forward?: number | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Settlement type (physical or cash).
   *
   * Defaults to cash settlement when omitted in serialized payloads.
   */
  settlement?: SettlementType;
  /**
   * Business day convention for settlement date adjustment.
   *
   * Defaults to `Following` for energy commodities, `ModifiedFollowing`
   * for precious metals.
   */
  settlement_business_day_convention?: BusinessDayConvention | null;
  /**
   * Calendar ID for settlement date adjustments.
   *
   * Used for business day adjustment of the settlement date. If `convention`
   * is set, uses the convention's calendar unless explicitly overridden.
   */
  settlement_calendar_id?: Id | null;
  /**
   * Settlement lag in business days (T+N).
   *
   * Defaults to 2 for most commodity markets (T+2). If `convention` is set,
   * uses the convention's default unless explicitly overridden here.
   *
   * # Market Standards
   *
   * | Market | Settlement |
   * |--------|------------|
   * | Energy (WTI, Brent, NG) | T+2 |
   * | Precious metals | T+2 |
   * | Base metals (LME) | T+2 |
   * | Power | T+1 |
   */
  settlement_days?: number | null;
  /**
   * Optional spot price ID (for delta calculations).
   */
  spot_id?: Id | null;
  /**
   * Commodity symbol label (e.g., "CL", "GC", "NG"); never a market-data key.
   */
  underlying_ticker: string;
  /**
   * Unit of measurement (e.g., "BBL", "OZ", "MT", "MMBTU")
   */
  unit: string;
}
/**
 * Commodity swap (fixed-for-floating commodity price exchange).
 *
 * One party pays a fixed price per unit, the other pays a floating price
 * determined by an index or average of spot prices over the period.
 *
 * # Pricing
 *
 * Fixed leg: ∑ Q × P_fixed × DF(t_i)
 * Floating leg: ∑ Q × E[P_float(t_i)] × DF(t_i)
 *
 * For a payer of fixed:
 * NPV = Floating leg PV - Fixed leg PV
 */
export interface CommoditySwap {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Business day convention for payment-schedule date adjustments.
   *
   * Only applied when `calendar_id` is set and resolves to a registered
   * holiday calendar; without a calendar the schedule dates are left
   * unadjusted.
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional calendar ID for date adjustments.
   */
  calendar_id?: Id | null;
  /**
   * Commodity type (e.g., "Energy", "Metal", "Agricultural")
   */
  commodity_type: string;
  /**
   * Base currency for pricing
   */
  currency: Currency;
  /**
   * Discount curve ID.
   */
  discount_curve_id: Id;
  /**
   * Fixed price per commodity unit, in the notional currency (finite).
   */
  fixed_price: number;
  /**
   * Commodity forward `PriceCurve` that projects the floating-leg price observations.
   */
  forward_curve_id: Id;
  /**
   * Payment frequency as a Tenor.
   */
  frequency: Tenor;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Optional index lag in **calendar days**: the floating-leg averaging
   * window is shifted back by exactly this many calendar days (no
   * business-day adjustment of the shifted window endpoints).
   */
  index_lag_days?: number | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * End date of the swap.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Realized floating-index fixings as `(date, price)` pairs.
   *
   * Floating-leg observations with date strictly before the valuation date
   * read from this store; a missing past fixing is an error — no silent
   * substitution of today's spot . Observations on or
   * after the valuation date project from the price curve.
   */
  past_fixings?: [unknown, unknown][];
  /**
   * Notional quantity per period.
   */
  quantity: PositiveF64Wire;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Direction of the swap: Pay means paying the fixed price leg,
   * Receive means receiving the fixed price leg.
   */
  side?: PayReceive;
  /**
   * Start date of the swap.
   */
  start_date: DateWire;
  /**
   * Commodity symbol label (e.g., "CL", "GC", "NG"); never a market-data key.
   */
  underlying_ticker: string;
  /**
   * Unit of measurement (e.g., "BBL", "OZ", "MT", "MMBTU")
   */
  unit: string;
}
/**
 * Commodity swaption (option on a fixed-for-floating commodity swap).
 *
 * The holder has the right to enter a commodity swap at expiry, paying
 * (or receiving) a fixed price in exchange for floating commodity prices.
 *
 * # Pricing
 *
 * Black-76 model on the forward swap rate:
 * - Forward swap rate is the weighted average of forward commodity prices
 *   over the swap period
 * - Annuity factor captures the present value of a unit payment stream
 */
export interface CommoditySwaption {
  /**
   * Attributes for scenario selection and tagging.
   */
  attributes?: Attributes;
  /**
   * Business day convention for date adjustments.
   */
  business_day_convention?: BusinessDayConvention;
  /**
   * Optional calendar ID for date adjustments.
   */
  calendar_id?: Id | null;
  /**
   * Commodity type (e.g., "Energy", "Metal", "Agricultural")
   */
  commodity_type: string;
  /**
   * Base currency for pricing
   */
  currency: Currency;
  /**
   * Day count convention for time to expiry.
   */
  day_count?: DayCount;
  /**
   * Discount curve ID for present value.
   */
  discount_curve_id: Id;
  /**
   * Option expiry date.
   */
  expiry: DateWire;
  /**
   * Fixed price (strike) of the underlying swap.
   */
  fixed_price: PositiveF64Wire;
  /**
   * Forward/futures curve ID for commodity price interpolation.
   */
  forward_curve_id: Id;
  /**
   * Unique instrument identifier.
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
   * Option type (call = right to enter pay-fixed swap, put = right to enter receive-fixed swap).
   */
  option_type: OptionType;
  /**
   * Notional quantity per period.
   */
  quantity: PositiveF64Wire;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Underlying swap payment frequency.
   */
  swap_frequency: Tenor;
  /**
   * Underlying swap end date.
   */
  underlying_maturity: DateWire;
  /**
   * Underlying swap start date.
   */
  underlying_start_date: DateWire;
  /**
   * Commodity symbol label (e.g., "CL", "GC", "NG"); never a market-data key.
   */
  underlying_ticker: string;
  /**
   * Unit of measurement (e.g., "BBL", "OZ", "MT", "MMBTU")
   */
  unit: string;
  /**
   * Volatility surface ID for implied vol.
   */
  vol_surface_id: Id;
}
/**
 * Commodity spread option: option on the price difference between two commodities.
 *
 * Pays max(S1 - S2 - K, 0) for calls, max(K - (S1 - S2), 0) for puts.
 *
 * # Kirk's Approximation
 *
 * The spread option is priced using Kirk's approximation, which maps the
 * two-asset problem into a single-asset Black-76 framework:
 *
 * 1. Forward prices F1, F2 from respective price curves
 * 2. Adjusted strike: K_adj = F2 + K
 * 3. Kirk's volatility:
 *    sigma_kirk = sqrt(sigma1^2 - 2*rho*sigma1*sigma2*F2/(F2+K) + (sigma2*F2/(F2+K))^2)
 * 4. Price via Black-76 on F1 vs K_adj with sigma_kirk
 *
 * For puts, put-call parity is used: P = C - DF * (F1 - F2 - K)
 *
 * # Correlation
 *
 * The `correlation` parameter captures the co-movement between the two
 * commodity prices. Higher correlation reduces the effective spread volatility
 * and hence the option price. The correlation must be in [-1, 1].
 */
export interface CommoditySpreadOption {
  /**
   * Attributes for scenario selection and tagging.
   */
  attributes?: Attributes;
  /**
   * Correlation between the two commodity prices, in [-1, 1].
   */
  correlation: CorrelationWire;
  /**
   * Settlement currency.
   */
  currency: Currency;
  /**
   * Day count convention for time to expiry.
   */
  day_count?: DayCount;
  /**
   * Discount curve ID for present value.
   */
  discount_curve_id: Id;
  /**
   * Option expiry date.
   */
  expiry: DateWire;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Forward/price curve ID for leg 1 (the "long" commodity).
   */
  leg1_forward_curve_id: Id;
  /**
   * Volatility surface ID for leg 1.
   */
  leg1_vol_surface_id: Id;
  /**
   * Forward/price curve ID for leg 2 (the "short" commodity).
   */
  leg2_forward_curve_id: Id;
  /**
   * Volatility surface ID for leg 2.
   */
  leg2_vol_surface_id: Id;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Option type (call or put on the spread S1 - S2).
   */
  option_type: OptionType;
  /**
   * Notional quantity (number of units).
   */
  quantity: PositiveF64Wire;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Spread strike price K in the payoff max(S1 - S2 - K, 0).
   */
  strike: number;
}
/**
 * Exchange-listed future whose settlement is one price or an average of prices.
 *
 * This covers linear commodity and price-index futures, including monthly
 * average-settled contracts such as iron ore, energy, and freight futures.
 */
export interface CommodityFuture {
  /**
   * Attributes for selection and reporting.
   */
  attributes?: Attributes;
  /**
   * Final-settlement price fixing rule (single observation or average).
   */
  fixing: CommodityFutureFixing;
  /**
   * Commodity forward `PriceCurve` used for projected observations.
   */
  forward_curve_id: Id;
  /**
   * Unique instrument identifier.
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
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Standard listed position and lifecycle terms.
   */
  terms: ListedFutureTerms;
  /**
   * Exchange symbol or underlying label.
   */
  underlying_ticker: string;
}
/**
 * Exchange-listed option on an arbitrary commodity futures contract.
 */
export interface CommodityFutureOption {
  /**
   * Attributes for selection and reporting.
   */
  attributes?: Attributes;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs: the flat option volatility in
   * `market_quotes.implied_volatility` (required while live) and optional
   * tree-step overrides.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Caller-supplied option-on-future pricing and settlement terms.
   */
  terms: FutureOptionTerms;
}
/**
 * Exchange-listed future on a deliverable currency pair.
 *
 * Prices are quote-currency units per one base-currency unit. The listed
 * multiplier is the base-currency contract size, so a one-unit price move is
 * worth `multiplier` units of the quote currency per contract.
 *
 * # Model limitation
 *
 * [`Self::fair_price`] is a deterministic-rate approximation: it equals the
 * covered-interest-parity forward. Use it only when forward/futures convexity
 * is immaterial or handled outside this instrument.
 */
export interface FxFuture {
  /**
   * Attributes for selection and reporting.
   */
  attributes?: Attributes;
  /**
   * Base currency, the numerator of the quoted pair.
   */
  base_currency: Currency;
  /**
   * Quote-currency discount curve.
   */
  domestic_discount_curve_id: Id;
  /**
   * Base-currency discount curve.
   */
  foreign_discount_curve_id: Id;
  /**
   * Unique instrument identifier.
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
   * Optional spot override in quote currency per base currency.
   */
  quoted_spot?: number | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Standard listed position and lifecycle terms. `terms.currency` is the
   * quote currency of the pair and the variation-margin currency.
   */
  terms: ListedFutureTerms;
}
/**
 * Exchange-listed option on an arbitrary FX futures contract.
 */
export interface FxFutureOption {
  /**
   * Attributes for selection and reporting.
   */
  attributes?: Attributes;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs: the flat option volatility in
   * `market_quotes.implied_volatility` (required while live) and optional
   * tree-step overrides.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Caller-supplied option-on-future pricing and settlement terms.
   */
  terms: FutureOptionTerms;
}
/**
 * Exchange-listed future on an equity, equity index, or fixed-currency quanto index.
 */
export interface EquityFuture {
  /**
   * Attributes for selection and reporting.
   */
  attributes?: Attributes;
  /**
   * Settlement-currency discount curve. Without `quanto` the settlement and
   * underlying currencies match, so this curve also carries the equity.
   */
  discount_curve_id: Id;
  /**
   * Optional discrete dividends `(ex_date, amount)` in index points.
   */
  discrete_dividends?: [unknown, unknown][];
  /**
   * Optional continuous dividend-yield scalar in decimal annual units.
   */
  div_yield_id?: Id | null;
  /**
   * Unique instrument identifier.
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
   * Required quanto adjustment when settlement and underlying currencies
   * differ. `quanto.asset_discount_curve_id` is the underlying-currency
   * carry curve and `quanto.asset_currency` must equal `underlying_currency`.
   */
  quanto?: QuantoSpec | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Current equity or index level.
   */
  spot_id: Id;
  /**
   * Standard listed position and lifecycle terms.
   */
  terms: ListedFutureTerms;
  /**
   * Currency in which the underlying spot and dividends are quoted.
   */
  underlying_currency: Currency;
  /**
   * Equity or index ticker.
   */
  underlying_ticker: string;
  /**
   * Equity volatility surface (decimal vol per square-root year). Required
   * with `quanto`, where it drives the quanto drift.
   */
  vol_surface_id?: Id | null;
}
/**
 * Quanto adjustment parameters for instruments where payoff currency differs from
 * underlying currency.
 */
export interface QuantoSpec {
  /**
   * Currency in which the underlying asset is quoted and financed.
   */
  asset_currency: Currency;
  /**
   * Discount curve for financing the underlying in its asset currency.
   */
  asset_discount_curve_id: Id;
  /**
   * Correlation between the asset price and payoff-currency units per asset-currency unit.
   * Must be in [-1, 1].
   */
  correlation: CorrelationWire;
  /**
   * Required positive FX spot scalar in payoff-currency units per asset-currency unit.
   * A monetary scalar must use the payoff currency. The FX surface is queried
   * at the forward FX rate implied by the asset and payoff discount curves.
   */
  fx_spot_id: Id;
  /**
   * FX volatility surface ID (required for quanto vol lookup).
   */
  fx_vol_surface_id: Id;
}
/**
 * Exchange-listed option on an arbitrary equity or equity-index futures contract.
 */
export interface EquityFutureOption {
  /**
   * Attributes for selection and reporting.
   */
  attributes?: Attributes;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs: the flat option volatility in
   * `market_quotes.implied_volatility` (required while live) and optional
   * tree-step overrides.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Caller-supplied option-on-future pricing and settlement terms.
   */
  terms: FutureOptionTerms;
}
/**
 * Exchange-listed equity or index total-return future.
 *
 * The clearing price follows the Eurex-style decomposition
 * `TRF = spot + accrued_distributions - accrued_funding + basis`, where
 * `basis = spot × spread_basis_points × 1e-4 × year_fraction(as_of, settlement)`.
 */
export interface EquityTotalReturnFuture {
  /**
   * Cumulative distribution points published by the exchange.
   */
  accrued_distributions_id: Id;
  /**
   * Cumulative funding points published by the exchange.
   */
  accrued_funding_id: Id;
  /**
   * Attributes for selection and reporting.
   */
  attributes?: Attributes;
  /**
   * Unique instrument identifier.
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
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Current underlying close or exchange-prescribed reference level.
   */
  spot_id: Id;
  /**
   * Current annualized TRF spread scalar expressed in basis points.
   */
  spread_bp_id: Id;
  /**
   * Day-count basis used to convert the annualized spread into index points.
   */
  spread_day_count: DayCount;
  /**
   * Standard listed position and lifecycle terms.
   */
  terms: ListedFutureTerms;
  /**
   * Equity or equity-index ticker.
   */
  underlying_ticker: string;
}
/**
 * Cliquet option instrument.
 */
export interface CliquetOption {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Discount curve ID for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Optional dividend-yield scalar ID.
   *
   * `Some(id)`: lookup MUST succeed (a missing or non-unitless scalar
   * returns an error). `None`: no implicit default; treated as zero
   * continuous dividend yield. Set explicitly for index underlyings.
   */
  div_yield_id?: Id | null;
  /**
   * Explicit terminal expiry date for the structure.
   */
  expiry: DateWire;
  /**
   * Global cap on sum of all period returns
   */
  global_cap: number;
  /**
   * Global floor on sum of all period returns (default 0.0)
   */
  global_floor: number;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Strike-set underlying level anchoring the first period's return.
   *
   * `None` (the default) uses the spot at the valuation date, which is only
   * correct for a new trade priced on its strike-set date. **Required for
   * seasoned trades** (any reset date strictly before `as_of`): pricing
   * errors otherwise. Do not duplicate the strike-set date inside
   * `reset_dates` when providing this — reset dates are period-end
   * observations.
   */
  initial_level?: number | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Local cap on individual period returns
   */
  local_cap: number;
  /**
   * Local floor on individual period returns (default 0.0)
   */
  local_floor: number;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount
   */
  notional: Money;
  /**
   * Observed underlying fixings for seasoned trades (date, level pairs).
   *
   * Every reset date strictly before the valuation date must have a
   * matching fixing here; pricing errors otherwise. Locked-in period
   * returns are computed deterministically from these fixings (with local
   * cap/floor applied) and only the remaining future periods are simulated.
   */
  past_fixings?: [unknown, unknown][];
  /**
   * Explicit path model selection; required to acknowledge model risk.
   */
  path_model: EquityPathModel;
  /**
   * Payoff aggregation type (default: Additive)
   */
  payoff_type?: CliquetPayoffType;
  /**
   * Reset dates for periodic return locking
   */
  reset_dates: DateWire[];
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Spot price identifier
   */
  spot_id: Id;
  /**
   * Underlying asset ticker symbol
   */
  underlying_ticker: string;
  /**
   * Volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * Range accrual instrument.
 *
 * Range accrual notes pay coupons that accrue only when a reference rate or asset
 * stays within a specified range. The accrual is proportional to the number of
 * observation dates where the underlying is within [lower_bound, upper_bound].
 *
 * # Bounds Interpretation
 *
 * The `bounds_type` field controls how `lower_bound` and `upper_bound` are interpreted:
 * - `Absolute`: Bounds are absolute price levels (e.g., 4500.0 for SPX at 4700)
 * - `RelativeToInitialSpot`: Bounds are multipliers of the initial spot (e.g., 0.95 = 95%)
 *
 * # Historical Fixings
 *
 * For mid-life valuations, use `past_observations_in_range` to specify how many past
 * observations were in range. The pricer will add this to expected future fixings.
 *
 * # Rate-Linked Underlyings: Pricing Routing
 *
 * The rate-linked fields (`index_id`, `forward_curve_id`,
 * `index_tenor`) describe the contract but are **not priceable by the
 * standalone `RangeAccrual` pricers**, which support equity/FX (GBM)
 * underlyings only and return a validation error when these fields are set.
 * Price rate-linked range accrual notes through
 * [`CallableRangeAccrual`](crate::instruments::exotics::callable_range_accrual)
 * (whose `range_accrual` field carries these [`RangeAccrualTerms`]), which
 * models the reference rate under HW1F and reconstructs the term rate per
 * observation.
 */
export interface RangeAccrual {
  /**
   * Attributes for scenario selection and grouping
   */
  attributes: Attributes;
  /**
   * How to interpret the range bounds (default: Absolute)
   */
  bounds_type?: BoundsType;
  /**
   * Coupon rate earned when in range (must be >= 0)
   */
  coupon_rate: number;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Discount curve ID for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Optional dividend-yield scalar ID
   */
  div_yield_id?: Id | null;
  /**
   * Rates forward curve that projects the observed index of a rate-linked range accrual.
   */
  forward_curve_id?: Id | null;
  /**
   * Unique instrument identifier
   */
  id: Id;
  /**
   * Rate-index identity (e.g. `USD-SOFR`) of a rate-linked range accrual.
   */
  index_id?: Id | null;
  /**
   * Contractual tenor of the observed rate index.
   */
  index_tenor?: Tenor | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Lower bound of accrual range (interpretation depends on bounds_type)
   */
  lower_bound: number;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount
   */
  notional: Money;
  /**
   * Observation dates for range checking (must be sorted ascending)
   */
  observation_dates: DateWire[];
  /**
   * Number of past observations that were in range (for mid-life valuations).
   * If None, past observations are not included in the accrual calculation.
   */
  past_observations_in_range?: number | null;
  /**
   * Optional payment date (defaults to last observation date)
   */
  payment_date?: DateWire | null;
  /**
   * Optional quanto adjustment parameters. When provided, applies a drift
   * correction for instruments whose payoff currency differs from the
   * underlying asset currency.
   */
  quanto?: QuantoSpec | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Spot price identifier
   */
  spot_id: Id;
  /**
   * Contractual accrual-period start date.
   */
  start_date: DateWire;
  /**
   * Total number of past observations (for mid-life valuations).
   * Must be provided if `past_observations_in_range` is set.
   */
  total_past_observations?: number | null;
  /**
   * Underlying asset ticker symbol
   */
  underlying_ticker: string;
  /**
   * Upper bound of accrual range (must be > lower_bound)
   */
  upper_bound: number;
  /**
   * Volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * Target Redemption Note (TARN).
 *
 * Pays periodic coupons = max(fixed_rate - floating_rate, floor) until the
 * cumulative coupon reaches a target level, at which point the note redeems
 * at par. Path-dependent due to the knockout on cumulative coupon.
 *
 * # Coupon Formula
 *
 * ```text
 * c_i = max(fixed_rate - L_i, floor)
 * ```
 *
 * where L_i is the floating rate (e.g., SOFR) for period i.
 *
 * # Target Knockout
 *
 * ```text
 * If sum(c_1, ..., c_i) >= target => redeem at par, stop paying coupons
 * ```
 *
 * The final coupon is reduced so the cumulative equals the target exactly.
 *
 * # References
 *
 * - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and
 *   Practice* (2nd ed.). Springer. Chapter 14: Exotic Derivatives. `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`
 */
export interface Tarn {
  /**
   * Attributes for scenario selection.
   */
  attributes: Attributes;
  /**
   * Floor on each period's coupon (typically 0.0).
   */
  coupon_floor: number;
  /**
   * Day count convention for coupon accrual.
   */
  day_count: DayCount;
  /**
   * Discount curve ID for PV calculations.
   */
  discount_curve_id: Id;
  /**
   * Fixed coupon rate (the "strike" rate) as a decimal annual rate (0.05 = 5%).
   */
  fixed_rate: DecimalWire;
  /**
   * Rates forward curve that projects the floating index (also the fixing-series key).
   */
  forward_curve_id: Id;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Contractual tenor of the observed floating index (e.g. 3M, 6M); must match the forward curve tenor.
   */
  index_tenor: Tenor;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount.
   */
  notional: Money;
  /**
   * Coupon payment dates, one per period, strictly ascending and after
   * `start_date`. Period `i` accrues from the previous payment date (or
   * `start_date`) to `payment_dates[i]` and fixes in advance at its start.
   */
  payment_dates: DateWire[];
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Accrual start of the first coupon period; also the first in-advance
   * fixing date.
   */
  start_date: DateWire;
  /**
   * Target cumulative coupon level (triggers early redemption).
   *
   * A zero target is allowed and represents redemption at the first
   * future coupon date without coupon accrual.
   */
  target_coupon: number;
  /**
   * Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.
   */
  vol_surface_id?: Id | null;
}
/**
 * Snowball structured note.
 *
 * The coupon in each period depends on the previous period's coupon,
 * creating a path-dependent "snowball" accumulation:
 *
 * ```text
 * c_i = max(c_{i-1} + fixed_rate - L_i, 0)
 * ```
 *
 * where L_i is the floating rate and c_0 = initial_coupon.
 *
 * If the floating rate stays low, coupons ratchet up over time.
 * If rates spike, the coupon floors at zero and must rebuild.
 *
 * # Variants
 *
 * - **Snowball**: Coupon depends on previous coupon (path-dependent)
 * - **Inverse Floater**: Coupon = fixed_rate - gearing * floating_rate
 *   (simpler, not path-dependent, but often combined with callability)
 *
 * # References
 *
 * - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models*. Chapter 14. `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`
 */
export interface Snowball {
  /**
   * Attributes.
   */
  attributes: Attributes;
  /**
   * Optional Bermudan call provision.
   */
  call_provision?: BermudanCallProvision | null;
  /**
   * Optional cap on each period coupon.
   */
  coupon_cap?: number | null;
  /**
   * Floor on each period coupon (typically 0.0).
   */
  coupon_floor: number;
  /**
   * Day count convention.
   */
  day_count: DayCount;
  /**
   * Discount curve ID.
   */
  discount_curve_id: Id;
  /**
   * Fixed rate component as a decimal annual rate (0.05 = 5%).
   */
  fixed_rate: DecimalWire;
  /**
   * Rates forward curve that projects the floating index (also the fixing-series key).
   */
  forward_curve_id: Id;
  /**
   * Multiplier on the floating fixing (1.0 for snowball, variable for inverse floater).
   */
  gearing: number;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Contractual tenor of the observed floating index (must match the forward curve tenor).
   */
  index_tenor: Tenor;
  /**
   * Initial coupon for snowball (c_0); ignored for inverse floater.
   */
  initial_coupon: number;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount.
   */
  notional: Money;
  /**
   * Coupon payment dates, one per period, strictly ascending and after
   * `start_date`. Period `i` accrues from the previous payment date (or
   * `start_date`) to `payment_dates[i]` and fixes in advance at its start.
   */
  payment_dates: DateWire[];
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Accrual start of the first coupon period; also the first in-advance
   * fixing date.
   */
  start_date: DateWire;
  /**
   * Snowball or inverse floater variant.
   */
  variant: SnowballVariant;
  /**
   * Optional normal-vol surface used to infer HW1F short-rate σ for stress scenarios.
   */
  vol_surface_id?: Id | null;
}
/**
 * Bermudan call provision for callable exotics.
 *
 * Allows the issuer to terminate the note on specified call dates
 * at a specified call price (typically par). Currently consumed by the
 * Callable Range Accrual note (PRDC is not implemented, and the Snowball
 * pricer rejects callable provisions).
 *
 * # Fields
 *
 * - `call_dates`: Sorted ascending dates on which the issuer may call.
 * - `price_pct_of_par`: Redemption price in percent of par (`100.0` = par,
 *   `102.0` = callable at 102).
 * - `lockout_end`: Optional end of the no-call period; call dates on or
 *   before it are not exercisable (exclusive bound, matching
 *   `BermudanSchedule.lockout_end` on Bermudan swaptions).
 */
export interface BermudanCallProvision {
  /**
   * Dates on which the issuer can call (must be sorted ascending).
   */
  call_dates: DateWire[];
  /**
   * End of the no-call (lockout) period. Call dates on or before this
   * date are dropped (exclusive bound); `None` means no lockout.
   */
  lockout_end?: DateWire | null;
  /**
   * Redemption price in percent of par (`100.0` = par). The pricer pays
   * `notional * price_pct_of_par / 100` at exercise.
   */
  price_pct_of_par: number;
}
/**
 * CMS Spread Option.
 *
 * Option on the spread between two CMS rates of different tenors.
 *
 * ```text
 * Payoff = max(CMS_long - CMS_short - strike, 0) * notional    [for a call]
 * Payoff = max(strike - (CMS_long - CMS_short), 0) * notional   [for a put]
 * ```
 *
 * Typically: long tenor = 10Y or 30Y CMS, short tenor = 2Y CMS.
 *
 * # Pricing Approach
 *
 * 1. Each CMS rate has SABR marginal distribution (reuses CMS option SABR calibration)
 * 2. Joint distribution via Gaussian copula with rank correlation
 * 3. CMS convexity adjustment applied to each leg via static replication
 *
 * # References
 *
 * - Hagan, P. S. (2003). "Convexity Conundrums." *Wilmott Magazine*. `docs/REFERENCES.md#hagan-2003-cms-convexity`
 * - Antonov, A., Konikov, M., & Spector, M. (2013). "SABR Spreads." *Risk*. `docs/REFERENCES.md#hagan-2002-sabr`
 */
export interface CmsSpreadOption {
  /**
   * Attributes.
   */
  attributes: Attributes;
  /**
   * Gaussian-copula correlation between the two CMS rates, a decimal in `[-1, 1]`.
   */
  correlation: CorrelationWire;
  /**
   * Day count convention.
   */
  day_count: DayCount;
  /**
   * Discount curve ID.
   */
  discount_curve_id: Id;
  /**
   * Option expiry date.
   */
  expiry: DateWire;
  /**
   * Forward curve ID (for swap rate projection).
   */
  forward_curve_id: Id;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Rate-index convention-registry key of the underlying CMS swaps (e.g. `EUR-ESTR-OIS`).
   *
   * When set, provides default values for the fixed/float frequency and
   * day count. Individual fields still override the convention when set.
   */
  index_id?: Id | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Long CMS tenor (e.g., 10Y).
   */
  long_cms_tenor: Tenor;
  /**
   * Swaption volatility surface for long tenor.
   */
  long_vol_surface_id: Id;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount.
   */
  notional: Money;
  /**
   * Call or put on the spread `CMS_long - CMS_short`: a call pays
   * `max(spread - strike, 0)`, a put pays `max(strike - spread, 0)`.
   */
  option_type: OptionType;
  /**
   * Payment date (may differ from expiry).
   */
  payment_date: DateWire;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Short CMS tenor (e.g., 2Y).
   */
  short_cms_tenor: Tenor;
  /**
   * Swaption volatility surface for short tenor.
   */
  short_vol_surface_id: Id;
  /**
   * Strike on the CMS spread as a decimal rate (0.005 = 50bp).
   */
  strike: DecimalWire;
  /**
   * Fixed leg day count of the underlying CMS swaps (overrides convention).
   */
  swap_fixed_day_count?: DayCount | null;
  /**
   * Fixed leg frequency of the underlying CMS swaps (overrides convention).
   */
  swap_fixed_frequency?: Tenor | null;
  /**
   * Floating leg day count of the underlying CMS swaps (overrides convention).
   */
  swap_float_day_count?: DayCount | null;
  /**
   * Floating leg frequency of the underlying CMS swaps (overrides convention).
   */
  swap_float_frequency?: Tenor | null;
}
/**
 * Equity total-return swap exchanging price and net dividend return against
 * floating-rate financing plus a contractual spread.
 *
 * Seasoned trades require the observed level at the current period start in
 * `past_fixings`, except that `initial_level` may anchor the first period.
 *
 * # Construction
 */
export interface EquityTotalReturnSwap {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Optional discrete cash dividends `(ex_date, amount)` for the underlying.
   *
   * When non-empty, pricing uses explicit period dividend pass-through and does
   * not add continuous-yield dividend return to avoid double counting.
   */
  discrete_dividends?: [unknown, unknown][];
  /**
   * Settlement timing for explicit manufactured dividends.
   */
  dividend_settlement: TrsDividendSettlement;
  /**
   * Dividend withholding tax rate for net return calculation.
   *
   * Specifies the fraction of dividends withheld for tax (e.g., 0.15 for 15% withholding).
   * When set to 0.0 (default), the TRS passes through 100% of dividends (gross return).
   * When set to a positive value, the dividend return component is reduced:
   * ```text
   * net_dividend_return = gross_dividend_return × (1 - dividend_tax_rate)
   * ```
   *
   * # Market Context
   *
   * Withholding tax varies by jurisdiction and investor domicile:
   * - US qualified dividends: typically 0% for domestic investors
   * - US non-qualified: up to 30% for foreign investors (varies by treaty)
   * - European: varies by country (15-30% typical)
   */
  dividend_tax_rate?: number;
  /**
   * Financing leg specification (curves, spread, day count).
   */
  financing_leg: FinancingLegSpec;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Optional first-period start level.
   *
   * Pricing uses this only when the first period is in progress and no
   * matching entry exists in `past_fixings`. Future periods use live spot.
   */
  initial_level?: number | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Optional OTC margin specification for VM/IM.
   *
   * Equity TRS use SIMM equity bucket for margin calculation.
   */
  margin_spec?: OtcMarginSpec | null;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount for the swap.
   */
  notional: Money;
  /**
   * Observed underlying levels at past reset (period-start) dates.
   *
   * For a seasoned TRS valued inside a return period, the total-return leg
   * must anchor the current period to the level *observed* at the period
   * start so the realized spot move enters the PV (equity delta). Provide
   * `(reset_date, level)` pairs for every period-start date on or before
   * the valuation date; the first period may use `initial_level` instead.
   * Pricing errors when the current period's start level is unavailable.
   */
  past_fixings?: [unknown, unknown][];
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Schedule specification (payment dates and frequency).
   */
  schedule: TrsScheduleSpec;
  /**
   * Trade side (receive/pay total return).
   */
  side: PayReceive;
  /**
   * Underlying equity parameters (spot ID, dividend yield, contract size).
   */
  underlying: EquityUnderlyingParams;
}
/**
 * Specification for TRS financing legs
 */
export interface FinancingLegSpec {
  /**
   * Rate-compounding convention of the financing rate.
   *
   * Defaults to `simple` (one term forward per period, e.g. 3M Term SOFR or
   * EURIBOR). Use a `compounded_*` variant for SOFR/SONIA/€STR
   * overnight-funded TRS so the financing rate captures the daily
   * compounding convexity (typically 12–15 bp of rate on an upward curve).
   * `simple_average` is rejected.
   */
  compounding?: FloatingLegCompounding;
  /**
   * Day count convention for accrual calculations
   */
  day_count: DayCount;
  /**
   * Discount curve identifier for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Forward curve identifier (e.g., USD-SOFR-3M)
   */
  forward_curve_id: Id;
  /**
   * Spread in basis points over the floating rate (e.g., 50 = 50bp = 0.5%)
   */
  spread_bp: DecimalWire;
}
/**
 * Schedule specification for TRS payment periods.
 *
 * Defines the payment schedule and frequency for both legs of the TRS.
 * This is shared between equity and fixed income TRS instruments.
 */
export interface TrsScheduleSpec {
  /**
   * End date for the TRS leg.
   */
  end: DateWire;
  /**
   * Schedule parameters (frequency, day count, business_day_convention, calendar, stub).
   */
  params: cashflows.ScheduleParams;
  /**
   * Start date for the TRS leg.
   */
  start: DateWire;
}
/**
 * Equity underlying parameters for options and equity-linked swaps.
 */
export interface EquityUnderlyingParams {
  /**
   * Contract size (shares per contract)
   */
  contract_size: number;
  /**
   * Base currency for pricing
   */
  currency: Currency;
  /**
   * Optional dividend yield identifier
   */
  div_yield_id?: Id | null;
  /**
   * Spot price identifier in market data
   */
  spot_id: Id;
  /**
   * Underlying ticker/identifier
   */
  ticker: string;
}
/**
 * Fixed-income index total-return swap priced with a carry analytic.
 *
 * For each scheduled period, the total-return leg uses
 * `exp(y × accrual_fraction) - 1`, where `y` is an annual continuously
 * compounded decimal yield and the accrual fraction uses the schedule day
 * count. Both legs are discounted with `financing.discount_curve_id`; the
 * financing leg projects `financing.forward_curve_id` plus `spread_bp`.
 * Receive-total-return value is `PV(total return) - PV(financing)`;
 * pay-total-return value reverses that sign.
 *
 * # Model limitations
 *
 * This deterministic carry analytic omits roll-down, underlying rate/spread
 * mark-to-market, stochastic credit, constituent decomposition, early
 * termination, and bespoke fees. The period in progress on the valuation
 * date is valued from the live index level (`underlying.index_id`) against
 * `initial_level`, then carried at the index yield to period end; a period
 * that has ended but is not yet paid stays in the PV at its projected carry.
 * Cashflow-schedule APIs
 * return payment dates with zero amounts; use
 * [`Self::pv_total_return_leg`] and [`Self::pv_financing_leg`] for projected
 * leg values.
 *
 * # Construction
 */
export interface FiIndexTotalReturnSwap {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Financing leg specification (curves, spread, day count).
   */
  financing_leg: FinancingLegSpec;
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Index level at the reset of the return period in progress, in the
   * index's own units.
   *
   * Required when the valuation date falls inside a return period; the
   * realized return to date is `index_level / initial_level`. Unused for
   * unseasoned trades. Must be positive and finite when set.
   */
  initial_level?: number | null;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Optional OTC margin specification for VM/IM.
   *
   * Fixed income index TRS use duration-based margin calculations.
   */
  margin_spec?: OtcMarginSpec | null;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Notional amount for the swap.
   */
  notional: Money;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Schedule specification (payment dates and frequency).
   */
  schedule: TrsScheduleSpec;
  /**
   * Trade side: `receive` receives the total-return leg and pays financing;
   * `pay` pays the total return and receives financing.
   */
  side: PayReceive;
  /**
   * Underlying index parameters (index ID, yield, duration, base currency).
   */
  underlying: IndexUnderlyingParams;
}
/**
 * Index underlying parameters for total return swaps and index-linked instruments.
 */
export interface IndexUnderlyingParams {
  /**
   * Currency the index (and so the swap notional) is denominated in.
   */
  currency: Currency;
  /**
   * Market scalar identifier for signed index duration in years. Required when
   * requesting FI TRS duration risk; the scalar must be unitless and finite. No duration is inferred from index name or maturity.
   */
  duration_id?: Id | null;
  /**
   * Index identifier (e.g., "CDX.IG", "HY.BOND.INDEX")
   */
  index_id: Id;
  /**
   * Optional yield curve/scalar identifier for carry calculation
   */
  yield_id?: Id | null;
}
/**
 * Private markets fund investment instrument.
 *
 * Models a private equity, private credit, or alternative fund with a
 * cashflow waterfall that determines LP/GP allocation. Future LP cashflows
 * are selected relative to the caller's valuation date and optionally
 * discounted with `discount_curve_id`.
 */
export interface PrivateMarketsFund {
  /**
   * Attributes for scenario selection and tagging.
   */
  attributes?: Attributes;
  /**
   * Functional currency of the fund.
   */
  currency: Currency;
  /**
   * Optional discount curve for future LP cashflows.
   *
   * When `None`, future cashflows are included undiscounted relative to the
   * caller's valuation date.
   */
  discount_curve_id?: Id | null;
  /**
   * Time-ordered list of fund events (contributions, proceeds, distributions).
   */
  events: FundEvent[];
  /**
   * Unique instrument identifier.
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
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Unrealized net asset value attributable to the LP, stated as of the
   * fund's valuation date.
   *
   * Holder-view residual value : the fund's present
   * value is the PV of LP cashflows strictly after the valuation date plus
   * this NAV. When `None`, the residual value is zero — a fully realized
   * fund prices to ~0.
   *
   * **Do not combine a NAV mark with forecast distribution events after
   * the valuation date.** The NAV mark already embeds the value of the
   * fund's future realizations, so supplying both double counts them:
   * model either realized history + NAV mark (marked fund), or realized
   * history + projected future events with no NAV (cashflow projection).
   * The NAV is added undiscounted, taken as stated at the valuation date.
   */
  unrealized_nav?: Money | null;
  /**
   * Waterfall specification defining LP/GP allocation tiers.
   */
  waterfall_spec: PeFundWaterfallSpec;
}
/**
 * Single fund cash flow event.
 */
export interface FundEvent {
  /**
   * Amount (positive for all event types, sign determined by kind)
   */
  amount: Money;
  /**
   * Date of the event
   */
  date: DateWire;
  /**
   * Deal identifier. Required for every event in an American-style
   * deal-by-deal waterfall.
   */
  deal_id?: string | null;
  /**
   * Type of event
   */
  kind: FundEventKind;
}
/**
 * Complete waterfall specification.
 */
export interface PeFundWaterfallSpec {
  /**
   * Catch-up mode
   */
  catch_up_mode?: CatchUpMode;
  /**
   * Clawback specification; `None` means no clawback (presence is the switch)
   */
  clawback?: ClawbackSpec | null;
  /**
   * Day count for LP/fund IRR year fractions and hurdle compounding
   * `(1 + hurdle_irr)^years` (default Act/365F)
   */
  day_count?: DayCount;
  /**
   * Allocation style (European vs American)
   */
  style: WaterfallStyle;
  /**
   * Ordered sequence of waterfall tranches
   */
  tranches: PeFundWaterfallTranche[];
}
/**
 * Clawback specification for GP carry reconciliation.
 */
export interface ClawbackSpec {
  /**
   * Optional share of GP carry held back until settlement, as a decimal
   * fraction in `[0, 1]` (`0.2` = 20%)
   */
  holdback_decimal?: number | null;
  /**
   * When to settle clawback
   */
  settle_on: ClawbackSettle;
}
/**
 * Real estate asset valuation instrument.
 *
 * Supports DCF (explicit NOI schedule) and direct capitalization valuation.
 */
export interface RealEstateAsset {
  /**
   * Acquisition (closing) cost line items in instrument currency, as
   * positive outflow magnitudes deducted at `as_of` in DCF valuation.
   */
  acquisition_costs?: Money[];
  /**
   * Optional appraisal override value.
   */
  appraisal_value?: Money | null;
  /**
   * Attributes for scenario selection and tagging
   */
  attributes?: Attributes;
  /**
   * Capitalization rate for direct cap (annualized).
   */
  cap_rate?: number | null;
  /**
   * Capital expenditure schedule (date, amount). Values are treated as **positive outflows**.
   *
   * When present, cashflows are valued as `NOI - CapEx` (unlevered net cash flow).
   */
  capex_schedule?: [unknown, unknown][] | null;
  /**
   * Currency for valuation.
   */
  currency: Currency;
  /**
   * Day count convention for year fractions.
   */
  day_count: DayCount;
  /**
   * Discount rate for DCF (annualized).
   */
  discount_rate?: number | null;
  /**
   * Optional disposition cost percentage applied to terminal value.
   *
   * A value of `0.02` represents 2% selling costs. Must be in \([0, 1)\).
   */
  disposition_cost_decimal?: number | null;
  /**
   * Optional detailed disposition cost line items (positive outflows) deducted from terminal proceeds.
   */
  disposition_costs?: Money[];
  /**
   * Unique instrument identifier.
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
   * Net operating income schedule (date, amount).
   */
  noi_schedule: [unknown, unknown][];
  /**
   * Optional property type classification (for reporting).
   */
  property_type?: RealEstatePropertyType | null;
  /**
   * Optional purchase price (useful for IRR / cap rate metrics).
   */
  purchase_price?: Money | null;
  /**
   * Optional sale/exit date that truncates the DCF horizon.
   *
   * When set, DCF only values unlevered flows up to and including `sale_date`.
   * Terminal proceeds (if configured) are realized on `sale_date`.
   */
  sale_date?: DateWire | null;
  /**
   * Optional explicit gross sale price (terminal proceeds), before disposition costs.
   *
   * When set, this takes precedence over `terminal_cap_rate` for terminal proceeds.
   */
  sale_price?: Money | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Optional stabilized NOI override for direct cap.
   */
  stabilized_noi?: number | null;
  /**
   * Optional terminal cap rate for DCF (uses last NOI).
   */
  terminal_cap_rate?: number | null;
  /**
   * Optional terminal growth rate used to project `NOI_{N+1}` for exit valuation.
   *
   * Market convention for exit-cap terminal value is \(TV = NOI_{N+1} / cap\_rate\_exit\).
   * When not provided, defaults to 0 (uses last NOI as-is).
   * Validation range is \([-100\%, 20\%]\) to guard against configuration errors.
   */
  terminal_growth_rate?: number | null;
  /**
   * Valuation method (DCF or DirectCap).
   */
  valuation_method: RealEstateValuationMethod;
}
/**
 * Discounted Cash Flow instrument for corporate valuation.
 *
 * DCF values a company by discounting projected free cash flows and terminal value.
 * The equity value is calculated as: Enterprise Value - Net Debt (or structured bridge).
 *
 * # Equity Bridge
 *
 * [`equity_bridge`](Self::equity_bridge) is the single EV-to-equity channel; a
 * flat net-debt deduction sets only its `total_debt` and `cash`.
 *
 * # Mid-Year Convention
 *
 * When [`mid_year_convention`](Self::mid_year_convention) is `true`, cash flows are
 * discounted at `t` minus half the average flow spacing instead of `t` years,
 * reflecting the assumption that cash flows arrive mid-period (standard IB/PE
 * practice). For annual grids this is the classic `(t - 0.5)`; for sub-annual
 * grids the shift is half a period. `ExitMultiple` terminal values always
 * discount at the full horizon (a point-in-time sale price).
 *
 * # Valuation Discounts
 *
 * Private company valuations can apply DLOM, DLOC, and other discounts via
 * [`valuation_discounts`](Self::valuation_discounts). These discounts apply
 * after the EV-to-equity bridge, so the reported enterprise value remains a
 * pre-discount enterprise value and does not equal discounted equity value
 * plus net debt.
 */
export interface DiscountedCashFlow {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Currency for all cashflows.
   */
  currency: Currency;
  /**
   * Dilutive securities (options, warrants, RSUs, convertibles) for treasury stock method.
   */
  dilution_securities?: DilutionSecurity[];
  /**
   * Equity bridge from enterprise value to equity value (debt, cash,
   * preferred equity, minority interest, non-operating assets and other
   * adjustments), in instrument currency.
   */
  equity_bridge: EquityBridge;
  /**
   * Explicit period free cash flows (date, amount pairs).
   */
  flows: [unknown, unknown][];
  /**
   * Unique identifier for the DCF.
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
   * Discount curve identifier, used for risk attribution only.
   *
   * Discounting always uses [`wacc`](Self::wacc) via `(1 + wacc)^{-t}`
   * regardless of whether this curve is loaded (the
   * previous behavior silently switched risky flows to risk-free curve
   * discounting). WACC sensitivity is exposed separately as `dcf::wacc01`.
   * Mid-year discounting convention (default: `false` = end-of-period).
   *
   * When `true`, each flow is discounted at `t` minus half the average
   * flow spacing instead of `t` (the classic `t - 0.5` for annual
   * grids; half a period for sub-annual grids), reflecting the
   * assumption that cash flows arrive mid-period. This is the standard
   * convention in IB/PE practice (Koller et al.). Exit-multiple
   * terminal values are not shifted (point-in-time sale at the horizon).
   */
  mid_year_convention?: boolean;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Basic shares outstanding for per-share value calculation.
   */
  shares_outstanding?: number | null;
  /**
   * Annualized terminal flow used by growth-perpetuity terminal values.
   *
   * Gordon Growth and H-Model capitalize an *annual* flow with annual
   * WACC and growth rates. When the explicit `flows` grid is sub-annual
   * (e.g. quarterly), the last flow is a period flow, not an annual one;
   * set this to the trailing-twelve-month aggregate of the final
   * explicit year so the terminal value is computed on a consistent
   * annual basis (Koller et al., Damodaran). When `None`, the last
   * explicit flow is used directly (correct for annual grids).
   *
   * Ignored by `ExitMultiple` terminal values.
   */
  terminal_flow_override?: number | null;
  /**
   * Terminal value specification.
   */
  terminal_value: TerminalValueSpec;
  /**
   * Valuation date (as-of date for the DCF).
   */
  valuation_date: DateWire;
  /**
   * Private company valuation discounts (DLOM, DLOC).
   */
  valuation_discounts?: ValuationDiscounts | null;
  /**
   * Weighted Average Cost of Capital (discount rate).
   */
  wacc: number;
}
/**
 * A dilutive security for the treasury stock method.
 *
 * Used to compute diluted shares outstanding from options, warrants,
 * RSUs, or convertible instruments.
 */
export interface DilutionSecurity {
  /**
   * Exercise or strike price per share (0.0 for RSUs/convertibles with no cost).
   */
  exercise_price: number;
  /**
   * Descriptive name (e.g., "Employee Stock Options").
   */
  name: string;
  /**
   * Number of shares issuable upon exercise/conversion.
   */
  quantity: number;
}
/**
 * Structured equity bridge for converting Enterprise Value to Equity Value.
 *
 * Standard professional bridge:
 * ```text
 * Equity = EV - Total Debt + Cash - Preferred Equity - Minority Interest
 *          + Non-Operating Assets + Σ(other adjustments)
 * ```
 *
 * Every [`DiscountedCashFlow`] carries one; a flat net-debt deduction is
 * written as `EquityBridge { total_debt, cash, ..Default::default() }`.
 */
export interface EquityBridge {
  /**
   * Cash and cash equivalents.
   */
  cash?: number;
  /**
   * Non-controlling (minority) interests.
   */
  minority_interest?: number;
  /**
   * Non-operating assets (excess cash, investments, real estate, etc.).
   */
  non_operating_assets?: number;
  /**
   * Named adjustments (e.g., unfunded pension, contingent liabilities).
   * Positive values increase equity; negative values decrease it.
   */
  other_adjustments?: [unknown, unknown][];
  /**
   * Preferred stock at liquidation preference.
   */
  preferred_equity?: number;
  /**
   * Total interest-bearing debt.
   */
  total_debt?: number;
}
/**
 * Valuation discounts for private company equity.
 *
 * Applied multiplicatively after the equity bridge:
 * ```text
 * FMV = Equity Value × (1 - DLOC) × (1 - DLOM) × (1 - other_discount)
 * ```
 *
 * The enterprise value reported by a DCF remains pre-discount. When discounts
 * are present, `enterprise_value - net_debt` reconciles to pre-discount
 * equity value, not to the discounted fair-market equity value.
 */
export interface ValuationDiscounts {
  /**
   * Discount for Lack of Control (0.0–1.0, e.g., 0.20 for 20%).
   */
  dloc?: number | null;
  /**
   * Discount for Lack of Marketability (0.0–1.0, e.g., 0.25 for 25%).
   */
  dlom?: number | null;
  /**
   * Additional discount (0.0–1.0).
   */
  other_discount?: number | null;
}
/**
 * Callable Range Accrual.
 *
 * Extends the existing range accrual concept with a Bermudan call provision
 * allowing the issuer to terminate early on specified call dates.
 *
 * The call decision interacts with the range accrual feature: the issuer
 * will call when the expected future value of remaining range accrual
 * coupons exceeds the call price (par). Pricing requires backward
 * induction (LSMC or HW tree) combined with forward range accrual
 * coupon simulation.
 *
 * # Pricing
 *
 * - **LSMC**: Simulate paths with HW1F short rate model. At each call
 *   date, compute continuation value via regression. Exercise if
 *   the call amount `notional * price_pct_of_par / 100` is below the
 *   continuation value.
 * - **HW Tree**: Build trinomial tree, attach range accrual cashflows
 *   at each node, apply backward induction with call decision.
 */
export interface CallableRangeAccrual {
  /**
   * Attributes.
   */
  attributes: Attributes;
  /**
   * Bermudan call provision.
   */
  call_provision: BermudanCallProvision;
  /**
   * Unique instrument identifier.
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
   * Range accrual contract terms (no identity, attributes or overrides of their own).
   */
  range_accrual: RangeAccrualTerms;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
}
/**
 * Contract terms of a range accrual note, without identity or pricing overrides.
 *
 * [`RangeAccrual`] flattens these terms into its own payload, and
 * [`CallableRangeAccrual`](crate::instruments::exotics::callable_range_accrual::CallableRangeAccrual)
 * nests them under `range_accrual`, so the callable note has exactly one
 * `id`, one `attributes` map and one set of pricing overrides.
 */
export interface RangeAccrualTerms {
  /**
   * How to interpret the range bounds (default: Absolute)
   */
  bounds_type?: BoundsType;
  /**
   * Coupon rate earned when in range (must be >= 0)
   */
  coupon_rate: number;
  /**
   * Day count convention
   */
  day_count: DayCount;
  /**
   * Discount curve ID for present value calculations
   */
  discount_curve_id: Id;
  /**
   * Optional dividend-yield scalar ID
   */
  div_yield_id?: Id | null;
  /**
   * Rates forward curve that projects the observed index of a rate-linked range accrual.
   */
  forward_curve_id?: Id | null;
  /**
   * Rate-index identity (e.g. `USD-SOFR`) of a rate-linked range accrual.
   */
  index_id?: Id | null;
  /**
   * Contractual tenor of the observed rate index.
   */
  index_tenor?: Tenor | null;
  /**
   * Lower bound of accrual range (interpretation depends on bounds_type)
   */
  lower_bound: number;
  /**
   * Notional amount
   */
  notional: Money;
  /**
   * Observation dates for range checking (must be sorted ascending)
   */
  observation_dates: DateWire[];
  /**
   * Number of past observations that were in range (for mid-life valuations).
   * If None, past observations are not included in the accrual calculation.
   */
  past_observations_in_range?: number | null;
  /**
   * Optional payment date (defaults to last observation date)
   */
  payment_date?: DateWire | null;
  /**
   * Optional quanto adjustment parameters. When provided, applies a drift
   * correction for instruments whose payoff currency differs from the
   * underlying asset currency.
   */
  quanto?: QuantoSpec | null;
  /**
   * Spot price identifier
   */
  spot_id: Id;
  /**
   * Contractual accrual-period start date.
   */
  start_date: DateWire;
  /**
   * Total number of past observations (for mid-life valuations).
   * Must be provided if `past_observations_in_range` is set.
   */
  total_past_observations?: number | null;
  /**
   * Underlying asset ticker symbol
   */
  underlying_ticker: string;
  /**
   * Upper bound of accrual range (must be > lower_bound)
   */
  upper_bound: number;
  /**
   * Volatility surface ID
   */
  vol_surface_id: Id;
}
/**
 * Bond future instrument.
 *
 * A standardized contract with a basket of eligible deliverables. The short
 * chooses the bond to deliver, commonly the cheapest-to-deliver (CTD) bond.
 *
 * # Contract Mechanics
 *
 * - **Conversion factors**: Supplied per deliverable, preferably from the exchange.
 * - **CTD resolution**: Explicit `ctd_bond_id`, then embedded `ctd_bond.id`,
 *   then the sole basket member. A larger basket requires an explicit or embedded CTD.
 * - **CTD analysis**: The `determine_ctd*` helpers return ranked candidates but
 *   do not mutate the selected CTD. [`crate::instruments::Instrument::value`]
 *   marks the caller-supplied CTD only; refresh it daily with
 *   [`Self::determine_ctd_by_implied_repo`] when the basket can switch.
 * - **Invoice price**: `(Futures Price × Conversion Factor) + Accrued Interest`.
 */
export interface BondFuture {
  /**
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Contract specifications (standard coupon, standard maturity, repo day count)
   */
  contract_specs: BondFutureSpecs;
  /**
   * Optional embedded CTD bond definition.
   *
   * Model-price and NPV paths require this definition because
   * `MarketContext` contains market data, not an instrument registry.
   * Identifier-only CTD analysis and conversion-factor lookup do not require it.
   */
  ctd_bond?: Bond | null;
  /**
   * Selected cheapest-to-deliver (CTD) bond identifier.
   *
   * Resolution is deterministic: this explicit identifier takes precedence,
   * followed by `ctd_bond.id`, then the sole basket member. A larger basket
   * with neither form of selection fails validation. The resolved identifier
   * must be in `deliverable_basket`; an embedded bond must have the same ID.
   *
   * [`Self::determine_ctd`] ranks clean prices by gross basis;
   * [`Self::determine_ctd_by_implied_repo`] ranks by highest implied
   * repo after coupon income and time to delivery. These helpers return a
   * candidate and do not update this field.
   */
  ctd_bond_id?: Id | null;
  /**
   * Basket of deliverable bonds with conversion factors
   *
   * @minItems 1
   */
  deliverable_basket: [DeliverableBond, ...DeliverableBond[]];
  /**
   * First delivery date
   */
  delivery_start: DateWire;
  /**
   * Financing/discount curve identifier.
   *
   * Used to carry the CTD bond forward to the delivery date (the cost of
   * financing the position) whenever no explicit [`repo_curve_id`](Self::repo_curve_id)
   * is set. Must be provisionable as a discount curve in the market context.
   */
  discount_curve_id: Id;
  /**
   * Unique identifier for the contract
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
   * Optional repo/financing curve identifier.
   *
   * When set, this curve is used for financing/carry calculations instead
   * of `discount_curve_id`. This allows capturing repo specials, where
   * specific collateral (e.g., on-the-run Treasuries) trades at rates
   * different from the general funding curve.
   *
   * If `None`, the `discount_curve_id` is used for financing calculations.
   */
  repo_curve_id?: Id | null;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Standard listed position and lifecycle terms. `terms.multiplier` is the
   * currency value of one full price point (per-contract face / 100),
   * `terms.entry_price` the trade price per 100 face, `terms.last_trading_date`
   * the last trading day and `terms.settlement_date` the last delivery date.
   */
  terms: ListedFutureTerms;
}
/**
 * Contract specifications for bond futures.
 *
 * Defines the notional bond parameters used for conversion factor
 * calculations and the implied-repo day count. Contract size lives on the
 * future's `terms`: `terms.multiplier` is the currency value of one full
 * price point, i.e. one hundredth of the per-contract face (1,000 for a
 * $100,000 UST 10Y contract).
 *
 * Delivery timing is carried by the future's explicit `delivery_start` /
 * `terms.settlement_date` and the caller-supplied invoice settlement date, so
 * the spec holds no settlement lag or holiday calendar.
 */
export interface BondFutureSpecs {
  /**
   * Day-count convention for implied repo rate annualization.
   *
   * Bond-future repo supports `act_360` and `act_365f`.
   */
  repo_day_count?: RepoDayCountWire;
  /**
   * Standard coupon rate for conversion factor calculation (e.g., 0.06 for 6%)
   */
  standard_coupon: number;
  /**
   * Standard maturity in years for conversion factor calculation
   */
  standard_maturity_years: number;
}
/**
 * An eligible deliverable and the conversion factor used by pricing.
 *
 * Pricing and CTD helpers consume the supplied positive factor; they do not
 * recalculate it. Prefer the exchange-published value for the security and
 * delivery month. For CME/CBOT contracts, callers can use
 * [`super::BondFuturePricer::calculate_conversion_factor`] to calculate a
 * factor and store the result here.
 */
export interface DeliverableBond {
  /**
   * Identifier of the deliverable bond
   */
  bond_id: Id;
  /**
   * Positive conversion factor consumed by invoice, basis, and pricing calculations.
   */
  conversion_factor: PositiveF64Wire;
}
/**
 * Unified structured credit instrument representation.
 *
 * This single type handles CLO, ABS, CMBS, and RMBS instruments using
 * composition for deal-specific differences.
 */
export interface StructuredCredit {
  /**
   * Attributes for scenario selection.
   */
  attributes: Attributes;
  /**
   * Business day convention for tranche payments (defaults to
   * ModifiedFollowing).
   */
  business_day_convention?: BusinessDayConvention | null;
  /**
   * Optional payment calendar identifier for schedule adjustments.
   */
  calendar_id?: Id | null;
  /**
   * Assumed optional redemption for price-to-call analytics. A deal-scope
   * call liquidates the collateral at [`Self::liquidation_price_pct`] on
   * the first payment date at or after the call date and redeems every
   * note at the call price, ending the projection; a tranche-scope call
   * leaves the deal's cashflows unchanged and only truncates that class's
   * `*_to_call` metrics. `TrancheMetrics` always reports the to-maturity
   * figures (projected without the call) next to the twins.
   */
  call_assumption?: CallAssumption | null;
  /**
   * Optional credit-card master-trust portfolio model. Asset and rep-line
   * pools only: the pool is the investor interest in the receivables, the
   * spec's payment rate, portfolio yield and charge-off rate replace the
   * prepayment, coupon and default assumptions. See [`CardPortfolioSpec`].
   */
  card?: CardPortfolioSpec | null;
  /**
   * Clean-up call pool factor threshold as a decimal fraction of the
   * original balance (`0.10` = 10%).
   *
   * When the pool factor (current balance / original balance) drops below
   * this threshold, the deal is optionally redeemed and all outstanding
   * tranche balances are returned. Industry standard: typically 10%.
   *
   * Set to `None` to disable clean-up call (default).
   */
  cleanup_call_decimal?: number | null;
  /**
   * Key dates.
   * Deal closing date (issuance).
   */
  closing_date: DateWire;
  /**
   * Optional correlation structure for stochastic modeling.
   */
  correlation_structure?: CorrelationStructure | null;
  /**
   * Collateral valuation rules for the OC tests: rating haircuts, the
   * value carried for defaulted collateral, the excess-CCC bucket and
   * discount obligations. `None` values performing collateral at par with
   * defaulted collateral at its modeled recovery. Attached to the template
   * waterfall by [`Self::create_waterfall`], and to a custom
   * [`Self::waterfall`] that carries no rules of its own.
   */
  coverage_rules?: CoverageRules | null;
  /**
   * Overcollateralization / interest-coverage tests evaluated each period.
   *
   * Each test names the tested class, its kind and level, and what a
   * failure does with the diverted interest. The synthesized template
   * places each test as a [`PaymentType::CoverageTest`] tier right after
   * the interest tier of its [`CoverageTestSpec::placement_tranche`]
   * (CLO/CBO: per-class interest tiers; ABS/RMBS/CMBS: after the single
   * interest tier, so only residual cash turbos). A custom
   * [`Self::waterfall`] receives them the same way. Only cash ranked below
   * the test position can be diverted. Empty (the default) means no
   * coverage tests run.
   */
  coverage_triggers?: CoverageTestSpec[];
  /**
   * Deal metadata (counterparties, identifiers).
   */
  deal_metadata?: Metadata;
  /**
   * Deal classification (ABS/CLO/CMBS/RMBS).
   */
  deal_type: DealType;
  /**
   * Default model specification.
   */
  default_spec?: cashflows.DefaultModelSpec;
  /**
   * Optional roll-rate delinquency model with servicer advancing and loan
   * modification. Asset and rep-line pools only: the default model then
   * feeds the first delinquency bucket and only the roll out of the last
   * bucket charges off. See [`DelinquencyModel`].
   */
  delinquency?: DelinquencyModel | null;
  /**
   * Discount curve for valuation.
   */
  discount_curve_id: Id;
  /**
   * Senior transaction fees paid ahead of every note.
   *
   * `None` (the default) skips the fee tier. Use [`Self::with_standard_fees`]
   * to apply the deal-type calibration from `types/constants.rs`.
   */
  fees?: DealFees | null;
  /**
   * First payment date to tranches.
   */
  first_payment_date: DateWire;
  /**
   * Payment frequency for the structure.
   */
  frequency: Tenor;
  /**
   * Interest rate swaps settled through the waterfall: net receipts join
   * interest proceeds, net payments rank as senior or junior fees. See
   * [`HedgeSwap`].
   */
  hedge_swaps?: HedgeSwap[];
  /**
   * Unique instrument identifier.
   */
  id: Id;
  /**
   * Instrument-owned pricing inputs.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Price at which the collateral is realized when the deal is called or
   * cleaned up, as a percent of par (`None` = par). The clean-up call is
   * only exercised when the liquidation proceeds, pending recoveries and
   * every cash account together cover the notes' claims.
   */
  liquidation_price_pct?: number | null;
  /**
   * How collateral losses reach the note balances.
   *
   * `None` (the default) selects the market convention for the deal type
   * via [`LossAllocationPolicy::default_for`]: realized-loss write-downs
   * for RMBS and CMBS, par-preserving balances for CLO/CBO/ABS/cards. Set
   * explicitly to override; see [`Self::effective_loss_allocation`].
   */
  loss_allocation?: LossAllocationPolicy | null;
  /**
   * When a collateral loss is booked: at default or when the defaulted
   * loan liquidates after the recovery lag.
   *
   * `None` (the default) selects the market convention for the deal type
   * via [`LossRecognition::default_for`]: at liquidation for RMBS and
   * CMBS, at default for CLO/CBO/ABS/cards. Set explicitly to override;
   * see [`Self::effective_loss_recognition`].
   */
  loss_recognition?: LossRecognition | null;
  /**
   * Market conditions impacting behavior.
   */
  market_conditions: MarketConditions;
  /**
   * Legal final maturity date.
   */
  maturity: DateWire;
  /**
   * Metric-time pricing configuration.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Asset pool definition.
   */
  pool: AssetPool;
  /**
   * Prepayment model specification.
   */
  prepayment_spec?: cashflows.PrepaymentModelSpec;
  /**
   * Whether the template waterfall pays senior fees and senior note
   * interest shortfalls from principal proceeds before any note is
   * redeemed (the CLO principal-waterfall convention).
   *
   * `None` (the default) follows the deal type: `true` for CLO/CBO,
   * `false` otherwise. Ignored by a custom [`Self::waterfall`], whose tiers
   * carry their own [`FundingSource`]; see
   * [`Waterfall::fund_senior_interest_from_principal`] and
   * [`Self::effective_principal_covers_senior_interest`].
   */
  principal_covers_senior_interest?: boolean | null;
  /**
   * Buyer settlement date for clean/dirty price and spread metrics.
   * `None` uses the valuation date. Cashflows payable on or before settlement
   * belong to the seller; model PV remains measured on the valuation date.
   */
  quote_settlement_date?: DateWire | null;
  /**
   * Recovery model specification.
   */
  recovery_spec?: cashflows.RecoveryModelSpec;
  /**
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Optional stochastic default model specification.
   */
  stochastic_default_spec?: StochasticDefaultSpec | null;
  /**
   * Optional stochastic prepayment model specification.
   */
  stochastic_prepay_spec?: StochasticPrepaySpec | null;
  /**
   * Optional stochastic recovery model for `price_stochastic`:
   * `MarketCorrelated` recoveries move with the systematic factor (and
   * disperse per name), so a path with heavy defaults also recovers
   * less. `None` keeps recoveries constant at `recovery_spec.rate`.
   */
  stochastic_recovery_spec?: RecoverySpec | null;
  /**
   * Scheduled lender draws on notes after closing, ascending by date
   * ([`TrancheDraw`]); empty for a fully funded structure.
   */
  tranche_draws?: TrancheDraw[];
  /**
   * Re-advance one note each revolving period up to its commitment and the
   * borrowing base ([`TrancheReadvance`]); `None` for no re-advances.
   */
  tranche_readvance?: TrancheReadvance | null;
  /**
   * Tranche structure.
   */
  tranches: TrancheStructure;
  /**
   * Custom payment waterfall used verbatim for pricing.
   *
   * `None` (the default) synthesizes the canonical sequential template from
   * the tranche structure ([`Waterfall::standard_sequential`]) plus any
   * [`Self::fees`]. When set, this waterfall is authoritative:
   * [`Self::create_waterfall`] returns it (with deal-level
   * [`Self::coverage_triggers`] appended) and no template is synthesized.
   * [`Self::waterfall_rules`] overlays (AFC, step-down, shifting interest,
   * controlled accumulation) still apply — they rewrite tiers generically by
   * `payment_type`, so they compose with custom structures.
   *
   * The waterfall also **defines each tranche's interest claim** (not just
   * cash allocation): an uncapped `TrancheInterest` recipient owes the full
   * coupon accrual, a `CappedTrancheInterest` recipient owes the capped
   * coupon (the capped-off portion is never owed and never defers), and a
   * debt tranche with **no** interest recipient owes nothing (a
   * principal-only class). Equity is exempt: its interest/principal split
   * is a reporting convention driven by the tranche's metadata coupon.
   *
   * Constraints, enforced by [`Self::with_waterfall`] and re-checked at
   * pricing time so JSON-supplied deals get identical errors:
   * - every tranche referenced by a tier recipient or coverage trigger must
   *   exist in `tranches`, and tranche-keyed recipients must not target an
   *   equity tranche (the engine records equity flows under
   *   [`RecipientType::Equity`], paid via `ResidualCash`);
   * - at most one interest-type recipient may name a given tranche (the
   *   claim definition must be unambiguous);
   * - `fees` must be `None` — senior fees are expressed as *leading*
   *   [`PaymentType::Fee`] tiers of the custom waterfall, which then feed
   *   the IC numerator and excess-spread/reserve sizing exactly like
   *   template fees (fee tiers ranked below note interest are junior fees
   *   and are deliberately not netted as senior claims);
   * - the waterfall's `currency` must match the pool currency.
   */
  waterfall?: Waterfall | null;
  /**
   * Declarative waterfall rules (available-funds caps, etc.) layered onto the
   * base waterfall each period by the simulation engine. `None` reproduces the base
   * waterfall exactly.
   */
  waterfall_rules?: WaterfallRules | null;
}
/**
 * Assumed optional redemption on a date at a price.
 */
export interface CallAssumption {
  /**
   * For a deal-scope call: also redeem on the first payment date at or
   * after this many months past an early-amortization event, when that
   * comes before `date` (a facility's term-out clock starting at the
   * trigger). `None` keeps the scheduled date only.
   */
  after_early_amortization_months?: number | null;
  /**
   * Earliest redemption date; the call settles on the first payment date
   * at or after it.
   */
  date: DateWire;
  /**
   * Redemption price as a percent of the notes' current balance
   * (`100.0` = par; above par the premium is paid as interest, below par
   * the shortfall is a principal write-down).
   */
  price_pct: number;
  /**
   * Whole deal or a single class.
   */
  scope: CallScope;
}
/**
 * Collateral valuation rules for the OC tests: rating haircuts, the value
 * carried for defaulted collateral, the excess-CCC bucket and discount
 * obligations (CLO indenture conventions). Percentages are percent values
 * (`7.5` = 7.5%); haircuts are decimal fractions.
 */
export interface CoverageRules {
  /**
   * Advance rates and concentration limits evaluated by
   * `CoverageTestType::BorrowingBase` tests; `None` makes such a test a
   * validation error.
   */
  borrowing_base?: BorrowingBaseRules | null;
  /**
   * Excess-CCC bucket: collateral rated CCC+ and below beyond
   * `threshold_pct` of the performing pool is carried at market value or
   * excluded.
   */
  ccc_bucket?: CccBucketRule | null;
  /**
   * Value carried for defaulted collateral whose recovery cash has not yet
   * arrived.
   */
  defaulted_valuation?: DefaultedValuation;
  /**
   * Discount obligations: collateral bought below `price_threshold_pct` of
   * par is carried at its purchase price.
   */
  discount_obligation?: DiscountObligationRule | null;
  /**
   * Haircut applied to the par of performing collateral by rating bucket,
   * as a decimal fraction (`0.5` carries the asset at half par). Ratings
   * are looked up by their letter bucket (`B+`, `B` and `B-` all read the
   * `B` entry, see `CreditRating::bucket`). An `NR` entry applies to
   * unrated asset rows supplied by the caller; reinvestment purchases and
   * materialized instrument collateral are unrated by construction and
   * stay at par. Empty (the indenture par-value convention) by default.
   */
  rating_haircuts?: {
    [k: string]: number;
  };
}
/**
 * Excess-CCC bucket rule.
 */
export interface CccBucketRule {
  /**
   * `true` carries the excess at the assets' `market_price_pct` (assets
   * without a price stay at par); `false` excludes the excess entirely.
   */
  carry_at_market_value: boolean;
  /**
   * Share of the performing pool, in percent, that CCC+ and lower rated
   * collateral may occupy at par (`7.5` = 7.5%).
   */
  threshold_pct: number;
}
/**
 * Discount-obligation rule.
 */
export interface DiscountObligationRule {
  /**
   * Purchase price, in percent of par, below which an asset is a discount
   * obligation carried at its purchase price (`80.0` = 80% of par).
   */
  price_threshold_pct: number;
}
/**
 * One coverage test at a position in the waterfall.
 *
 * The ratio is computed on the period's collateral and note balances
 * (overcollateralization: collateral value over the tested class and every
 * class senior to it; interest coverage: interest collections net of senior
 * fees over the interest due to the same classes); the *position* of the
 * tier that carries the test decides which cash a failure can divert.
 */
export interface CoverageTestSpec {
  /**
   * What a failure does with the diverted interest.
   */
  action?: CoverageTestAction;
  /**
   * Cap on what a failure diverts, as a percent of the interest remaining
   * at the test tier (`50.0` diverts at most half); `None` diverts up to
   * the cure amount.
   */
  divert_pct?: number | null;
  /**
   * Unique test identifier, reported in `WaterfallDistribution::coverage_tests`
   * (`OC_<tranche>` / `IC_<tranche>` by convention).
   */
  id: string;
  /**
   * Overcollateralization or interest coverage.
   */
  kind: CoverageTestType;
  /**
   * Template placement of the test tier; `None` places it after the
   * tested tranche's own interest tier. Used only when the deal
   * synthesizes its waterfall; a custom waterfall places test tiers
   * explicitly.
   */
  placement?: CoveragePlacement | null;
  /**
   * Tested class; the ratio covers this class and every class senior to it.
   */
  tranche_id: string;
  /**
   * Minimum ratio that passes (1.20 means 120%).
   */
  trigger_level: number;
}
/**
 * Deal metadata (counterparties and identifiers).
 */
export interface Metadata {
  /**
   * Manager identifier (for CLO).
   */
  manager_id?: string | null;
  /**
   * Master servicer identifier (for CMBS/RMBS).
   */
  master_servicer_id?: string | null;
  /**
   * Servicer identifier (for ABS/RMBS/CMBS).
   */
  servicer_id?: string | null;
  /**
   * Special servicer identifier (for CMBS).
   */
  special_servicer_id?: string | null;
  /**
   * Trustee identifier (for ABS).
   */
  trustee_id?: string | null;
}
/**
 * An interest rate swap the deal pays and receives through its waterfall.
 */
export interface HedgeSwap {
  /**
   * Notional the flows track each period.
   */
  notional: SwapNotional;
  /**
   * Fee tier the net payments rank in.
   */
  priority: SwapPriority;
  /**
   * The swap; its `side` is the deal's side (`Pay` pays fixed and
   * receives floating), and its notional currency must be the deal's.
   */
  swap: InterestRateSwap;
}
/**
 * Market conditions that affect prepayment behavior.
 */
export interface MarketConditions {
  /**
   * Finite annual decimal refinancing rate for Richard-Roll incentives; may be negative.
   */
  refi_rate: number;
}
/**
 * A scheduled increase of one note's balance: the lender advances `amount`
 * into the deal on the first payment date at or after `date`; the cash
 * joins principal proceeds (recycled while the deal revolves).
 */
export interface TrancheDraw {
  /**
   * Amount advanced, in the deal currency.
   */
  amount: Money;
  /**
   * Earliest draw date; applied on the first payment date at or after it.
   */
  date: DateWire;
  /**
   * Id of the note drawn.
   */
  tranche_id: string;
}
/**
 * Re-advance one note each revolving period up to its commitment and the
 * borrowing base (`coverage_rules.borrowing_base` on the live collateral):
 * the draw is `max(0, min(commitment, borrowing base) − balance)`.
 */
export interface TrancheReadvance {
  /**
   * Commitment the note's balance may be drawn up to, in the deal currency.
   */
  commitment: Money;
  /**
   * Id of the note re-advanced.
   */
  tranche_id: string;
}
/**
 * Collection of tranches forming the capital structure
 */
export interface TrancheStructure {
  /**
   * Ordered tranches (typically sorted by payment priority)
   */
  tranches: Tranche[];
}
/**
 * Structured credit tranche with attachment/detachment points
 */
export interface Tranche {
  /**
   * Lower structural boundary as a percent of the capital structure
   * (`0.0` for the first-loss class). `None` until
   * [`TrancheStructure::new`] derives it from the balance shares in
   * payment-priority order; a declared value must match that share
   * within the structure's thickness tolerance.
   */
  attach_pct?: number | null;
  /**
   * Attributes for scenario selection
   */
  attributes: Attributes;
  /**
   * Interest specification
   */
  coupon: RateSpec;
  /**
   * Current outstanding balance (after amortization and losses)
   */
  current_balance: Money;
  /**
   * Day count convention for interest accrual
   */
  day_count: DayCount;
  /**
   * Accumulated deferred interest (if payment has been deferred)
   */
  deferred_interest: Money;
  /**
   * Upper structural boundary as a percent of the capital structure
   * (`100.0` for the most senior class); derived like
   * [`Self::attach_pct`](field@Self::attach_pct) when `None`.
   */
  detach_pct?: number | null;
  /**
   * Payment characteristics
   */
  frequency: Tenor;
  /**
   * Interest coverage trigger specification
   */
  ic_trigger?: CoverageTrigger | null;
  /**
   * Unique tranche identifier
   */
  id: Id;
  /**
   * Legal final maturity date
   */
  maturity: DateWire;
  /**
   * Whether the coupon is a non-deferrable claim the template pays from
   * principal proceeds when interest proceeds fall short (and the deal's
   * `principal_covers_senior_interest` allows it). `None` follows the
   * seniority convention: senior notes are non-deferrable, every other
   * class defers.
   */
  non_deferrable?: boolean | null;
  /**
   * Coverage test triggers
   */
  oc_trigger?: CoverageTrigger | null;
  /**
   * Size and balances
   */
  original_balance: Money;
  /**
   * Whether interest shortfalls capitalize into tranche balance (PIK accretion).
   *
   * When `true`, unpaid interest is added to the outstanding tranche balance
   * and accrues interest in subsequent periods (payment-in-kind).
   * When `false` (default for debt tranches), shortfalls are tracked but do
   * NOT increase the balance, matching standard CLO/ABS indenture treatment
   * where shortfalls are paid from future interest collections.
   */
  pik_enabled?: boolean;
  /**
   * Credit rating (if rated by agencies)
   */
  rating?: CreditRating | null;
  /**
   * Tranche characteristics
   */
  seniority: TrancheSeniority;
}
/**
 * Coverage-test trigger specification for a tranche or deal.
 */
export interface CoverageTrigger {
  /**
   * Date on which the breach was recorded, if one has occurred.
   */
  breach_date?: DateWire | null;
  /**
   * Consequence applied while the trigger is breached.
   */
  consequence: TriggerConsequence;
  /**
   * Optional higher coverage ratio required to cure a breach.
   */
  cure_level?: number | null;
  /**
   * Breach threshold, expressed as a coverage ratio (1.20 means 120%).
   */
  trigger_level: number;
}
/**
 * Main waterfall engine with tier-based distribution
 */
export interface Waterfall {
  /**
   * Collateral valuation rules for the OC tests; `None` values collateral
   * at par with defaulted assets at recovery.
   */
  coverage_rules?: CoverageRules | null;
  /**
   * Base currency
   */
  currency: Currency;
  /**
   * Ordered payment tiers, including [`PaymentType::CoverageTest`] positions
   */
  tiers: WaterfallTier[];
}
/**
 * Waterfall tier: a payment step with recipients, or a coverage-test
 * position ([`PaymentType::CoverageTest`]) carrying `tests` and no recipients.
 */
export interface WaterfallTier {
  /**
   * How to allocate within tier
   */
  allocation_mode: AllocationMode;
  /**
   * Collection account(s) this tier draws on; `None` uses
   * [`FundingSource::default_for`] the tier's `payment_type`. See
   * [`Self::effective_funding`].
   */
  funding?: FundingSource | null;
  /**
   * Unique tier identifier
   */
  id: string;
  /**
   * Payment type classification
   */
  payment_type: PaymentType;
  /**
   * Priority order (lower = higher priority)
   */
  priority: number;
  /**
   * Recipients in this tier (empty for a coverage-test tier)
   */
  recipients: Recipient[];
  /**
   * Coverage tests evaluated at this position (only for
   * [`PaymentType::CoverageTest`] tiers; empty otherwise). Every test in
   * one tier shares the same [`CoverageTestAction`].
   */
  tests?: CoverageTestSpec[];
}
/**
 * Individual payment recipient within a tier
 */
export interface Recipient {
  /**
   * How to calculate payment amount
   */
  calculation: PaymentCalculation;
  /**
   * Unique identifier
   */
  id: string;
  /**
   * Recipient type
   */
  recipient_type: RecipientType;
  /**
   * Weight for pro-rata distribution (None = equal weight)
   */
  weight?: number | null;
}
/**
 * Declarative, additively-applied waterfall rules layered onto a deal's base
 * waterfall.
 *
 * Each sub-spec is optional; when none are present the resolved waterfall is
 * identical to the base waterfall. The simulation engine applies them to
 * each period's copy of the waterfall.
 */
export interface WaterfallRules {
  /**
   * Available-funds / net-WAC cap on named tranches' interest.
   */
  afc?: AfcSpec | null;
  /**
   * Controlled accumulation: accumulate principal into a funding account and
   * repay the investor as a bullet at the accumulation end.
   */
  controlled_accumulation?: ControlledAccumulationSpec | null;
  /**
   * Early amortization: end a revolving period early on a performance breach
   * (master-trust style), switching the deal into amortization.
   */
  early_amortization?: EarlyAmortizationSpec | null;
  /**
   * Excess-spread (spread-account) capture and draw.
   */
  excess_spread?: ExcessSpreadSpec | null;
  /**
   * Reserve account target, replenishment, excess release and final
   * principal cover.
   */
  reserve?: ReserveAccountSpec | null;
  /**
   * Shifting interest: senior receives a scheduled (declining) share of
   * principal. Mutually exclusive with `step_down` (both govern principal
   * allocation); `shifting_interest` takes precedence when both are set.
   */
  shifting_interest?: ShiftingInterestSpec | null;
  /**
   * Step-down: switch principal allocation to pro-rata after a date when the
   * step-down trigger passes.
   */
  step_down?: StepDownSpec | null;
  /**
   * Targeted-overcollateralization amortization: notes are paid down each
   * period to the amount that holds the pool's overcollateralization at
   * the target, with the excess released to the residual holder.
   */
  target_oc?: TargetOcSpec | null;
}
/**
 * Controlled-accumulation specification for revolving (master-trust) deals.
 *
 * Between `start_date` and `bullet_date` the deal is in its controlled-
 * accumulation period: collected pool principal is held in a principal funding
 * account (investor balances stay flat, no pass-through paydown) rather than
 * recycled or distributed. At the first payment date on or after `bullet_date`
 * the entire account is released into the waterfall as a single bullet
 * principal payment. Accumulation is suspended while a revolving/reinvestment
 * period is still active (which recycles principal) and on early amortization
 * (which pays principal down immediately).
 *
 * `start_date` is typically the revolving-period end and `bullet_date` the
 * note's expected maturity (and at/before the legal final maturity, so the
 * release occurs before the deal winds down). If the deal terminates before
 * the bullet date (cleanup call, pool exhaustion, or a bullet date beyond the
 * last payment), any residual funding-account balance is swept to the
 * outstanding tranches senior-first at deal end, so accumulated principal is
 * never stranded.
 */
export interface ControlledAccumulationSpec {
  /**
   * Date the accumulated funding account is released as a bullet payment.
   */
  bullet_date: DateWire;
  /**
   * First date principal is accumulated into the funding account.
   */
  start: DateWire;
}
/**
 * Early-amortization specification for revolving (master-trust) deals.
 *
 * While a deal's reinvestment/revolving period is active, principal is recycled
 * and the investor (tranche) balances are held flat. If cumulative losses reach
 * `max_cumulative_loss` or the trailing excess spread falls below
 * `min_excess_spread_3m`, an early-amortization event is triggered: the
 * revolving period ends immediately and the deal begins paying principal down
 * (amortizing) even before its scheduled revolving-period end.
 */
export interface EarlyAmortizationSpec {
  /**
   * Cumulative-loss fraction (decimal, of the original pool balance) at or
   * above which the revolving period ends early and amortization begins.
   * `None` disables the loss test.
   */
  max_cumulative_loss?: number | null;
  /**
   * Annualized excess spread (decimal of the opening pool balance: pool
   * interest less debt coupons due, fees paid and net charge-offs) whose
   * three-period trailing average, once it falls below this floor, ends the
   * revolving period for good. `None` disables the excess-spread test.
   */
  min_excess_spread_3m?: number | null;
}
/**
 * Excess-spread / spread-account specification.
 *
 * Each period the account captures residual interest (that would otherwise be
 * distributed to equity) up to `target_balance`, and draws down to cover debt
 * tranche interest shortfalls — providing credit enhancement from excess
 * spread. At termination it pays deferred debt coupons; a breached loss trap
 * then applies remaining interest to debt principal before releasing the
 * surplus to the residual holder.
 */
export interface ExcessSpreadSpec {
  /**
   * Target funded balance of the spread account (currency units).
   */
  target_balance: Money;
  /**
   * Optional cumulative-loss fraction (decimal, e.g. `0.05` = 5% of the
   * original pool) at or above which terminal spread-account cash repays
   * debt principal after deferred coupons. Any surplus reaches the residual
   * holder. `None` releases surplus after deferred coupons without this
   * interest-to-principal transfer.
   */
  trap_loss_decimal?: number | null;
}
/**
 * Deal reserve account rules for the template waterfall.
 *
 * The account starts at `AssetPool::reserve_account`. Every period the
 * target is resolved from the live pool; a `ReserveReplenishment` recipient
 * after the last note coupon and the junior fees tops the account up from
 * interest proceeds (`replenish`), the balance above the target is released
 * into the waterfall's interest proceeds (`release_excess`), and the account
 * covers senior fees and note coupons whenever interest proceeds fall short.
 * At legal final the balance retires note principal by priority
 * (`covers_principal_at_final`) or goes straight to the residual holder.
 */
export interface ReserveAccountSpec {
  /**
   * At legal final, apply the balance to unpaid note principal by
   * priority before the residual holder. Defaults to `true`; `false`
   * releases it to the residual holder.
   */
  covers_principal_at_final?: boolean;
  /**
   * Release any balance above the target into the period's interest
   * proceeds. Defaults to `true`.
   */
  release_excess?: boolean;
  /**
   * Top the account up from interest proceeds after the note coupons and
   * junior fees. Defaults to `true`.
   */
  replenish?: boolean;
  /**
   * Required balance, re-evaluated every period.
   */
  target: ReserveTarget;
}
/**
 * Shifting-interest principal allocation (non-agency senior/sub RMBS).
 *
 * Scheduled principal is always paid pro-rata by current balance; the
 * schedule governs *unscheduled* principal (prepayments and recoveries).
 * Under [`ShiftMode::ShiftOfSubordinate`] (the default) each step is the
 * fraction of the subordinates' pro-rata share that shifts to the senior:
 * `1.0` is the full lockout, later steps (`0.7`, `0.6`, ...) release the
 * subordinates' share progressively, `0.0` is pro-rata. Under
 * [`ShiftMode::SeniorShare`] each step is the senior's share of unscheduled
 * principal directly. While any of `triggers` fails the shift reverts to the
 * full lockout regardless of the schedule.
 */
export interface ShiftingInterestSpec {
  /**
   * How the schedule values are read; `ShiftOfSubordinate` by default.
   */
  mode?: ShiftMode;
  /**
   * Schedule ascending by `months_from_closing`; each step's `senior_decimal`
   * is read per `mode`.
   */
  schedule: ShiftingInterestStep[];
  /**
   * Id of the senior tranche that receives the shifted principal.
   */
  senior_id: string;
  /**
   * Performance tests that must all pass for the schedule to apply; while
   * any fails the senior takes every unscheduled dollar (lockout).
   */
  triggers?: StepDownTrigger[];
}
/**
 * One step of a shifting-interest schedule: the senior's share of principal
 * from `months_from_closing` onward (until the next step).
 */
export interface ShiftingInterestStep {
  /**
   * Months from closing at which this senior share takes effect.
   */
  months_from_closing: number;
  /**
   * Senior share of principal (decimal, `1.0` = 100% lockout) from this step.
   */
  senior_decimal: number;
}
/**
 * Step-down specification for senior/subordinate principal allocation.
 *
 * Principal is paid sequentially (senior first) until the deal seasons past
 * `step_down_date`; from then on, *if* every configured [`StepDownTrigger`]
 * passes, principal switches to pro-rata across the debt tranches, releasing
 * subordination to the juniors. While any trigger is breached the deal reverts
 * to sequential, so the switch is re-evaluated every period (non-sticky). An
 * empty `triggers` list steps down purely on the date.
 */
export interface StepDownSpec {
  /**
   * Earliest date principal may switch to pro-rata.
   */
  step_down_date: DateWire;
  /**
   * Performance triggers; all must pass for the step-down to take effect.
   */
  triggers: StepDownTrigger[];
}
/**
 * Targeted overcollateralization amortization (auto and consumer ABS).
 *
 * Each period the notes are paid down to the amount that leaves the pool's
 * overcollateralization (`pool − notes`) at the target: the larger of
 * `fraction_of_current` of the pool balance after the period's collections and
 * `floor_fraction_of_original` of the cut-off balance. The required principal
 * distribution is `max(0, notes − max(pool − target, 0))`, paid to the
 * notes by priority from interest proceeds first and principal proceeds
 * for the rest; the collections above it are released to the residual
 * holder.
 */
export interface TargetOcSpec {
  /**
   * Floor on the target as a decimal fraction of the original (cut-off)
   * pool balance; `0.0` for no floor.
   */
  floor_fraction_of_original?: number;
  /**
   * Target overcollateralization as a decimal fraction of the current
   * pool balance (after the period's collections).
   */
  fraction_of_current: number;
}
/**
 * Levered real estate equity = unlevered asset + financing.
 *
 * Value convention:
 * - `PV_equity = PV_asset - PV_financing` (financing valued from lender perspective).
 *
 * Return/coverage metrics are computed from a simplified equity cashflow schedule:
 * - Initial outflow: `-(purchase_price + Σ acquisition_costs)` at `as_of`
 * - Financing funding legs on/after `as_of` are included as equity inflows
 * - Interim equity CFs: `(NOI - CapEx) - debt_service_cash`
 * - Exit: `(sale_proceeds - financing_payoff)` at the asset horizon (`asset.sale_date`, else the last NOI date)
 */
export interface LeveredRealEstateEquity {
  /**
   * Underlying unlevered asset.
   */
  asset: RealEstateAsset;
  /**
   * Attributes for tagging and scenarios.
   * Attributes for scenario selection and tagging
   */
  attributes: Attributes;
  /**
   * Currency (must match asset currency; financing PV is validated at valuation time).
   */
  currency: Currency;
  /**
   * Supported borrower liabilities, valued from the lender perspective and
   * netted from asset PV.
   */
  financing?: RealEstateFinancing[];
  /**
   * Unique instrument identifier.
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
   * Scenario-only pricing adjustments.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
}
/**
 * Priceable composite instrument containing an unresolved policy and immutable state.
 *
 * Valuation and risk use `state` exactly as stored. Call [`Self::rebalance`]
 * to obtain a distinct instrument; this type does not mutate in place.
 */
export interface CompositeInstrument {
  /**
   * Instrument-owned pricing inputs.
   *
   * A composite has no pricing model of its own, so this must stay empty;
   * set quotes and model inputs on each leg instead.
   */
  instrument_pricing_overrides?: InstrumentPricingOverrides;
  /**
   * Metric-time pricing configuration.
   *
   * Also applied to every leg as defaults: a leg's own setting wins, and
   * `bump_config.adaptive_bumps` is enabled on a leg when either sets it.
   */
  metric_pricing_overrides?: MetricPricingOverrides;
  /**
   * Scenario-only pricing adjustments.
   *
   * Applied to the composite value after leg aggregation.
   */
  scenario_pricing_overrides?: ScenarioPricingOverrides;
  /**
   * Economic definition and future rebalance policy.
   */
  spec: CompositeSpec;
  /**
   * Frozen quantities used for every valuation until explicit rebalance.
   */
  state: CompositeState;
}
/**
 * Unresolved composite definition and rebalance policy.
 */
export interface CompositeSpec {
  /**
   * Scenario-selection and reporting metadata.
   */
  attributes: Attributes;
  /**
   * Positive return denominator per composite unit.
   */
  capital: Money;
  /**
   * Stable composite instrument identifier.
   */
  id: Id;
  /**
   * Self-contained underlying instrument definitions.
   */
  legs: CompositeLegSpec[];
  /**
   * Dates on which explicit dynamic resolution becomes eligible.
   */
  rebalance_rule: RebalanceRule;
  /**
   * Currency in which value, P&L, additive risk, and capital are reported.
   */
  reporting_currency: Currency;
  /**
   * Rule used to resolve quantities at initialization and rebalance.
   */
  weighting_method: WeightingMethod;
}
/**
 * One self-contained leg in a composite specification.
 */
export interface CompositeLegSpec {
  /**
   * Canonical typed definition of the underlying instrument.
   */
  instrument: InstrumentJson;
  /**
   * Stable identifier of the embedded instrument; must equal `instrument.id()`.
   */
  instrument_id: Id;
  /**
   * Signed leg score: the resolved unit quantity under
   * [`WeightingMethod::FixedQuantity`] and a scale-free signed target score
   * that every dynamic method normalizes.
   */
  score: number;
}
/**
 * Expression AST with optional unique ID for DAG planning.
 *
 * Deserialization is strict (`deny_unknown_fields`): unknown fields on
 * inbound payloads are rejected rather than silently ignored.
 */
export interface Expr {
  /**
   * Unique identifier for this expression node (for DAG planning).
   */
  id?: number | null;
  /**
   * The actual expression node.
   */
  node: ExprNode;
}
/**
 * Immutable holdings state used by pricing, risk, scenarios, and history.
 */
export interface CompositeState {
  /**
   * Date from which the resolved quantities are effective.
   */
  effective_date: DateWire;
  /**
   * Resolved top-level quantities in specification order.
   */
  resolved_legs: ResolvedCompositeLeg[];
  /**
   * Finite scalar inputs retained for rebalance audit and reproducibility.
   */
  weighting_inputs: {
    [k: string]: number;
  };
}
/**
 * Immutable resolved quantity for one top-level composite leg.
 */
export interface ResolvedCompositeLeg {
  /**
   * Identifier of the corresponding leg specification.
   */
  instrument_id: Id;
  /**
   * Signed quantity held from the state's effective date until the next rebalance.
   */
  quantity: number;
}
/**
 * Configuration for basket pricing behaviour.
 */
export interface BasketPricingConfig {
  /**
   * Day basis used for fee accrual (e.g., 365.0 or 365.25). Avoid hardcoding in logic.
   */
  days_in_year: number;
  /**
   * FX policy hint for conversions when constituent currency != basket currency.
   */
  fx_policy: FxConversionPolicy;
}
/**
 * Asset-backed facility borrowing base, availability and advance-rate test.
 */
export interface BorrowingBaseReport {
  /**
   * Advance-rate-weighted eligible collateral after the limits.
   */
  borrowing_base: Money;
  /**
   * Eligible balance excluded by the concentration limits.
   */
  concentration_excess: Money;
  /**
   * Collateral balance that meets the eligibility criteria, before
   * concentration limits.
   */
  eligible_collateral: Money;
}
/**
 * Single-row enriched cashflow view.
 */
export interface CashflowRow {
  /**
   * Accrual factor stored on the `CashFlow`.
   */
  accrual_factor: number;
  /**
   * Signed cashflow amount in row currency.
   */
  amount: number;
  /**
   * Beginning pool balance for the period (agency MBS only).
   */
  beginning_balance?: number | null;
  /**
   * Interval default probability `SP(t_{i-1}) − SP(t_i)` (hazard mode only).
   */
  conditional_default_prob?: number | null;
  /**
   * Row currency (matters for `XccySwap` / `FxSwap`).
   */
  currency: Currency;
  /**
   * Payment date.
   */
  date: DateWire;
  /**
   * Discount curve used for this row.
   */
  discount_curve_id: Id;
  /**
   * `df(as_of, date)`.
   */
  discount_factor: number;
  /**
   * Ending pool balance for the period (agency MBS only).
   */
  ending_balance?: number | null;
  /**
   * Inflation index ratio (populated for `InflationLinkedBond`).
   */
  inflation_index_ratio?: number | null;
  /**
   * `CFKind` discriminator (serde rename: `fixed`, `notional`, …).
   */
  kind: CFKind;
  /**
   * Single Monthly Mortality for the period (populated for agency MBS).
   */
  prepayment_smm?: number | null;
  /**
   * Per-flow present value in the envelope reporting currency. Sums to `total_pv`.
   */
  pv: number;
  /**
   * Projected / contractual rate when present (floats, real-coupon rates, etc.).
   */
  rate?: number | null;
  /**
   * Reset date when the flow is a floating-rate fixing.
   */
  reset_date?: DateWire | null;
  /**
   * Cumulative survival probability (hazard mode only).
   */
  survival_probability?: number | null;
  /**
   * Year fraction from `as_of` to `date` under the discount curve's day count.
   */
  year_fraction: number;
  [k: string]: unknown;
}
/**
 * Market conventions of one CDS currency and documentation clause.
 */
export interface CdsConventionSpec {
  /**
   * The business day convention.
   */
  business_day_convention: BusinessDayConvention;
  /**
   * The calendar used for business day adjustments.
   */
  calendar_id: string;
  /**
   * The day count convention for the premium leg.
   */
  day_count: DayCount;
  /**
   * Regional schedule family represented by this registry row.
   */
  family: CdsConvention;
  /**
   * The payment frequency of the premium leg.
   */
  frequency: Tenor;
  /**
   * The number of business days for settlement.
   */
  settlement_days: number;
  /**
   * Stub convention used when constructing the premium schedule.
   */
  stub: StubKind;
  [k: string]: unknown;
}
/**
 * Preset identity, coupon and convention of a standard CDS index series.
 */
export interface CdsIndexParams {
  /**
   * Regional ISDA convention. Bundled into the preset because each
   * well-known index has a fixed convention (CDX uses `IsdaNa`, iTraxx
   * uses `IsdaEu`).
   */
  convention: CdsConvention;
  /**
   * Running fixed coupon in basis points (e.g. 100bp for CDX.NA.IG).
   */
  coupon_bp: number;
  /**
   * Index name (e.g., "CDX.NA.IG", "iTraxx Europe").
   */
  index_name: string;
  /**
   * Number of reference entities in this series, when known.
   *
   * Membership counts vary by series (e.g. iTraxx Crossover has been 75
   * names only since Series 9; CDX.NA.HY membership varies), so this is
   * part of the per-series preset rather than inferred from the name.
   * `None` for custom presets where the count is unknown — callers must
   * then attach an explicit count via `CdsIndex::with_num_constituents`.
   */
  num_constituents?: number | null;
  /**
   * Index series number (e.g., 42).
   */
  series: number;
  /**
   * Index version number within the series.
   */
  version: number;
}
/**
 * Attachment, detachment, notional and coupon of a CDS index tranche.
 */
export interface CdsTrancheParams {
  /**
   * Attachment point as percentage
   */
  attach_pct: number;
  /**
   * Running coupon in basis points
   */
  coupon_bp: number;
  /**
   * Detachment point as percentage
   */
  detach_pct: number;
  /**
   * Index name (e.g., "CDX.NA.IG", "iTraxx Europe")
   */
  index_name: string;
  /**
   * Maturity date
   */
  maturity: DateWire;
  /**
   * Notional amount
   */
  notional: Money;
  /**
   * Realized (settled) loss as a decimal fraction of the original portfolio notional, in `[0.0, 1.0]`
   */
  realized_loss: number;
  /**
   * Index series
   */
  series: number;
}
/**
 * Complete primitive decomposition with path, net, and gross views.
 */
export interface CompositeExposureReport {
  /**
   * Primitive aggregates ordered by identifier.
   */
  aggregates: PrimitiveAggregate[];
  /**
   * Every primitive path before overlap netting.
   */
  paths: PrimitiveExposure[];
  /**
   * Composite reporting currency.
   */
  reporting_currency: Currency;
}
/**
 * Net and gross exposure aggregated by primitive instrument identifier.
 */
export interface PrimitiveAggregate {
  /**
   * Sum of absolute path risk by metric.
   */
  gross_measures: {
    [k: string]: number;
  };
  /**
   * Sum of absolute path quantities.
   */
  gross_quantity: number;
  /**
   * Sum of absolute reporting-currency path values.
   */
  gross_value: Money;
  /**
   * Primitive instrument identifier.
   */
  instrument_id: Id;
  /**
   * Canonical primitive instrument type discriminator.
   */
  instrument_type: string;
  /**
   * Algebraic additive risk by metric.
   */
  net_measures: {
    [k: string]: number;
  };
  /**
   * Algebraic sum of path quantities.
   */
  net_quantity: number;
  /**
   * Algebraic reporting-currency value.
   */
  net_value: Money;
}
/**
 * Path-level primitive exposure in a resolved composite tree.
 */
export interface PrimitiveExposure {
  /**
   * Primitive instrument identifier.
   */
  instrument_id: Id;
  /**
   * Canonical primitive instrument type discriminator.
   */
  instrument_type: string;
  /**
   * Reporting-currency additive risk measures for this path.
   */
  measures: {
    [k: string]: number;
  };
  /**
   * Composite and leg identifiers from the root to the primitive.
   */
  path: string[];
  /**
   * Signed primitive quantity after multiplying every nested state quantity.
   */
  quantity: number;
  /**
   * Reporting-currency signed value for this path.
   */
  value: Money;
}
/**
 * One dated composite holdings state and its accumulated trades.
 */
export interface CompositeHistoryRow {
  /**
   * Signed primitive cashflows on `(previous_date, date]`; zero on the first row.
   */
  cashflows: Money;
  /**
   * Market observation date of this close.
   */
  date: DateWire;
  /**
   * Primitive path, net, and gross exposures under the held (pre-rebalance) state.
   */
  exposures: CompositeExposureReport;
  /**
   * Effective date of quantities held into this close.
   */
  held_state_effective_date: DateWire;
  /**
   * New state date made effective for the next interval, when rebalanced.
   */
  next_state_effective_date?: DateWire | null;
  /**
   * `pnl / capital` for one composite unit; zero on the first row.
   */
  period_return: number;
  /**
   * `Δvalue + cashflows` versus the prior interval's financed opening value;
   * zero on the first row.
   */
  pnl: Money;
  /**
   * Primitive quantity deltas emitted by a close-of-period rebalance.
   */
  rebalance_trades: CompositeTrade[];
  /**
   * Chained total-return index, initialized to `100` on the first row.
   */
  return_index: number;
  /**
   * Composite value before any close-of-period rebalance, in reporting currency.
   */
  value: Money;
}
/**
 * Primitive execution delta produced by initialization or rebalance.
 */
export interface CompositeTrade {
  /**
   * Primitive instrument identifier.
   */
  instrument_id: Id;
  /**
   * Canonical primitive instrument type discriminator.
   */
  instrument_type: string;
  /**
   * Signed change in primitive quantity.
   */
  quantity_delta: number;
}
/**
 * One top-level leg result retained in composite valuation details.
 */
export interface CompositeLegValuation {
  /**
   * Identifier of the top-level leg specification.
   */
  instrument_id: Id;
  /**
   * Quantity-scaled value in the underlying instrument's native currency.
   */
  native_value: Money;
  /**
   * Signed frozen leg quantity applied to the unit valuation.
   */
  quantity: number;
  /**
   * Quantity-scaled value converted to the composite reporting currency.
   */
  reporting_value: Money;
  /**
   * Complete unit-instrument valuation, including nested details where applicable.
   */
  valuation: ValuationResult;
}
/**
 * Canonical valuation result containing PV and typed metrics.
 */
export interface ValuationResult {
  /**
   * Valuation date (T+0) for the calculation.
   */
  as_of: DateWire;
  /**
   * Covenant compliance results for structured products.
   *
   * Present only for instruments with covenants (loans, structured credit).
   * Each covenant is keyed by its identifier with pass/fail status and details.
   */
  covenants?: {
    [k: string]: CovenantReport;
  } | null;
  /**
   * Optional rich model-specific pricing detail.
   */
  details?: ValuationDetails | null;
  /**
   * Optional computation explanation trace.
   *
   * Enabled via `ExplainOpts` in configuration. Provides step-by-step
   * trace of calculations for debugging and auditability.
   */
  explanation?: ExplanationTrace | null;
  /**
   * Unique identifier for the priced instrument.
   */
  instrument_id: string;
  /**
   * Computed risk measures and financial metrics.
   *
   * Contains **derived risk metrics** such as DV01, Delta, Vega, etc.
   * The present value (PV) is **not** included here - it is available
   * in the `value` field above.
   *
   * Keys are strongly-typed metric IDs (serialized as strings such as
   * "ytm", "dv01", "delta"). Use `MetricId` helpers for consistent lookups.
   *
   * # Interpretation
   *
   * Entries in this map are heterogeneous by design:
   * - some are currency amounts (`jump_to_default`)
   * - some are currency-per-bump sensitivities (`dv01`, `vega`, `rho`)
   * - some are decimal rates or probabilities (`ytm`, `default_probability`)
   * - some are ratios or counts (`tvpi_lp`, `constituent_count`)
   *
   * Always interpret a measure together with its [`MetricId`] contract.
   */
  measures: {
    [k: string]: number;
  };
  /**
   * Calculation metadata and policy stamps.
   *
   * Contains:
   * - Numeric mode (Decimal vs f64)
   * - Rounding context and precision
   * - FX policy for cross-currency calculations
   * - Calculation timing information
   */
  meta: ResultsMeta;
  /**
   * Required wire-format schema version. Only numeric `1` is accepted.
   */
  schema_version: SchemaVersion;
  /**
   * Present value in the instrument's native currency.
   *
   * This is the primary pricing output and is **always available** regardless
   * of which metrics are requested. The PV is **not** included in the `measures`
   * map - it is provided here as a `Money` type with full currency information.
   *
   * For cross-currency instruments, this may be in a different currency than
   * the base calculation currency.
   */
  value: Money;
}
/**
 * Covenant check result.
 */
export interface CovenantReport {
  /**
   * Actual value of the metric
   */
  actual_value?: number | null;
  /**
   * Stable machine-readable covenant instance identifier.
   */
  covenant_id?: string | null;
  /**
   * Type of covenant being checked
   */
  covenant_type: string;
  /**
   * Details or explanation
   */
  details?: string | null;
  /**
   * Cushion relative to threshold (positive => passing buffer)
   */
  headroom?: number | null;
  /**
   * Audit stamp: numeric mode, rounding context, and FX policy in force.
   */
  meta?: ResultsMeta;
  /**
   * Whether the covenant passed
   */
  passed: boolean;
  /**
   * Required threshold
   */
  threshold?: number | null;
}
/**
 * Metadata bundle that accompanies valuation outputs.
 *
 * The metadata is intentionally small so it can be attached to reports and
 * downstream data stores for reproducibility and audit trails.
 */
export interface ResultsMeta {
  /**
   * Optional FX policy applied by the computing layer (human-readable key).
   */
  fx_policy_applied?: string | null;
  /**
   * Numeric engine mode used to produce the results.
   *
   * Always [`NUMERIC_MODE_F64`] today; a plain string so result
   * envelopes stay wire-compatible across host languages.
   */
  numeric_mode: string;
  /**
   * Whether the producing computation ran in parallel.
   *
   * Serial results omit the field entirely so existing payloads and golden
   * files stay byte-identical.
   */
  parallel?: boolean;
  /**
   * Rounding context snapshot applied to IO boundaries.
   */
  rounding: RoundingContext;
  /**
   * Timestamp when result was computed (ISO 8601 format).
   * Useful for audit trails and reproducibility.
   */
  timestamp?: string | null;
  /**
   * Finstack Quant library version used to produce the result.
   */
  version?: string | null;
  [k: string]: unknown;
}
/**
 * Snapshot of active rounding settings used for result stamping.
 *
 * Instances are typically produced via [`rounding_context_from`] and persisted
 * alongside valuation results.
 */
export interface RoundingContext {
  /**
   * Ingest scale map snapshot by currency code.
   */
  ingest_scale_by_currency: {
    [k: string]: number;
  };
  /**
   * Active rounding mode.
   */
  mode: RoundingMode;
  /**
   * Output scale map snapshot by currency code.
   */
  output_scale_by_currency: {
    [k: string]: number;
  };
  /**
   * Tolerance settings snapshot for floating-point comparisons.
   */
  tolerances?: ToleranceConfig;
  /**
   * Schema version for forward compatibility.
   */
  version: number;
  [k: string]: unknown;
}
/**
 * Numerical tolerance configuration for floating-point comparisons.
 *
 * Provides configurable epsilon values for zero-checks in rate calculations
 * and generic floating-point comparisons. These defaults are chosen to balance
 * numerical stability with practical precision requirements.
 */
export interface ToleranceConfig {
  /**
   * Epsilon for generic floating-point comparisons (default: 1e-10).
   *
   * Used for general numerical comparisons where higher tolerance is acceptable.
   */
  generic_epsilon?: PositiveF64Wire;
  /**
   * Epsilon for rate comparisons (default: 1e-12).
   *
   * Used when comparing interest rates, yields, and other small ratios.
   */
  rate_epsilon?: PositiveF64Wire;
}
/**
 * Rich structured details attached to composite valuation results.
 */
export interface CompositeValuationDetails {
  /**
   * Recursive path-level and net/gross primitive exposures.
   */
  exposures: CompositeExposureReport;
  /**
   * Native and reporting-currency results for every top-level leg.
   */
  leg_results: CompositeLegValuation[];
  /**
   * Currency of all reported values and additive risk measures.
   */
  reporting_currency: Currency;
  /**
   * Frozen top-level quantities used for this valuation.
   */
  resolved_legs: ResolvedCompositeLeg[];
  /**
   * Effective date of the frozen holdings state used for pricing.
   */
  state_effective_date: DateWire;
  /**
   * Scalar inputs retained when the state was resolved.
   */
  weighting_inputs: {
    [k: string]: number;
  };
}
/**
 * Metadata for CDS-family valuation paths.
 */
export interface CreditDerivativeValuationDetails {
  /**
   * CDS integration method actually used, when applicable.
   */
  integration_method?: string | null;
  /**
   * Registered model key used by the pricer.
   */
  model_key: ModelKey;
  [k: string]: unknown;
}
/**
 * Stochastic pricing result for a structured credit deal.
 */
export interface StochasticPricingResult {
  /**
   * Clean price (percent of the sum of current tranche balances),
   * UNADJUSTED for accrued interest.
   *
   * This equals [`Self::dirty_price`]. The deal-level stochastic
   * result carries no per-tranche interest flows, so accrued cannot be
   * computed here (the same constraint documented on the accrued
   * calculator). It is reported unadjusted rather than fabricated: the
   * error is at most one period's accrued interest, in a known direction.
   * Use `calculate_tranche_metrics` when an accrued-adjusted clean price is
   * required.
   */
  clean_price: number;
  /**
   * Dirty price (percent of the sum of current tranche balances),
   * including accrued interest.
   *
   * This is the authoritative price: it is the present value of all future
   * cashflows from the valuation date, which by definition contains the
   * accrued portion of the next coupon. The face is the notes' current
   * balance, the same basis as the deterministic `dirty_price` metric,
   * so a deal whose notes total less than the pool prices near par rather
   * than at the note-to-pool ratio.
   */
  dirty_price: number;
  /**
   * Mean over paths of the draw option cost: the value to the deal of its
   * revolvers' simulated draws having been made at the contractual margin
   * instead of each path's fair spread, obtained per path as the present
   * value of the actual tranche cashflows less that of a counterfactual
   * run where every draw accrues at its fair spread. Negative when spreads
   * widen; zero for pools without stochastic revolvers.
   */
  draw_option_cost: Money;
  /**
   * Per-path draw option cost in path order (same currency as `npv`),
   * populated only for pools with stochastic revolvers.
   */
  draw_option_cost_paths?: number[];
  /**
   * ES confidence level used
   */
  es_confidence: number;
  /**
   * Mean over paths of the collateral draws funded through the reserve
   * account and principal collections (revolver utilization increases,
   * delayed draws and loan-equivalent draws at default).
   */
  expected_collateral_draws: Money;
  /**
   * Expected loss (probability-weighted average loss)
   */
  expected_loss: Money;
  /**
   * Expected shortfall (tail risk metric)
   */
  expected_shortfall: Money;
  /**
   * Net present value of the deal
   */
  npv: Money;
  /**
   * Number of simulated scenario paths (`2 × pricing_mode.num_paths` for
   * antithetic Monte Carlo, prefixes × suffixes for Hybrid).
   */
  num_paths: number;
  /**
   * Pricing mode used.
   */
  pricing_mode: StructuredCreditPricingMode;
  /**
   * 95% confidence interval for the mean PV
   *
   * @minItems 2
   * @maxItems 2
   */
  pv_confidence_interval: [unknown, unknown];
  /**
   * Standard error of the mean PV estimate
   */
  pv_std_error: number;
  /**
   * Tranche-level results
   */
  tranche_results: TranchePricingResult[];
  /**
   * Unexpected loss (loss standard deviation)
   */
  unexpected_loss: Money;
  /**
   * Fraction of paths on which a collateral draw could not be funded from
   * the reserve account and that period's principal collections (`0.0`
   * for pools without draws).
   */
  unfunded_draw_path_fraction?: number;
  [k: string]: unknown;
}
/**
 * Tranche-level pricing result.
 */
export interface TranchePricingResult {
  /**
   * Attachment point in percent of the capital structure (0 = first loss).
   */
  attach_pct: number;
  /**
   * Credit duration (price sensitivity to credit spread)
   */
  credit_duration: number;
  /**
   * Detachment point in percent of the capital structure (100 = most senior).
   */
  detach_pct: number;
  /**
   * This tranche's share of the deal's draw option cost: the mean over
   * paths of its present value on the actual run less that on the
   * counterfactual run where revolver draws accrue at their fair spread.
   * Tranche shares sum to the deal's `draw_option_cost` on every path.
   */
  draw_option_cost: Money;
  /**
   * Expected loss
   */
  expected_loss: Money;
  /**
   * Expected shortfall
   */
  expected_shortfall: Money;
  /**
   * Net present value
   */
  npv: Money;
  /**
   * Number of simulated paths on which the tranche received any
   * principal (the paths `wal` averages over).
   */
  paths_with_principal?: number;
  /**
   * Mean present value as a percent of the tranche's current balance at
   * the valuation date (100 = par); `0.0` for a fully retired tranche.
   */
  price_pct: number;
  /**
   * Tranche seniority level.
   */
  seniority: TrancheSeniority;
  /**
   * Tranche identifier
   */
  tranche_id: string;
  /**
   * Unexpected loss
   */
  unexpected_loss: Money;
  /**
   * Weighted-average life (WAL, years) averaged over the paths on which
   * the tranche received principal; `0.0` when it never did.
   */
  wal: number;
  [k: string]: unknown;
}
/**
 * Metadata for FX-bearing valuation paths (FX forwards, FX options, quanto).
 *
 * Captures the FX policy applied at pricing time so downstream audit /
 * reconciliation can trace whether a quoted rate came from a direct
 * market quote or via triangulation through the matrix pivot currency.
 * Mirrors the cross-cutting invariant documented in
 * `.claude/rules/project-description.md` ("FX policy visibility: Applied
 * conversion strategy recorded per layer (e.g., valuations, statements,
 * portfolio)").
 */
export interface FxValuationDetails {
  /**
   * `true` when the FX spot was obtained via triangulation through the
   * [`FxMatrix`](finstack_quant_core::money::fx::FxMatrix) pivot currency
   * rather than a direct quote. Mirrors
   * [`FxRateResult.triangulated`](finstack_quant_core::money::fx::FxRateResult).
   * `None` when the instrument resolved spot from an explicit market
   * scalar (`fx_spot_id`) rather than the matrix.
   */
  fx_triangulated?: boolean | null;
  [k: string]: unknown;
}
/**
 * Reproducibility and convergence diagnostics for a Monte Carlo valuation.
 */
export interface MonteCarloValuationDetails {
  /**
   * Whether antithetic variates were enabled.
   */
  antithetic: boolean;
  /**
   * Whether Brownian-bridge ordering was enabled for Sobol paths.
   */
  brownian_bridge: boolean;
  /**
   * Number of independent path estimators contributing to the mean.
   */
  estimator_paths: number;
  /**
   * Independent paths used to fit state-conditional make-whole reference
   * values. Zero when no stochastic make-whole stage is required.
   */
  make_whole_training_paths: number;
  /**
   * Total factor paths simulated for state-conditional make-whole training,
   * including antithetic partners. Zero when that stage is absent.
   */
  make_whole_training_simulated_paths: number;
  /**
   * Registered model used for the simulation.
   */
  model_key: ModelKey;
  /**
   * Deterministic random seed used for the run.
   */
  seed: number;
  /**
   * Total number of simulated paths, including antithetic partners.
   */
  simulated_paths: number;
  /**
   * Whether Sobol quasi-random sampling was enabled.
   */
  sobol: boolean;
  /**
   * Sampling standard error of the discounted PV mean in the result
   * currency. For LSMC valuations, this measures pricing-path uncertainty
   * under the frozen fitted exercise policy. It excludes regression
   * approximation, time-grid discretization, and model error.
   */
  standard_error: number;
  /**
   * Simulation times in year fractions, including zero and maturity.
   */
  time_grid: number[];
  /**
   * Number of independent paths used to fit an exercise or control policy.
   *
   * Zero for Monte Carlo engines that do not have a separate training
   * stage.
   */
  training_paths: number;
  /**
   * Total factor paths simulated in the policy-training stage, including
   * antithetic partners. Zero when no policy is trained.
   */
  training_simulated_paths: number;
  [k: string]: unknown;
}
/**
 * Container for detailed execution traces of financial computations.
 *
 * Traces are organized by type (calibration, pricing, waterfall) and contain
 * a sequence of domain-specific entries. Traces can be serialized to JSON for
 * inspection, debugging, or audit purposes.
 *
 * Mutation is intentionally single-threaded through `&mut self`. If multiple
 * workers need to append to one trace, wrap it in external synchronization and
 * keep ordering semantics explicit at the call site.
 */
export interface ExplanationTrace {
  /**
   * Sequence of trace entries
   */
  entries: TraceEntry[];
  /**
   * Whether the trace was truncated due to size limits
   */
  truncated?: boolean | null;
  /**
   * Type of trace (e.g., "calibration", "pricing", "waterfall")
   */
  type: string;
  [k: string]: unknown;
}
/**
 * Result of resolving a new composite holdings state.
 */
export interface CompositeRebalanceResult {
  /**
   * Newly resolved, immutable, priceable composite, serialized as its
   * canonical instrument envelope.
   */
  instrument: InstrumentEnvelope;
  /**
   * Net primitive execution deltas required to reach the new state.
   */
  trades: CompositeTrade[];
}
/**
 * Canonical v1 envelope for every supported financial instrument.
 */
export interface InstrumentEnvelope {
  /**
   * The instrument definition
   */
  instrument: InstrumentJson;
  /**
   * Required v1 instrument contract marker.
   */
  schema: InstrumentSchema;
}
/**
 * Tree price and Greeks of a convertible bond.
 */
export interface ConvertibleGreeks {
  /**
   * Delta (spot sensitivity per unit spot move)
   */
  delta: number;
  /**
   * Gamma (curvature, second derivative w.r.t. spot)
   */
  gamma: number;
  /**
   * Instrument price
   */
  price: number;
  /**
   * Rho (interest rate sensitivity per 1bp rate move)
   */
  rho: number;
  /**
   * Theta (time decay per day)
   */
  theta: number;
  /**
   * Vega (volatility sensitivity per 1% vol move)
   */
  vega: number;
  [k: string]: unknown;
}
/**
 * One coverage test as the waterfall executor evaluated it in a period.
 */
export interface CoverageTestDiagnostic {
  /**
   * `ratio − trigger_level`; negative while the test fails.
   */
  cushion: number;
  /**
   * Whether the test passed in this period.
   */
  passing: boolean;
  /**
   * Ratio the executor computed (OC: collateral ÷ notes; IC: interest ÷ due).
   */
  ratio: number;
  /**
   * `CoverageTestSpec::id` of the test.
   */
  test_id: string;
  /**
   * Trigger level the ratio was tested against.
   */
  trigger_level: number;
  [k: string]: unknown;
}
/**
 * One structured finding emitted while loading a persisted contract.
 */
export interface Diagnostic {
  /**
   * Version found in the payload, when one was present.
   */
  actual_version?: number | null;
  /**
   * Canonical artifact digest, including its algorithm prefix, when available.
   */
  artifact_hash?: string | null;
  /**
   * Stable machine-readable code, such as `"contract/version-unsupported"`.
   */
  code: string;
  /**
   * Stable persisted contract identifier, when the finding is contract-specific.
   */
  contract?: string | null;
  /**
   * Version expected by the loader, when version context is available.
   */
  expected_version?: number | null;
  /**
   * Instrument identifier associated with the finding, when available.
   */
  instrument_id?: string | null;
  /**
   * Human-readable explanation with source details such as parse line and column.
   */
  message: string;
  /**
   * Loading stage that produced the finding.
   */
  phase: LoadPhase;
  /**
   * JSON Pointer locating the finding in the source document, when known.
   */
  pointer?: string | null;
  /**
   * Portfolio-position identifier associated with the finding, when available.
   */
  position_id?: string | null;
  /**
   * Storage revision identifier associated with the artifact, when available.
   */
  revision_id?: string | null;
  /**
   * Whether the finding is fatal or recoverable.
   */
  severity: Severity;
}
/**
 * Revolving-credit Monte Carlo value with per-path details.
 */
export interface EnhancedMonteCarloResult {
  /**
   * Monte Carlo estimate of [`PathResult::draw_option_cost`] across paths,
   * with the same antithetic-aware standard error as the present value.
   */
  draw_option_cost: MoneyEstimate;
  /**
   * Standard MC statistics (mean, std error, CI)
   */
  mc_result: MonteCarloResult;
  /**
   * Individual path results for distribution analysis
   */
  path_results: PathResult[];
  [k: string]: unknown;
}
/**
 * Discounted Monte Carlo estimate tagged with a currency.
 *
 * The engine computes these values from discounted path outcomes. `mean` and
 * `ci_95` are stored as [`Money`], while the auxiliary statistics remain raw
 * `f64` values in the same currency unit as `mean.amount()`.
 */
export interface MoneyEstimate {
  /**
   * 95% confidence interval for the discounted mean present value.
   *
   * @minItems 2
   * @maxItems 2
   */
  ci_95: [unknown, unknown];
  /**
   * Optional maximum of captured discounted path values.
   */
  max?: number | null;
  /**
   * Discounted mean present value.
   */
  mean: Money;
  /**
   * Optional median of captured discounted path values.
   *
   * This is populated only when captured-path diagnostics are available.
   */
  median?: number | null;
  /**
   * Optional minimum of captured discounted path values.
   */
  min?: number | null;
  /**
   * Number of independent path estimators contributing to the estimate.
   *
   * See [`crate::monte_carlo::estimate::Estimate::num_paths`] for the full semantics,
   * including how antithetic variates split simulated work across
   * estimators.
   */
  num_paths: number;
  /**
   * Total number of simulated sample paths driving the estimator.
   *
   * See [`crate::monte_carlo::estimate::Estimate::num_simulated_paths`]. Equal to
   * `num_paths` without variance reduction, or `2 * num_paths` with
   * antithetic variates.
   */
  num_simulated_paths: number;
  /**
   * Optional 25th percentile of captured discounted path values.
   */
  percentile_25?: number | null;
  /**
   * Optional 75th percentile of captured discounted path values.
   */
  percentile_75?: number | null;
  /**
   * Optional sample standard deviation of discounted path values.
   */
  std_dev?: number | null;
  /**
   * Standard error of the discounted mean, in `mean.amount()` units.
   */
  stderr: number;
}
/**
 * Monte Carlo pricing result with optional captured paths.
 *
 * The estimate always reflects all simulated discounted path values used by the
 * pricing run. When `paths` is present, it contains the captured subset only.
 */
export interface MonteCarloResult {
  /**
   * Discounted pricing estimate for the full simulation.
   */
  estimate: MoneyEstimate;
  /**
   * Optional captured-path subset for diagnostics and visualization.
   */
  paths?: PathDataset | null;
  /**
   * Reproducibility metadata for the run (execution policy and seed).
   */
  run?: RunMetadata | null;
  [k: string]: unknown;
}
/**
 * Captured-path collection plus the metadata needed to interpret it.
 *
 * The dataset may contain every path or only a deterministic sample of the
 * full simulation, depending on the value of `sampling_method`.
 */
export interface PathDataset {
  /**
   * Total number of paths simulated by the engine.
   */
  num_paths_total: number;
  /**
   * Captured paths in deterministic order.
   */
  paths: SimulatedPath[];
  /**
   * Metadata needed to interpret captured state vectors.
   */
  process_params: ProcessParams;
  /**
   * Sampling method used to retain `paths`.
   */
  sampling_method: PathSamplingMethod;
  [k: string]: unknown;
}
/**
 * A complete captured Monte Carlo path.
 *
 * Contains all captured points for a single simulated path, plus the final
 * discounted value used in summary statistics.
 */
export interface SimulatedPath {
  /**
   * Final discounted payoff value for this path.
   *
   * This is the path-level amount after the engine applies the run's
   * `discount_factor`.
   */
  final_value: number;
  /**
   * Internal rate of return inferred from the captured cashflow amounts, if calculable.
   */
  irr?: number | null;
  /**
   * Path identifier (0-indexed)
   */
  path_id: number;
  /**
   * Time points along the path
   */
  points: PathPoint[];
  [k: string]: unknown;
}
/**
 * A single captured point along a Monte Carlo path.
 *
 * Captures the process state at a specific time step, together with any
 * cashflows emitted at that step and, optionally, a payoff snapshot.
 *
 * # State Vector Layout
 *
 * The `state` vector contains the raw state variables in process-defined order.
 * For simple single-asset models, the crate's internal `state_indices`
 * constants provide common aliases.
 * For multi-asset or process-specific layouts, consult
 * [`PathDataset::process_params.factor_names`](PathDataset::process_params) or
 * [`PathDataset::state_var_keys`].
 */
export interface PathPoint {
  /**
   * Typed cashflows generated at this time step as `(time, amount, type)`.
   *
   * Amounts follow the sign convention `positive = inflow`,
   * `negative = outflow`.
   */
  cashflows?: [unknown, unknown, unknown][];
  /**
   * Optional payoff snapshot at this point.
   *
   * This is populated only when path capture requested payoff snapshots. It
   * uses the payoff's native amount units and is not additionally discounted
   * inside `PathPoint`.
   */
  payoff_value?: number | null;
  /**
   * State variables at this point (spot, variance, rate, etc.)
   * Indexed by position - see `state_indices` for standard layout
   */
  state: number[];
  /**
   * Time step index (0 = initial, N = final)
   */
  step: number;
  /**
   * Time in years from valuation date
   */
  time: number;
  [k: string]: unknown;
}
/**
 * Metadata describing the process behind a captured dataset.
 *
 * This structure is typically populated by
 * [`crate::monte_carlo::process::metadata::ProcessMetadata`] implementations and stored in a
 * [`PathDataset`]. It describes how to interpret captured state vectors rather
 * than how to price the instrument.
 */
export interface ProcessParams {
  /**
   * Optional row-major `n x n` correlation matrix.
   */
  correlation?: number[] | null;
  /**
   * Names describing the order of captured state-vector entries.
   */
  factor_names: string[];
  /**
   * Process parameters keyed by implementation-defined names such as `r`,
   * `q`, `sigma`, `kappa`, or `theta`.
   */
  parameters: {
    [k: string]: number;
  };
  /**
   * Process type identifier, such as `"GBM"` or `"Heston"`.
   */
  process_type: string;
  [k: string]: unknown;
}
/**
 * Reproducibility metadata stamped on a Monte Carlo pricing run.
 *
 * Records the execution policy that produced an estimate so results are
 * auditable and replayable: the same `(seed, num_paths, num_steps,
 * chunk_size)` reproduces the run bit-for-bit regardless of thread count or
 * host (see the determinism notes on [`crate::monte_carlo::engine::McEngine::price`]).
 */
export interface RunMetadata {
  /**
   * Whether antithetic variates were enabled.
   */
  antithetic?: boolean;
  /**
   * Effective chunk size of the deterministic reduction tree.
   */
  chunk_size?: number;
  /**
   * Number of time-grid steps.
   */
  num_steps?: number;
  /**
   * Root RNG seed, when the calling pricer derived the stream from one.
   *
   * `None` when the engine was driven by an externally constructed
   * [`crate::monte_carlo::traits::RandomStream`] whose seed the engine cannot observe.
   */
  seed?: number | null;
  /**
   * Whether the run used the parallel execution path.
   */
  use_parallel?: boolean;
  [k: string]: unknown;
}
/**
 * Result for a single path valuation.
 *
 * Contains the present value, optional 3-factor path data, and the detailed cashflow schedule.
 */
export interface PathResult {
  /**
   * Cashflow schedule for this path
   */
  cashflows: CashFlowSchedule;
  /**
   * Value to the lender of the path's draws having been made at the
   * contractual margin instead of the path's fair spread. The fair spread
   * is anchored to the margin at the valuation date, so each draw is a
   * forward loan to maturity worth `−ΔD · (s − s₀) · risky annuity`,
   * summed over the path's draws. Negative when the spread has widened
   * since the valuation date (the option the borrower holds has been
   * exercised against the lender); zero for deterministic schedules and
   * for any constant spread process. Loan-equivalent draws at default
   * belong to the default leg and are excluded.
   */
  draw_option_cost: Money;
  /**
   * 3-factor path data (if from MC)
   */
  path_data?: ThreeFactorPathData | null;
  /**
   * Present value for this path
   */
  pv: Money;
  [k: string]: unknown;
}
/**
 * Path data from 3-factor Monte Carlo simulation.
 *
 * Contains the full trajectory of utilization, interest rates, and credit spreads
 * at each observation date (contractual accrual boundaries plus term-index
 * reset dates), enabling cashflow generation and survival probability
 * computation.
 */
export interface ThreeFactorPathData {
  /**
   * Credit spread trajectory (for survival probability)
   */
  credit_spread_path: number[];
  /**
   * Observation dates aligned with the trajectories: the contractual
   * accrual boundaries plus every term-index reset date inside the facility
   * life, sorted ascending. The name predates the separation of accrual,
   * reset and adjusted payment dates.
   */
  payment_dates: DateWire[];
  /**
   * Short rate trajectory (for floating rates)
   */
  short_rate_path: number[];
  /**
   * Whether `short_rate_path` was simulated by a stochastic rate process
   * (Hull-White with σ > 0). When true the pricer discounts pathwise on
   * the simulated bank account; when false the static discount curve is
   * used (deterministic-forward or fixed-rate modes).
   */
  stochastic_rates?: boolean;
  /**
   * Time points corresponding to each value: years from the commitment date
   * on the ACT/365F model clock (`MC_CLOCK_DAY_COUNT`).
   */
  time_points: number[];
  /**
   * Utilization trajectory at each payment date [0, 1]
   */
  utilization_path: number[];
  [k: string]: unknown;
}
/**
 * Equity-tranche return metrics: IRR, MOIC, NAV and cash-on-cash.
 */
export interface EquityMetrics {
  /**
   * Each distribution as a fraction of `invested`, by payment date.
   */
  cash_on_cash: [unknown, unknown][];
  /**
   * ISO-4217 code of the currency the amounts are denominated in.
   */
  currency: Currency;
  /**
   * Cash invested on the valuation date: current balance × purchase price.
   */
  invested: number;
  /**
   * Annualized IRR of `−invested` on the valuation date against every
   * projected distribution (XIRR); `None` when no rate solves.
   */
  irr?: number | null;
  /**
   * Multiple on invested capital: total distributions ÷ `invested`.
   */
  moic: number;
  /**
   * Present value of the distributions on the deal's discount curve, as a
   * percent of the equity's current balance.
   */
  nav_pct: number;
  /**
   * Identifier of the equity tranche.
   */
  tranche_id: string;
}
/**
 * Projected cashflows of an asset-backed facility and its residual.
 */
export interface FacilityProjection {
  /**
   * Commitment fee accrued on `commitment − opening facility balance` per
   * accrual period while the line revolves, paid on the period's payment
   * date.
   */
  commitment_fees: [unknown, unknown][];
  /**
   * Per-period deal record of the synthetic deal.
   */
  diagnostics: SimulationDiagnostics;
  /**
   * Lender draws applied (scheduled draws and re-advances) per payment
   * date: cash the lender advances, an outflow in the lender's IRR.
   */
  draws?: [unknown, unknown][];
  /**
   * Interest and principal paid to the facility note.
   */
  facility: TrancheCashflows;
  /**
   * Cash paid to the residual class.
   */
  residual: TrancheCashflows;
}
/**
 * Deal-level accounting produced alongside the tranche cashflows.
 */
export interface SimulationDiagnostics {
  /**
   * Collateral draws funded from principal collections over the simulation.
   */
  draws_from_principal: Money;
  /**
   * Collateral draws funded from the reserve account over the simulation.
   */
  draws_from_reserve: Money;
  /**
   * Payment date on which an early-amortization event (or a coverage-test
   * acceleration) ended the revolving period, when one fired.
   */
  early_amortization_date?: DateWire | null;
  /**
   * Per-period deal accounting, in payment order.
   */
  periods?: PeriodDiagnostics[];
  /**
   * Reserve-account balance at the end of each simulated period.
   */
  reserve_balance_path: [unknown, unknown][];
  /**
   * Reserve interest earned each period, before routing to its destination.
   */
  reserve_interest_paid: [unknown, unknown][];
  /**
   * Revolver repayments diverted to replenish the reserve over the simulation.
   */
  reserve_replenished: Money;
  /**
   * Lender draws applied to notes, as `(tranche id, payment date, amount)`.
   */
  tranche_draws?: [unknown, unknown, unknown][];
  /**
   * Collateral draws that could not be funded over the simulation.
   */
  unfunded_draws: Money;
  [k: string]: unknown;
}
/**
 * Deal accounting for one payment period, recorded after the waterfall.
 *
 * Balances are end-of-period; collections, defaults, recoveries and fees are
 * the period's amounts; composition statistics are balance-weighted over the
 * performing pool at period end.
 */
export interface PeriodDiagnostics {
  /**
   * Every coverage test the executor evaluated this period.
   */
  coverage_tests: CoverageTestDiagnostic[];
  /**
   * Par that defaulted (charged off) this period.
   */
  defaults: Money;
  /**
   * Delinquent collateral balance at period end (delinquency model).
   */
  delinquent_balance: Money;
  /**
   * Annualized excess spread realized this period (decimal).
   */
  excess_spread: number;
  /**
   * Fees paid through the waterfall this period.
   */
  fees_paid: Money;
  /**
   * Controlled-accumulation funding account at period end.
   */
  funding_account: Money;
  /**
   * Interest collected from the pool (including servicer advances).
   */
  interest_collections: Money;
  /**
   * Payment date of the period.
   */
  payment_date: DateWire;
  /**
   * Collateral balance at period end.
   */
  pool_balance: Money;
  /**
   * `pool_balance` divided by the original (cut-off) pool balance
   * (`AssetPool::original_balance_or_reconstructed`).
   */
  pool_factor: number;
  /**
   * Scheduled and prepaid principal collected from the pool.
   */
  principal_collections: Money;
  /**
   * Recovery cash released to the waterfall this period.
   */
  recoveries: Money;
  /**
   * Principal recycled into replacement collateral this period.
   */
  reinvested_par: Money;
  /**
   * Reserve account balance at period end.
   */
  reserve_balance: Money;
  /**
   * Servicer advances of missed principal and interest outstanding at
   * period end (not yet repaid by cures or liquidations).
   */
  servicer_advances_outstanding: Money;
  /**
   * Excess-spread account balance at period end.
   */
  spread_account: Money;
  /**
   * Weighted-average coupon (annual decimal) of the performing fixed-rate
   * collateral at period end: the same population as `AssetPool::wac`.
   */
  wac: number;
  /**
   * Balance-weighted Moody's rating factor of the collateral.
   */
  warf: number;
  /**
   * Balance-weighted spread of the floating-rate collateral (basis points).
   */
  weighted_avg_spread_bp: number;
  [k: string]: unknown;
}
/**
 * Result containing tranche-specific cashflows and metadata.
 */
export interface TrancheCashflows {
  /**
   * Contractual coupon periods with balances before projected principal events.
   */
  accrual_periods: TrancheAccrualPeriod[];
  /**
   * Cashflow schedule for this tranche (simple dated flows).
   */
  cashflows: [unknown, unknown][];
  /**
   * Interest DEFERRED to future periods on a non-PIK tranche.
   *
   * Non-PIK shortfalls used to be recorded in `pik_flows`. PIK means
   * the unpaid interest is CAPITALIZED into the tranche balance and accrues
   * thereafter; a non-PIK deferral is a separate senior claim that does not
   * touch notional. Conflating them misleads any consumer reading
   * `total_pik` as capitalized balance — the two have different effects on
   * notional, on later interest due, and on OC denominators.
   */
  deferred_flows?: [unknown, unknown][];
  /**
   * Detailed cashflows with proper classification using CFKind.
   */
  detailed_flows: CashFlow[];
  /**
   * Final tranche balance after all payments.
   */
  final_balance: Money;
  /**
   * Interest cashflows (component of total).
   */
  interest_flows: [unknown, unknown][];
  /**
   * PIK capitalization flows.
   */
  pik_flows: [unknown, unknown][];
  /**
   * Principal cashflows (component of total).
   */
  principal_flows: [unknown, unknown][];
  /**
   * Total interest deferred on a non-PIK tranche.
   */
  total_deferred: Money;
  /**
   * Total interest received.
   */
  total_interest: Money;
  /**
   * Total PIK capitalized.
   */
  total_pik: Money;
  /**
   * Total principal received.
   */
  total_principal: Money;
  /**
   * Total write-down (loss allocation).
   */
  total_writedown: Money;
  /**
   * Tranche identifier.
   */
  tranche_id: string;
  /**
   * Write-down flows (loss allocation reducing tranche balance).
   */
  writedown_flows: [unknown, unknown][];
}
/**
 * Contractual coupon accrual retained independently of paid or deferred cash.
 */
export interface TrancheAccrualPeriod {
  /**
   * Annual decimal coupon after any contractual available-funds cap.
   */
  coupon_rate: number;
  /**
   * Contractual convention used to accrue this note's coupon.
   */
  day_count: DayCount;
  /**
   * Unadjusted exclusive contractual accrual boundary.
   */
  end: DateWire;
  /**
   * Outstanding note balance at the start of the period, in note currency.
   */
  opening_balance: Money;
  /**
   * Adjusted date on which the coupon is payable.
   */
  payment_date: DateWire;
  /**
   * Unadjusted inclusive contractual accrual boundary.
   */
  start: DateWire;
}
/**
 * FX pair identifier using base/quote currency ordering.
 */
export interface FxPair {
  /**
   * Base currency (numerator).
   */
  base: Currency;
  /**
   * Quote currency (denominator).
   */
  quote: Currency;
}
/**
 * Market conventions of one inflation-swap market.
 */
export interface InflationSwapConventions {
  /**
   * Business day convention.
   */
  business_day_convention: BusinessDayConvention;
  /**
   * Calendar for payment/fixing.
   */
  calendar_id: string;
  /**
   * Day count for the fixed leg.
   */
  day_count: DayCount;
  /**
   * Inflation lag (observation lag) in months/period.
   */
  inflation_lag: Tenor;
  /**
   * Monthly reference CPI rule: interpolated daily for USD, monthly step for EUR/UK.
   */
  interpolation: InflationInterpolation;
  /**
   * Settlement lag in business days.
   */
  settlement_days: number;
  [k: string]: unknown;
}
/**
 * Native cashflow rows, reporting-currency PV and reconciliation status.
 */
export interface InstrumentCashflowEnvelope {
  /**
   * Valuation date.
   */
  as_of: DateWire;
  /**
   * Hazard curve ID (`credit_curve_id`) used (omitted for `discounting` model).
   */
  credit_curve_id?: Id | null;
  /**
   * Reporting currency used for row PVs and `total_pv`.
   */
  currency: Currency;
  /**
   * Discount curve ID used.
   */
  discount_curve_id: Id;
  /**
   * Per-row enriched cashflows.
   */
  flows: CashflowRow[];
  /**
   * Instrument identifier.
   */
  instrument_id: string;
  /**
   * Model key used (`"discounting"` or `"hazard_rate"`).
   */
  model: string;
  /**
   * `true` when `total_pv` agrees with the instrument's canonical
   * `base_value` (`Instrument::value`) within rounding tolerance. The
   * exporter returns an error instead of emitting a non-reconciling
   * envelope, so every successful response carries `true`.
   */
  reconciles_with_base_value: boolean;
  /**
   * Recovery rate from the hazard curve (omitted for `discounting` model).
   */
  recovery_rate?: number | null;
  /**
   * Sum of `flows[i].pv`. Matches `base_value` for supported products.
   */
  total_pv: number;
  [k: string]: unknown;
}
/**
 * Collection of curves used by an instrument, categorized by market role.
 */
export interface InstrumentCurves {
  /**
   * Credit/hazard curves used by the instrument.
   */
  credit_curves: Id[];
  /**
   * Discount curves used by the instrument (including primary and foreign).
   */
  discount_curves: Id[];
  /**
   * Forward/projection curves used by the instrument.
   */
  forward_curves: Id[];
  /**
   * Inflation curves or published inflation indices used by the instrument.
   */
  inflation_curves: Id[];
}
/**
 * Contract conventions of one interest-rate future.
 */
export interface IrFutureConventions {
  /**
   * Calendar for business day adjustments.
   */
  calendar_id: string;
  /**
   * Exchange-defined averaging or fixing method for final settlement.
   */
  compounding: FloatingLegCompounding;
  /**
   * Optional convexity adjustment in rate terms.
   */
  convexity_adjustment?: number | null;
  /**
   * Number of delivery months for the underlying rate period.
   */
  delivery_months: number;
  /**
   * Face value of the contract.
   */
  face_value: number;
  /**
   * Underlying rate index identifier.
   */
  index_id: Id;
  /**
   * Rule used to derive the rate reference period from the quoted expiry.
   */
  reference_period: IrFutureReferencePeriod;
  /**
   * Business-day lag from expiry to period start for forward-starting contracts.
   *
   * This must be zero for in-arrears reference-period rules.
   */
  settlement_days: number;
  /**
   * Tick size in price points.
   */
  tick_size: number;
  /**
   * Tick value in currency units.
   */
  tick_value: number;
  [k: string]: unknown;
}
/**
 * Listed-product catalog coverage for one exchange product.
 */
export interface ListedProductCoverage {
  /**
   * Rates, fixed income, equity, FX, commodity, volatility, or digital assets.
   */
  asset_class: string;
  /**
   * Exchange venue.
   */
  exchange: ListedExchange;
  /**
   * Exchange features exercised by this mapping.
   */
  features: string[];
  /**
   * Canonical valuation type selected by the library.
   */
  instrument_type: InstrumentType;
  /**
   * Human-readable exchange product family.
   */
  name: string;
  /**
   * Future, option on future, or direct option.
   */
  product_kind: ListedProductKind;
  /**
   * Residual feature not included in the canonical valuation, if any.
   */
  residual_gap?: string | null;
  /**
   * Current official exchange page used to verify the family.
   */
  source_url: string;
  /**
   * Native, composed, or partial coverage.
   */
  status: ListedCoverageStatus;
  /**
   * Exchange root symbols, comma-separated where one row covers a close family.
   */
  symbols: string;
}
/**
 * Market data an instrument needs to price: curves, spots, surfaces, FX pairs and fixings.
 */
export interface MarketDependencies {
  /**
   * Credit-index aggregates resolved through `MarketContext::get_credit_index`.
   *
   * These identifiers are distinct from direct hazard-curve IDs because a
   * credit index also carries base correlation and optional issuer curves.
   */
  credit_index_ids?: Id[];
  /**
   * Curve dependencies grouped by type.
   */
  curves: InstrumentCurves;
  /**
   * FX pairs required for pricing (spot matrices).
   */
  fx_pairs: FxPair[];
  /**
   * Scalar market-value identifiers resolved through `MarketContext::get_price`.
   *
   * This includes tradable spots and non-price unitless scalars such as
   * continuous dividend yields. [`Self::series_ids`] is reserved for
   * `MarketContext::get_series` dependencies.
   */
  market_scalar_ids: string[];
  /**
   * Scalar time series identifiers (e.g., OHLC price series for realized variance).
   */
  series_ids: string[];
  /**
   * Typed volatility dependencies in deterministic insertion order.
   */
  volatility_dependencies: VolatilityDependency[];
}
/**
 * A volatility-surface dependency with the context needed for diagnostics.
 */
export interface VolatilityDependency {
  /**
   * Optional contractual strike used by local volatility diagnostics.
   */
  reference_strike?: number | null;
  /**
   * Optional market-scalar id of the underlying spot paired with the surface.
   */
  spot_id?: Id | null;
  /**
   * Volatility surface identifier.
   */
  vol_surface_id: Id;
}
/**
 * Historical market scenarios for historical VaR and expected shortfall.
 */
export interface MarketHistory {
  /**
   * Base date (current market state reference point)
   */
  base_date: DateWire;
  /**
   * Historical scenarios (one per day in lookback window)
   * Ordered chronologically from oldest to newest
   */
  scenarios: MarketScenario[];
  /**
   * Historical window size in days
   */
  window_days: number;
}
/**
 * Collection of all risk factor shifts for a single historical date.
 *
 * Represents a complete market scenario that can be applied to revalue
 * a portfolio. Each scenario contains shifts for all relevant risk factors.
 */
export interface MarketScenario {
  /**
   * Historical date this scenario represents
   */
  date: DateWire;
  /**
   * All risk factor shifts on this date (relative to base date)
   */
  shifts: RiskFactorShift[];
}
/**
 * Historical shift for a single risk factor on a single date.
 *
 * Represents the change in a market variable from its base value.
 * For example, a +15bp shift in 5Y USD rates.
 */
export interface RiskFactorShift {
  /**
   * Risk factor being shifted
   */
  factor: RiskFactorType;
  /**
   * Absolute change in the factor
   * - For rates/spreads: change in basis points as decimal (e.g., 0.0015 = 15bp)
   * - For equity/FX spot: relative change (e.g., -0.025 = -2.5%)
   * - For volatility: absolute vol change (e.g., 0.02 = +2 vol points)
   */
  shift: number;
}
/**
 * Merton Monte Carlo price of a PIK or toggle bond with path statistics.
 */
export interface MertonMcResult {
  /**
   * Average PIK fraction across all coupon dates and paths.
   */
  average_pik_fraction: number;
  /**
   * Clean price as percentage of par: `dirty_price_pct` minus the accrued
   * interest at the valuation date.
   */
  clean_price_pct: number;
  /**
   * Dirty price as percentage of par: the mean simulated present value at
   * the valuation date, including the full next coupon.
   */
  dirty_price_pct: number;
  /**
   * Effective spread in basis points implied by MC price vs risk-free.
   */
  effective_spread_bp: number;
  /**
   * Expected loss as fraction of PIK-aware risk-free PV.
   *
   * Defined as `1 - mean_mc_pv / risk_free_pv` where the risk-free PV
   * accounts for the PIK schedule (accreted notional in the no-default
   * scenario). For Toggle periods, the risk-free scenario assumes cash
   * (zero hazard implies no PIK trigger).
   */
  expected_loss: number;
  /**
   * Expected shortfall at the 95% confidence level.
   */
  expected_shortfall_95: number;
  /**
   * Number of independent estimators used (antithetic pairs count once).
   */
  num_paths: number;
  /**
   * Path-level statistics.
   */
  path_statistics: PathStatistics;
  /**
   * Standard error of the clean price estimate (percentage of par).
   */
  standard_error: number;
  /**
   * Unexpected loss (standard deviation of path PVs / notional).
   */
  unexpected_loss: number;
}
/**
 * Path-level statistics from the Monte Carlo simulation.
 */
export interface PathStatistics {
  /**
   * Average default time (in years) among defaulted paths.
   */
  avg_default_time: number;
  /**
   * Average recovery rate (decimal fraction) among defaulted paths.
   */
  avg_recovery_rate: number;
  /**
   * Average terminal notional (reflects PIK accrual).
   */
  avg_terminal_notional: number;
  /**
   * Fraction of paths that defaulted.
   */
  default_rate: number;
  /**
   * Fraction of coupon dates where PIK was elected.
   */
  pik_exercise_rate: number;
}
/**
 * Canonical per-key metric interpretation for host presentation.
 */
export interface MetricMetadata {
  /**
   * Whether this is a DV01/CS01 bucket with nonempty identifier and bucket coordinates.
   */
  bucketed: boolean;
  /**
   * Decoded coordinate labels in original order; empty for scalar metrics.
   */
  components: string[];
  /**
   * Native display group; absent for a non-standard base metric.
   */
  group?: string | null;
  /**
   * Original canonical wire key, retained without renaming.
   */
  key: string;
  /**
   * Base metric name without composite coordinates.
   */
  metric: string;
  /**
   * Native unit family; custom or calculator-dependent units remain unknown.
   */
  unit: MetricUnit;
}
/**
 * Structured-credit option-adjusted spread and its solver diagnostics.
 */
export interface OasResult {
  /**
   * Target clean settlement price (% of current balance).
   */
  market_price: number;
  /**
   * Model clean settlement price (% of the tranche's CURRENT balance, the
   * factor-adjusted quote basis) at the solved OAS.
   */
  model_price: number;
  /**
   * Number of scenarios used.
   */
  num_paths: number;
  /**
   * Option-adjusted spread (decimal; `0.01` = 100 bp).
   */
  oas: number;
  /**
   * Monte-Carlo standard error of the mean price (% of current balance).
   */
  price_std_error: number;
  [k: string]: unknown;
}
/**
 * Market conventions of one interest-rate index.
 */
export interface RateIndexConventions {
  /**
   * Operating currency of the index.
   */
  currency: Currency;
  /**
   * Market standard day count convention.
   */
  day_count: DayCount;
  /**
   * Market-standard fixed leg day count.
   */
  default_fixed_leg_day_count: DayCount;
  /**
   * Market-standard fixed leg frequency.
   */
  default_fixed_leg_frequency: Tenor;
  /**
   * Typical payment frequency for swaps referencing this index.
   */
  default_payment_frequency: Tenor;
  /**
   * Business days between accrual end and payment.
   */
  default_payment_lag_days: number;
  /**
   * Business days between fixing and accrual start.
   */
  default_reset_lag_days: number;
  /**
   * Index category (Overnight vs Term).
   */
  kind: RateIndexKind;
  /**
   * Market-standard business day convention.
   */
  market_business_day_convention: BusinessDayConvention;
  /**
   * Market-standard calendar identifier.
   */
  market_calendar_id: string;
  /**
   * Market-standard spot settlement lag (business days).
   */
  market_settlement_days: number;
  /**
   * Methodology for compounding overnight rates (OIS only).
   */
  ois_compounding?: FloatingLegCompounding | null;
  /**
   * Index tenor (None for overnight indices).
   */
  tenor?: Tenor | null;
  [k: string]: unknown;
}
/**
 * One evaluated cell of the scenario table.
 */
export interface ScenarioCell {
  /**
   * Annual CDR (decimal) for this cell.
   */
  cdr: number;
  /**
   * Annual CPR (decimal) for this cell.
   */
  cpr: number;
  /**
   * Clean settlement price as a percentage of the tranche's CURRENT
   * balance (the factor-adjusted quote basis): the model dirty value at
   * buyer settlement less accrued, the same figure as
   * `TrancheMetrics::price_pct`.
   */
  price: number;
  /**
   * Loss severity (decimal) for this cell.
   */
  severity: number;
  /**
   * Weighted-average life in years.
   */
  wal: number;
  /**
   * Total principal writedown in currency units.
   */
  writedown: number;
}
/**
 * Structured-credit scenario grid results for one tranche.
 */
export interface ScenarioTable {
  /**
   * Evaluated cells, in CPR-major, then CDR, then severity order.
   */
  cells: ScenarioCell[];
  /**
   * Identifier of the tranche evaluated.
   */
  tranche_id: string;
}
/**
 * Market conventions of one swaption market.
 */
export interface SwaptionConventions {
  /**
   * Business day convention for dates.
   */
  business_day_convention: BusinessDayConvention;
  /**
   * Calendar for exercise and settlement.
   */
  calendar_id: string;
  /**
   * Fixed leg day count.
   */
  fixed_leg_day_count: DayCount;
  /**
   * Fixed leg payment frequency.
   */
  fixed_leg_frequency: Tenor;
  /**
   * Floating leg index (implies float leg conventions).
   */
  float_leg_index: string;
  /**
   * Settlement lag in business days.
   */
  settlement_days: number;
  [k: string]: unknown;
}
/**
 * Per-tranche risk and spread metrics from the tranche's own projected cashflows.
 */
export interface TrancheMetrics {
  /**
   * Effective convexity (years²) from the same ±1 bp re-projection.
   */
  convexity: number;
  /**
   * Credit-spread DV01 — currency change for a +1 bp z-spread shock. Negative
   * for a long tranche (wider spreads reduce PV).
   */
  cs01: number;
  /**
   * ISO-4217 code of the currency `pv` and `cs01` are denominated in.
   */
  currency: Currency;
  /**
   * Discount margin to maturity at `target_price_pct` (basis points): the
   * constant spread over the deal discount curve that reprices the
   * floater's projected cashflows; `None` for fixed-rate tranches.
   */
  dm_bp?: number | null;
  /**
   * Discount margin to the assumed call (basis points); floating-rate
   * tranches only.
   */
  dm_to_call_bp?: number | null;
  /**
   * Pool factor of the note: `current_balance / original_balance`, so a
   * price on original face is `price_pct * factor`.
   */
  factor: number;
  /**
   * Effective (rate) duration (years): the tranche is re-projected with
   * every rate curve in the market (discount and forward) bumped ±1 bp in
   * parallel and the dirty settlement values differenced, so a floater's
   * coupon resets move with the curve and its duration is short, while a
   * fixed-coupon note's equals its modified duration.
   */
  modified_duration: number;
  /**
   * Model clean settlement price as a percentage of the tranche's CURRENT
   * balance (the factor-adjusted secondary-market quote basis).
   */
  price_pct: number;
  /**
   * Present value of the tranche (currency units).
   */
  pv: number;
  /**
   * Spread convexity (years²): second-order z-spread sensitivity of the
   * projected cashflows at the solved z-spread, on the same kernel as
   * `spread_duration`.
   */
  spread_convexity: number;
  /**
   * Spread duration (years): `-CS01 / (dirty settlement target · 1bp)`.
   */
  spread_duration: number;
  /**
   * Price the z-spread/CS01 were solved against (% of current balance) —
   * the supplied market price, or the model price when none was given.
   */
  target_price_pct: number;
  /**
   * Identifier of the tranche.
   */
  tranche_id: string;
  /**
   * Weighted-average life (years).
   */
  wal: number;
  /**
   * Weighted-average life to the deal's assumed call (years); `None`
   * without a `call_assumption` covering this tranche.
   */
  wal_to_call?: number | null;
  /**
   * Z-spread to `target_price_pct` (basis points): the constant spread over the
   * discount curve equating the tranche's PV to that price. Zero when solved
   * against the tranche's own model price (no spread to its curve-discounted value).
   */
  z_spread_bp: number;
  /**
   * Z-spread to the assumed call at `target_price_pct` (basis points).
   */
  z_spread_to_call_bp?: number | null;
}
/**
 * Bounded structured diagnostics emitted by persisted-contract validation.
 */
export interface ValidationReport {
  /**
   * Individual findings retained up to the configured diagnostic limit.
   */
  diagnostics: Diagnostic[];
  /**
   * Whether one or more additional findings were omitted due to the limit.
   *
   * A truncated report with no retained diagnostics is treated as fatal
   * because the wire format cannot distinguish omitted warnings from errors.
   */
  truncated: boolean;
}
/**
 * Market conventions of one cross-currency swap pair.
 */
export interface XccyConventions {
  /**
   * Base-currency calendar identifier for business day adjustments.
   */
  base_calendar_id: string;
  /**
   * Base (foreign) currency of the pair.
   */
  base_currency: Currency;
  /**
   * Rate index identifier for the base-currency floating leg.
   */
  base_index_id: Id;
  /**
   * Business day convention for schedule and settlement dates.
   */
  business_day_convention: BusinessDayConvention;
  /**
   * Accrual day count convention.
   */
  day_count: DayCount;
  /**
   * Notional-exchange behaviour for this currency pair.
   *
   * For G10 pairs against USD (where `quote_currency = USD`) the dealer convention
   * is `MtmResetting { resetting_side: Leg1 }`: the base-currency leg (leg1) has its
   * notional re-marked each period to match the constant quote-currency (leg2)
   * notional in current FX. Pair conventions registered the other way around (USD
   * as base) must invert this to `Leg2`. Registry entries must state it
   * explicitly.
   */
  notional_exchange: NotionalExchange;
  /**
   * Coupon payment frequency for both legs.
   */
  payment_frequency: Tenor;
  /**
   * Quote-currency calendar identifier for business day adjustments.
   */
  quote_calendar_id: string;
  /**
   * Quote (domestic) currency of the pair.
   */
  quote_currency: Currency;
  /**
   * Rate index identifier for the quote-currency floating leg.
   */
  quote_index_id: Id;
  /**
   * Standard T+N spot settlement lag in business days.
   */
  settlement_days: number;
  [k: string]: unknown;
}
