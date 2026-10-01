// Generated from the finstack-quant-statements-analytics JSON schemas by scripts/generate-contract-types.mjs. Do not edit.
import type * as statements from '../statements/index.js';

/**
 * Type of balance sheet account.
 */
export type AccountType = "asset" | "liability" | "equity";
/**
 * Value that can be currency-aware or unitless.
 *
 * Used for node values that can represent:
 * - **Amount**: Currency-aware monetary values (e.g., USD 1,000,000)
 * - **Scalar**: Unitless values (e.g., ratios, percentages, counts)
 */
export type AmountOrScalar = Money | number;
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
 * Type-safe identifier for a node in a financial model.
 *
 * Serializes as a plain string and is interoperable with `&str` via
 * [`Borrow`] and [`AsRef`].
 */
export type NodeId = string;
/**
 * Identifier for a Gregorian period like `2025Q1` or a fiscal period like
 * `FY2025W53`.
 */
export type PeriodId = string;
/**
 * Tagged enum describing any built-in check in a serializable form.
 *
 * Each variant wraps its check struct, so the struct is the single schema;
 * the JSON shape is the struct's fields plus a `type` tag. Convert into a
 * boxed [`Check`] via [`BuiltinCheckSpec::to_check`].
 */
export type BuiltinCheckSpec =
  | (BalanceSheetArticulation & {
      type: "balance_sheet_articulation";
      [k: string]: unknown;
    })
  | (RetainedEarningsReconciliation & {
      type: "retained_earnings_reconciliation";
      [k: string]: unknown;
    })
  | (CashReconciliation & {
      type: "cash_reconciliation";
      [k: string]: unknown;
    })
  | (MissingValueCheck & {
      type: "missing_value";
      [k: string]: unknown;
    })
  | (SignConventionCheck & {
      type: "sign_convention";
      [k: string]: unknown;
    })
  | (NonFiniteCheck & {
      type: "non_finite";
      [k: string]: unknown;
    });
/**
 * Declared sign convention for a flow / magnitude input to a reconciliation
 * check.
 *
 * Each reconciliation declares its input expectation so callers can
 * validate data at construction time rather than at P&L
 * reconciliation. Previously each reconciliation embedded its sign
 * assumptions implicitly (e.g. `CapexReconciliation` expected
 * `capex_cf` to be a positive magnitude while INVARIANTS.md §3
 * documented the CFS convention as negative), which produced silent
 * cross-check failures on any model that used the other convention.
 *
 * # Variants
 *
 * * [`Self::MagnitudePositive`] — the value is a non-negative magnitude;
 *   direction (inflow vs outflow) is encoded in the reconciliation's
 *   formula via `+` / `-` operators. Used by capex, depreciation,
 *   dividends, and disposal magnitudes across the cross-statement
 *   reconciliations.
 * * [`Self::InflowPositive`] — the value is a signed flow where positive
 *   means "inflow" / "addition" and negative means "outflow" /
 *   "reduction". Used by `total_cash_flow` in `CashReconciliation` and
 *   `net_income` in `RetainedEarningsReconciliation`.
 */
export type SignConventionPolicy = "magnitude_positive" | "inflow_positive";
/**
 * Scope that determines which periods a check applies to.
 */
export type PeriodScope = "all_periods" | "actuals_only" | "forecast_only";
/**
 * CECL calculation methodology.
 *
 * [`CeclMethodology::PdLgdEad`] is the discounted PD-LGD-EAD integration;
 * [`CeclMethodology::Warm`] is the undiscounted weighted-average remaining
 * maturity loss-rate method. (A `Vintage` variant existed through 0.6.x but
 * was removed before it was ever implemented — no cohort data model exists
 * to support it honestly.)
 */
export type CeclMethodology = "pd_lgd_ead" | "warm";
/**
 * How the PD curve reverts from forecast to historical after the R&S period.
 */
export type ReversionMethod =
  | "immediate"
  | {
      linear: {
        /**
         * Reversion period in years (e.g., 1.0 = 1-year linear fade).
         */
        reversion_years: number;
        [k: string]: unknown;
      };
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
 * Severity level for a check finding, ordered from least to most severe.
 */
export type CheckSeverity = "info" | "warning" | "error";
/**
 * Opaque company identifier within a peer set.
 */
export type CompanyId = string;
/**
 * Status of a corkscrew validation run.
 */
export type CorkscrewStatus = "success" | "failed";
/**
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
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
 * Exit-multiple sensitivity shock shape.
 *
 * Absolute bumps are in multiple-turn units (e.g. `Absolute(1.0)` is
 * ±1.0x). Relative bumps are decimal fractions of the base multiple
 * (e.g. `Relative(0.10)` is ±10%).
 */
export type ExitMultipleBump =
  | {
      absolute: number;
    }
  | {
      relative: number;
    };
/**
 * Method for computing downturn LGD from base (through-the-cycle) LGD.
 */
export type DownturnMethod =
  | {
      stressed_approximation: {
        /**
         * Asset correlation (rho). Typical: 0.10-0.24 per Basel.
         */
        asset_correlation: number;
        /**
         * LGD sensitivity to systematic factor. Typical: 0.3-0.5.
         */
        lgd_sensitivity: number;
        /**
         * Stress quantile for downturn scenario. Typical: 0.999 (99.9th percentile).
         */
        stress_quantile: number;
      };
    }
  | {
      regulatory_floor: {
        /**
         * Flat add-on over base LGD. Typical: 0.05-0.10 (5-10pp).
         */
        add_on: number;
        /**
         * Absolute LGD floor. Typical: 0.10 for secured, 0.25 for unsecured.
         */
        floor: number;
      };
    };
/**
 * LGD methodology selection.
 *
 * Selects how the base LGD carried on an [`Exposure`] (or a scenario
 * [`MacroScenario::lgd_override`]) is converted into the effective LGD used
 * in the ECL calculation. Each non-default variant requires a companion
 * [`EclConfig`] field; [`EclConfig::validate`] enforces that the field is
 * present for the selected variant and absent otherwise (a "set but unused"
 * knob is rejected symmetrically with a "used but unset" one), so a
 * misconfigured methodology cannot be silently stamped into results.
 *
 * # Variants and formulas
 *
 * - [`LgdType::PointInTime`]: `LGD_eff = base`, where `base` is
 *   [`Exposure::lgd`] or the scenario [`MacroScenario::lgd_override`] when
 *   present. No companion field.
 * - [`LgdType::ThroughTheCycle`]: `LGD_eff = EclConfig::ttc_lgd`, a
 *   cycle-average LGD that pins the effective LGD regardless of `base`.
 *   Because the pin would silently discard a scenario `lgd_override`,
 *   combining `ThroughTheCycle` with any scenario override is a validation
 *   error (see [`compute_ecl_weighted`]).
 * - [`LgdType::Downturn`]: `LGD_eff = EclConfig::downturn_lgd.adjust(base)`,
 *   applied via [`DownturnLgd::adjust`] on top of `base` (so a scenario
 *   `lgd_override` sets the base and the downturn stress is layered on top
 *   of it):
 *   - Stressed (`DownturnMethod::StressedApproximation`):
 *     `LGD_eff = clamp(base + s·√ρ·Φ⁻¹(q)·√(base·(1−base)), 0, 1)`, core's
 *     documented mean-plus-Bernoulli-stdev approximation (see
 *     `core/src/credit/lgd/downturn.rs` Methodology Note; Frye & Jacobs
 *     (2012) is related literature only, not the formula implemented here).
 *   - Regulatory floor (`DownturnMethod::RegulatoryFloor`):
 *     `LGD_eff = clamp(max(base + add_on, floor), 0, 1)`.
 *
 * # References
 *
 * - Basel Framework CRE36.85-36.90 -- downturn LGD requirement and
 *   regulatory floors.
 * - Frye, J. & Jacobs, M. (2012). "Credit Loss and Systematic Loss Given
 *   Default." *Journal of Credit Risk*, 8(1), 109-140 (related literature
 *   only; see [`DownturnLgd`] for the exact formula implemented).
 */
export type LgdType = "point_in_time" | "through_the_cycle" | "downturn";
/**
 * IFRS 9 impairment stage for a credit exposure.
 *
 * Under IFRS 9, financial instruments are classified into three stages that
 * determine the ECL measurement horizon:
 *
 * - **Stage 1**: 12-month ECL (no significant increase in credit risk)
 * - **Stage 2**: Lifetime ECL (significant increase in credit risk detected)
 * - **Stage 3**: Lifetime ECL (credit-impaired, objective evidence of default)
 *
 * # References
 *
 * IFRS 9 Financial Instruments, Section 5.5 -- Impairment. `docs/REFERENCES.md#ifrs-9-impairment`
 */
export type Stage = "stage1" | "stage2" | "stage3";
/**
 * Rounding modes supported by the library.
 *
 * The variants mirror the most common conventions found in pricing engines.
 *
 * # Examples
 * ```rust
 * use finstack_quant_core::config::{FinstackConfig, RoundingMode};
 *
 * let mut cfg = FinstackConfig::default();
 * cfg.rounding.mode = RoundingMode::TowardZero;
 * assert!(matches!(cfg.rounding.mode, RoundingMode::TowardZero));
 * ```
 */
export type RoundingMode = "bankers" | "away_from_zero" | "toward_zero" | "floor" | "ceil";
/**
 * Finite JSON number that is strictly greater than zero.
 *
 * This type is used by serde field adapters so runtime deserialization and
 * generated schemas enforce the same positive-number contract.
 */
export type PositiveF64Wire = number;
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
 * Node computation type.
 *
 * Determines how a node's value is computed:
 * - **Value**: Only explicit values (actuals, assumptions)
 * - **Calculated**: Only formula-derived
 * - **Mixed**: Value OR Forecast OR Formula (precedence: Value > Forecast > Formula)
 */
export type NodeType = "value" | "calculated" | "mixed";
/**
 * Reason for stage assignment (audit trail).
 *
 * Each variant captures the specific values that triggered the classification,
 * enabling regulatory reporting and model governance review.
 */
export type StagingTrigger =
  | {
      dpd_stage3: {
        /**
         * Actual days past due.
         */
        dpd: number;
        /**
         * Configured threshold.
         */
        threshold: number;
      };
    }
  | {
      stage3_qualitative: {
        /**
         * Name of the default-evidence flag (e.g. `bankruptcy`,
         * `distressed_modification`, `cross_default`, or a user
         * supplied key from `other_default_evidence`).
         */
        flag: string;
      };
    }
  | {
      pd_delta_absolute: {
        /**
         * Actual PD delta (current minus origination).
         */
        delta: number;
        /**
         * Configured threshold.
         */
        threshold: number;
      };
    }
  | {
      pd_delta_relative: {
        /**
         * Actual ratio (current PD / origination PD).
         */
        ratio: number;
        /**
         * Configured threshold.
         */
        threshold: number;
      };
    }
  | {
      rating_downgrade: {
        /**
         * Actual downgrade notches.
         */
        notches: number;
        /**
         * Configured threshold.
         */
        threshold: number;
      };
    }
  | {
      dpd_stage2: {
        /**
         * Actual days past due.
         */
        dpd: number;
        /**
         * Configured threshold.
         */
        threshold: number;
      };
    }
  | {
      qualitative: {
        /**
         * Name of the active flag.
         */
        flag: string;
      };
    }
  | "no_trigger";
/**
 * Convention that determines how `growth_rate` compounds in a lease.
 *
 * - `AnnualEscalator` (default): `growth_rate` is applied once per
 *   **lease-start anniversary**, measured in model periods (every
 *   `periods_per_year()` periods from the segment start). Matches Argus /
 *   NCREIF annual bumps: rent is flat within a lease year and steps on the
 *   anniversary. The bump counter resets at each rent step.
 * - `PerPeriod`: `growth_rate` is applied every model period. Must be set
 *   explicitly; omitted / defaulted values are never per-period.
 */
export type LeaseGrowthConvention = "per_period" | "annual_escalator";
/**
 * Basis for management fee calculation.
 */
export type ManagementFeeBase = "egi" | "effective_rent";
/**
 * Identifies which metric to extract from CompanyMetrics.
 */
export type MetricExtractor =
  | {
      named: string;
    }
  | {
      multiple: Multiple;
    }
  | {
      custom: string;
    };
/**
 * Valuation multiple.
 *
 * Enterprise value multiples use EV as the numerator. Equity multiples
 * use market capitalization or share price. Credit multiples use spread
 * or yield as the numerator and a fundamental metric as denominator.
 */
export type Multiple =
  | "ev_ebitda"
  | "ev_revenue"
  | "ev_ebit"
  | "ev_fcf"
  | "pe"
  | "pb"
  | "ptbv"
  | "p_fcf"
  | "dividend_yield"
  | "spread_per_turn"
  | "yield_per_coverage";
/**
 * Time basis for computing a valuation multiple.
 */
export type PeriodBasis =
  | "ltm"
  | "ntm"
  | {
      custom: string;
    };
/**
 * Rich/cheap sign convention for a scoring dimension's Y metric.
 *
 * Determines how a higher-than-peers Y value maps onto the composite
 * score (positive = cheap). Spread- and yield-like metrics are cheap
 * when high ([`HigherIsCheap`](Self::HigherIsCheap)); valuation
 * multiples (P/E, EV/EBITDA) are rich when high
 * ([`HigherIsRich`](Self::HigherIsRich)).
 *
 * The direction is applied consistently to both the regression-residual
 * path and the univariate z-score path. The default,
 * `HigherIsCheap`, matches the historical regression-path convention
 * (positive residual = actual spread above fitted fair spread = cheap).
 */
export type ScoreDirection = "higher_is_cheap" | "higher_is_rich";
/**
 * Status of a scorecard run.
 */
export type ScorecardStatus = "success" | "failed";
/**
 * Sensitivity analysis mode.
 */
export type SensitivityMode = "diagonal" | "full_grid" | "tornado";
/**
 * Direction in which a metric should ideally move.
 */
export type TrendDirection = "increasing_is_good" | "decreasing_is_good";

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
 * Verifies that Assets = Liabilities + Equity for every period.
 */
export interface BalanceSheetArticulation {
  /**
   * Node IDs whose values represent total assets.
   */
  assets_nodes: NodeId[];
  /**
   * Node IDs whose values represent total equity.
   */
  equity_nodes: NodeId[];
  /**
   * Node IDs whose values represent total liabilities.
   */
  liabilities_nodes: NodeId[];
  /**
   * Tolerance override; falls back to
   * [`CheckConfig::default_tolerance`](crate::checks::CheckConfig::default_tolerance).
   */
  tolerance?: number | null;
  [k: string]: unknown;
}
/**
 * Bridge chart for a single target metric and period.
 */
export interface BridgeChart {
  /**
   * Baseline scenario label.
   */
  baseline_label: string;
  /**
   * Baseline value of the target metric.
   */
  baseline_value: number;
  /**
   * Comparison scenario label.
   */
  comparison_label: string;
  /**
   * Comparison value of the target metric.
   */
  comparison_value: number;
  /**
   * Period identifier.
   */
  period: PeriodId;
  /**
   * Ordered list of driver contributions.
   */
  steps: BridgeStep[];
  /**
   * Target metric identifier (e.g. `"ebitda"`).
   */
  target_metric: string;
  /**
   * Residual not explained by the driver deltas:
   * `(comparison_value - baseline_value) - Σ steps.contribution`.
   *
   * Driver contributions are raw deltas in *driver* units, not
   * sensitivities of the target metric, so they generally do not sum
   * to the target variance; this term makes the gap explicit instead
   * of leaving the bridge silently unbalanced.
   */
  unexplained?: number;
  [k: string]: unknown;
}
/**
 * Single driver contribution entry in a bridge chart.
 */
export interface BridgeStep {
  /**
   * Contribution of this driver to the total variance, in the same units
   * as the target metric.
   */
  contribution: number;
  /**
   * Driver node identifier (e.g. `"revenue"`).
   */
  driver: string;
  [k: string]: unknown;
}
/**
 * Checks that retained earnings flow correctly across periods:
 * RE(t) = RE(t−1) + NI(t) − Dividends(t) ± Adjustments(t).
 *
 * # Sign convention
 *
 * * `net_income_node` carries the [`SignConventionPolicy::InflowPositive`]
 *   convention — positive values increase RE (profit), negative values
 *   decrease it (loss). No validation is emitted for NI since both signs
 *   are meaningful.
 * * `dividends_node` carries the [`SignConventionPolicy::MagnitudePositive`]
 *   convention by default — the formula subtracts dividends explicitly.
 * * `other_adjustments` carry
 *   [`SignConventionPolicy::InflowPositive`] — they are signed amounts
 *   added directly.
 */
export interface RetainedEarningsReconciliation {
  /**
   * Optional node for dividends paid.
   */
  dividends_node?: NodeId | null;
  /**
   * Sign convention applied to the `dividends_node` input. Defaults
   * to [`SignConventionPolicy::MagnitudePositive`].
   * `net_income_node` and `other_adjustments` always use
   * `InflowPositive` by construction.
   */
  dividends_sign_convention?: SignConventionPolicy;
  /**
   * Node for net income.
   */
  net_income_node: NodeId;
  /**
   * Additional adjustment nodes (buybacks, AOCI, etc.).
   */
  other_adjustments?: NodeId[];
  /**
   * Node for retained earnings balance.
   */
  retained_earnings_node: NodeId;
  /**
   * Tolerance override; falls back to
   * [`CheckConfig::default_tolerance`](crate::checks::CheckConfig::default_tolerance).
   */
  tolerance?: number | null;
  [k: string]: unknown;
}
/**
 * Checks Cash(t) = Cash(t−1) + TotalCF(t) and optionally
 * TotalCF = CFO + CFI + CFF.
 */
export interface CashReconciliation {
  /**
   * Node for cash balance.
   */
  cash_balance_node: NodeId;
  /**
   * Optional node for cash from financing.
   */
  cff_node?: NodeId | null;
  /**
   * Optional node for cash from investing.
   */
  cfi_node?: NodeId | null;
  /**
   * Optional node for cash from operations.
   */
  cfo_node?: NodeId | null;
  /**
   * Tolerance override; falls back to
   * [`CheckConfig::default_tolerance`](crate::checks::CheckConfig::default_tolerance).
   */
  tolerance?: number | null;
  /**
   * Node for total cash flow.
   */
  total_cash_flow_node: NodeId;
  [k: string]: unknown;
}
/**
 * Flags required nodes that lack values in applicable periods.
 *
 * **Advisory-only**: findings are `Severity::Warning`, so
 * `CheckResult::passed` is always `true`; the check surfaces gaps without
 * failing a pipeline gate.
 */
export interface MissingValueCheck {
  /**
   * Nodes that must have values in every in-scope period.
   */
  required_nodes: NodeId[];
  /**
   * Which periods to inspect.
   */
  scope: PeriodScope;
  [k: string]: unknown;
}
/**
 * Flags values with unexpected signs (e.g., revenue < 0 or expense > 0).
 *
 * **Advisory-only**: every finding is `Severity::Warning`, so this check's
 * `CheckResult::passed` is always `true` and it never fails a pipeline gate.
 * Treat its findings as review prompts, not assertions.
 */
export interface SignConventionCheck {
  /**
   * Nodes expected to carry negative values.
   */
  negative_nodes?: NodeId[];
  /**
   * Nodes expected to carry positive values.
   */
  positive_nodes?: NodeId[];
  [k: string]: unknown;
}
/**
 * Detects NaN or infinite values in node results.
 *
 * **Gating**: findings are `Severity::Error`, so any non-finite cell sets
 * `CheckResult::passed` to `false` and fails a pipeline gate. Non-finite
 * values poison every downstream calculation, so they are never advisory.
 */
export interface NonFiniteCheck {
  /**
   * Specific nodes to check; if empty, all nodes in results are inspected.
   */
  nodes?: NodeId[];
  [k: string]: unknown;
}
/**
 * Verifies that cash-flow-statement capex equals the sum of PP&E additions and intangible additions.
 */
export interface CapexReconciliation {
  /**
   * Capex from the cash flow statement (investing section).
   */
  capex_cf_node: NodeId;
  /**
   * Optional intangible-asset additions node.
   */
  intangible_additions_node?: NodeId | null;
  /**
   * Optional PP&E additions node.
   */
  ppe_additions_node?: NodeId | null;
  /**
   * Sign convention applied to `capex_cf_node`, `ppe_additions_node`,
   * and `intangible_additions_node`. Defaults to
   * [`SignConventionPolicy::MagnitudePositive`].
   */
  sign_convention?: SignConventionPolicy;
  /**
   * Tolerance override; falls back to
   * [`finstack_quant_statements::checks::CheckConfig::default_tolerance`].
   */
  tolerance?: number | null;
  [k: string]: unknown;
}
/**
 * Configuration for CECL (US GAAP ASC 326) calculation.
 */
export interface CeclConfig {
  /**
   * Time bucket width in years (same as IFRS 9). Default: 0.25.
   * Must be finite and at least 0.0001 years (at most one million buckets
   * for the maximum supported 100-year maturity).
   */
  bucket_width_years: number;
  /**
   * Whether expected losses are discounted at the exposure EIR. Default:
   * true for continuity with the existing PD-LGD-EAD implementation.
   */
  discount_expected_losses?: boolean;
  /**
   * Reasonable and supportable (R&S) forecast period in years.
   * Beyond this horizon, PD reverts to historical average.
   * Typical range: 1-3 years.
   */
  forecast_horizon_years: number;
  /**
   * Historical long-run annual PD used after the R&S period.
   */
  historical_annual_pd: number;
  /**
   * DPD threshold at or above which an exposure is treated as
   * credit-impaired under CECL. Default: 90.
   */
  impaired_dpd_threshold?: number;
  /**
   * Whether objective qualitative default evidence triggers the
   * credit-impaired CECL shortcut. Default: true.
   */
  impaired_qualitative_triggers_enabled?: boolean;
  /**
   * Expected time to recovery / resolution for credit-impaired exposures,
   * in years. Impaired allowance is current EAD less expected recovery
   * `(1 - LGD) * EAD`, discounted to this horizon when discounting is enabled.
   * Default: 1.0.
   */
  impaired_time_to_recovery_years?: number;
  /**
   * CECL methodology selection.
   */
  methodology: CeclMethodology;
  /**
   * Reversion method: how PD transitions from R&S to historical.
   */
  reversion_method: ReversionMethod;
  /**
   * Macro scenario specifications (same structure as IFRS 9).
   */
  scenarios: MacroScenario[];
  /**
   * Average annual net charge-off rate (decimal) for the WARM method.
   * Required when `methodology == CeclMethodology::Warm`; rejected otherwise.
   */
  warm_annual_loss_rate?: number | null;
}
/**
 * A forward-looking macro scenario with a probability weight.
 *
 * Used for probability-weighted ECL calculation per IFRS 9 B5.5.42.
 */
export interface MacroScenario {
  /**
   * Scenario identifier (e.g., "base", "upside", "downside").
   */
  id: string;
  /**
   * Optional LGD override for this scenario (downturn LGD).
   */
  lgd_override?: number | null;
  /**
   * Probability weight in \[0, 1\]. All scenario weights must sum to 1.0.
   */
  weight: number;
}
/**
 * CECL result for a single exposure.
 */
export interface CeclResult {
  /**
   * Total lifetime ECL.
   */
  ecl: number;
  /**
   * Exposure identifier.
   */
  exposure_id: string;
  /**
   * ECL horizon in years (always remaining maturity).
   */
  horizon: number;
  /**
   * Methodology used.
   */
  methodology: CeclMethodology;
  [k: string]: unknown;
}
/**
 * Configuration parameters that govern check execution.
 *
 * Identity checks trigger a finding when
 * `|diff| > max(default_tolerance, default_relative_tolerance * |reference|)`,
 * so an analyst can set an absolute floor (currency units) that catches
 * micro-errors on small balances plus a relative ceiling that scales with
 * larger balance sheets. A per-check tolerance override bypasses both
 * defaults and is applied verbatim.
 *
 * All four fields are part of the public configuration surface and are
 * intended to be tuned per-deployment via JSON / serde overrides — e.g.,
 * raising `default_relative_tolerance` to `1e-4` (1 bp) when a downstream
 * process is known to round to the nearest hundred. The defaults
 * (`default_tolerance = 0.01`, `default_relative_tolerance = 1e-9`,
 * `materiality_threshold = 0.0`, `min_severity = Info`) are conservative:
 * the absolute cent floor catches micro-errors on small balances while the
 * relative component absorbs f64 accumulation noise on large ones.
 */
export interface CheckConfig {
  /**
   * Default **relative** tolerance as a fraction of the reference
   * denominator (e.g. 1e-6 ≈ "one basis point of a basis point"). Zero
   * disables relative tolerance.
   */
  default_relative_tolerance?: number;
  /**
   * Default **absolute** tolerance for equality comparisons, expressed in
   * the same currency units as the node values being compared (e.g. 0.01
   * means one cent when nodes are in whole dollars).
   */
  default_tolerance?: number;
  /**
   * Findings below this absolute materiality threshold are excluded from
   * reports. Error-severity findings are exempt: materiality filters
   * advisory noise, it never changes a check's pass/fail verdict.
   */
  materiality_threshold?: number;
  /**
   * Minimum severity a finding must have to appear in the report.
   */
  min_severity?: CheckSeverity;
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
 * Serializable descriptor for a [`CheckSuite`] that can be saved/loaded as
 * JSON for team-wide check policies.
 *
 * Both built-in and formula checks are resolved by [`CheckSuiteSpec::resolve`].
 */
export interface CheckSuiteSpec {
  /**
   * Built-in checks to include.
   */
  builtin_checks?: BuiltinCheckSpec[];
  /**
   * Suite configuration.
   */
  config?: CheckConfig;
  /**
   * Suite description.
   */
  description?: string | null;
  /**
   * User-defined formula checks.
   */
  formula_checks?: FormulaCheckSpec[];
  /**
   * Suite name.
   */
  name: string;
}
/**
 * Serializable, runnable user-defined formula-check specification.
 *
 * Full suite definitions (built-in + formula) can be stored as a single JSON
 * document and resolved directly with [`CheckSuiteSpec::resolve`].
 */
export interface FormulaCheckSpec {
  /**
   * Category grouping.
   */
  category: CheckCategory;
  /**
   * Statements DSL expression to evaluate (e.g. `"revenue > 0"`).
   *
   * Formula checks use the same evaluator as calculated statement nodes,
   * including time-series functions and `cs.*` capital-structure references.
   */
  formula: string;
  /**
   * Unique identifier for this check instance.
   */
  id: string;
  /**
   * Template for the finding message (`{period}` is replaced at runtime).
   */
  message_template: string;
  /**
   * Human-readable name shown in reports.
   */
  name: string;
  /**
   * Severity assigned to findings when the formula fails.
   */
  severity: CheckSeverity;
  /**
   * Numeric tolerance for floating-point comparisons.
   */
  tolerance?: number | null;
}
/**
 * Metrics for a single company in a peer set.
 *
 * All monetary values should be in the same currency before constructing
 * a `PeerSet`. Currency normalization is the caller's responsibility.
 * Ratios are plain scalars (e.g., `6.5` means 6.5x leverage).
 */
export interface CompanyMetrics {
  /**
   * Optional instrument-level attributes (sector, geography, rating).
   * Used by `PeerFilter` for inclusion/exclusion decisions.
   */
  attributes?: Attributes;
  /**
   * Book value of equity.
   */
  book_value?: number | null;
  /**
   * Arbitrary additional metrics keyed by name.
   * Used for custom multiples or regression factors.
   */
  custom?: {
    [k: string]: number;
  };
  /**
   * Dividends per share (annualized).
   */
  dividends_per_share?: number | null;
  /**
   * EBIT.
   */
  ebit?: number | null;
  /**
   * EBITDA (period basis determined by the PeerSet context).
   */
  ebitda?: number | null;
  /**
   * EBITDA margin (decimal, e.g., 0.25 = 25%).
   */
  ebitda_margin?: number | null;
  /**
   * Enterprise value.
   */
  enterprise_value?: number | null;
  /**
   * Company identifier.
   */
  id: CompanyId;
  /**
   * EBITDA / Interest Expense.
   */
  interest_coverage?: number | null;
  /**
   * Total debt / EBITDA.
   */
  leverage?: number | null;
  /**
   * Levered free cash flow.
   */
  lfcf?: number | null;
  /**
   * Equity market capitalization.
   */
  market_cap?: number | null;
  /**
   * Net income / earnings.
   */
  net_income?: number | null;
  /**
   * Option-adjusted spread in basis points.
   */
  oas_bp?: number | null;
  /**
   * Revenue.
   */
  revenue?: number | null;
  /**
   * Revenue growth rate (decimal, e.g., 0.05 = 5%).
   */
  revenue_growth?: number | null;
  /**
   * Share price.
   */
  share_price?: number | null;
  /**
   * Tangible book value.
   */
  tangible_book_value?: number | null;
  /**
   * Unlevered free cash flow.
   */
  ufcf?: number | null;
  /**
   * Yield to worst / yield to maturity.
   */
  yield_pct?: number | null;
  [k: string]: unknown;
}
/**
 * Configuration for a single corkscrew account.
 */
export interface CorkscrewAccount {
  /**
   * Account type (asset, liability, equity)
   */
  account_type: AccountType;
  /**
   * Optional: Node ID for beginning balance override
   */
  beginning_balance_node?: string | null;
  /**
   * Node IDs representing increases (or signed net changes) to the balance.
   *
   * Each change node is **added** to the prior (or beginning) balance.
   * Prefer [`Self::decreases`] for positive outflows so roll-forward
   * decrease nodes do not need to be negated.
   */
  changes?: string[];
  /**
   * Node IDs representing positive decreases (repayments, outflows,
   * disposals) subtracted from the balance.
   *
   * Identity: `expected = prev + Σ changes − Σ decreases`, or
   * `beginning + Σ changes − Σ decreases` when
   * [`Self::beginning_balance_node`] is set. Empty by default so existing
   * configs that store reductions as negative `changes` still parse.
   */
  decreases?: string[];
  /**
   * Node ID for the balance account
   */
  node_id: string;
}
/**
 * Configuration for corkscrew analysis.
 */
export interface CorkscrewConfig {
  /**
   * List of balance sheet accounts to validate
   */
  accounts?: CorkscrewAccount[];
  /**
   * Whether to fail on inconsistencies (default: false)
   */
  fail_on_error?: boolean;
  /**
   * Tolerance for rounding differences (default: 0.01)
   */
  tolerance?: number;
}
/**
 * Report produced by `CorkscrewExtension::execute`.
 */
export interface CorkscrewReport {
  /**
   * Structured output (e.g. per-account validations as JSON)
   */
  data?: {
    [k: string]: unknown;
  };
  /**
   * Errors (fatal in strict mode, otherwise reported)
   */
  errors?: string[];
  /**
   * Human-readable summary
   */
  message: string;
  /**
   * Overall execution status
   */
  status: CorkscrewStatus;
  /**
   * Warnings (non-fatal)
   */
  warnings?: string[];
  [k: string]: unknown;
}
/**
 * Unified analysis result combining statement, equity, and credit perspectives.
 */
export interface CorporateAnalysis {
  /**
   * Per-instrument coverage and leverage metrics.
   */
  credit: {
    [k: string]: CreditContextMetrics;
  };
  /**
   * Equity valuation result (if DCF was configured)
   */
  equity?: CorporateValuationResult | null;
  /**
   * `true` when a DCF enterprise value was computed but suppressed as the
   * LTV reference because it was non-positive. Credit metrics are then
   * computed without an enterprise-value reference.
   */
  ev_suppressed_non_positive: boolean;
  /**
   * Full statement evaluation (all nodes, all periods)
   */
  statement: statements.StatementResult;
  [k: string]: unknown;
}
/**
 * Per-instrument credit context metrics derived from statement data.
 *
 * Ratios are plain scalars: `2.0` means `2.0x` coverage and `0.40` means
 * `40%` loan-to-value.
 *
 * DSCR is reported in three flavours, all using cash flow available for debt
 * service (`cfads_node`) rather than EBITDA:
 *
 * - [`CreditContextMetrics::dscr`] / [`CreditContextMetrics::dscr_min`]:
 *   CFADS divided by cash interest plus principal.
 * - [`CreditContextMetrics::dscr_total`] /
 *   [`CreditContextMetrics::dscr_total_min`]: CFADS divided by total interest
 *   (cash plus PIK) and principal.
 * - [`CreditContextMetrics::dscr_incl_fees`] /
 *   [`CreditContextMetrics::dscr_incl_fees_min`]: CFADS divided by cash
 *   interest, principal, and fees.
 *
 * Interest coverage uses a separate `interest_coverage_node`, normally EBITDA
 * or EBIT, so changing the DSCR numerator cannot silently redefine interest
 * coverage.
 */
export interface CreditContextMetrics {
  /**
   * Cash DSCR by period: `CFADS / (interest_cash + principal)`.
   */
  dscr: [unknown, unknown][];
  /**
   * Fee-inclusive cash DSCR by period:
   * `CFADS / (interest_cash + principal + fees)`.
   */
  dscr_incl_fees: [unknown, unknown][];
  /**
   * Minimum fee-inclusive cash DSCR across all periods.
   */
  dscr_incl_fees_min?: number | null;
  /**
   * Minimum cash DSCR across all periods.
   */
  dscr_min?: number | null;
  /**
   * Total DSCR by period: `CFADS / (interest_total + principal)`.
   */
  dscr_total: [unknown, unknown][];
  /**
   * Minimum total DSCR across all periods.
   */
  dscr_total_min?: number | null;
  /**
   * Interest coverage by period:
   * `interest_coverage_node_value / interest_expense_total`.
   */
  interest_coverage: [unknown, unknown][];
  /**
   * Minimum interest coverage across all periods.
   */
  interest_coverage_min?: number | null;
  /**
   * LTV by period: `debt_balance[t] / reference_value[t]`.
   *
   * A scalar DCF enterprise value is broadcast as the same denominator
   * on every requested period (current valuation versus forward debt,
   * not a rolled enterprise-value path). A per-period statement node
   * supplies a varying value path; a missing period omits LTV for that
   * period only.
   */
  ltv: [unknown, unknown][];
  /**
   * Periods requested but excluded from one or more coverage series.
   *
   * Missing cashflows, missing numerator values, and non-positive
   * denominators exclude a period. Non-finite and cross-currency inputs are
   * errors rather than skipped observations.
   */
  skipped_periods?: PeriodId[];
  [k: string]: unknown;
}
/**
 * Corporate valuation result containing DCF outputs.
 *
 * Monetary outputs are returned in the model currency inferred from
 * `FinancialModelSpec::meta["currency"]`. Ratios such as
 * `equity_value_per_share` are plain scalars.
 */
export interface CorporateValuationResult {
  /**
   * Diluted share count (if shares_outstanding was provided)
   */
  diluted_shares?: number | null;
  /**
   * Enterprise value (PV of all cash flows + terminal value)
   */
  enterprise_value: Money;
  /**
   * Equity value (EV - Net Debt, after discounts)
   */
  equity_value: Money;
  /**
   * Equity value per diluted share (if shares_outstanding was provided)
   */
  equity_value_per_share?: number | null;
  /**
   * Net debt (or effective bridge amount) used in calculation
   */
  net_debt: Money;
  /**
   * Terminal value (present value)
   */
  terminal_value_pv: Money;
  [k: string]: unknown;
}
/**
 * Optional DCF, coverage, check-suite, valuation-date and LTV settings of the corporate analysis pipeline.
 */
export interface CorporateAnalysisOptions {
  /**
   * Valuation date (ISO `YYYY-MM-DD`); required when a market context is
   * supplied.
   */
  as_of?: DateWire | null;
  /**
   * Statement node supplying cash flow available for debt service; `None`
   * leaves DSCR out of the credit metrics.
   */
  cfads_node?: string | null;
  /**
   * Check suite run against the statement evaluation. DCF and
   * capital-structure analyses require one that includes the `non_finite`
   * built-in check.
   */
  check_suite?: CheckSuiteSpec | null;
  /**
   * Statement node used as the interest-coverage numerator; `None` uses
   * `"ebitda"`.
   */
  interest_coverage_node?: string | null;
  /**
   * Statement node supplying a per-period LTV denominator; `None`
   * broadcasts a positive DCF enterprise value instead.
   */
  ltv_value_node?: string | null;
  /**
   * Flat net debt in model currency used instead of the model-derived
   * equity bridge; only read when `wacc` is set.
   */
  net_debt_override?: number | null;
  /**
   * Terminal-value method for the DCF; required when `wacc` is set and
   * ignored otherwise.
   */
  terminal_value?: TerminalValueSpec | null;
  /**
   * Weighted average cost of capital as a decimal (`0.10` = 10%). Setting it
   * enables the DCF equity valuation and requires `terminal_value`.
   */
  wacc?: number | null;
}
/**
 * Flags periods where a coverage ratio (e.g.
 */
export interface CoverageFloorCheck {
  /**
   * Denominator node (e.g. interest expense, total debt service).
   */
  denominator_node: NodeId;
  /**
   * Minimum ratio before an error is issued.
   */
  min_error: number;
  /**
   * Minimum ratio before a warning is issued.
   */
  min_warning: number;
  /**
   * Numerator node (e.g. EBITDA, cash flow available for debt service).
   */
  numerator_node: NodeId;
  [k: string]: unknown;
}
/**
 * Structured credit assessment: leverage, interest coverage, and free cash flow at a `period` plus a per-period series for trend display.
 */
export interface CreditAssessment {
  /**
   * Free cash flow at `period`.
   */
  free_cash_flow?: number | null;
  /**
   * Interest coverage at `period`.
   */
  interest_coverage?: number | null;
  /**
   * Leverage ratio at `period`.
   */
  leverage_ratio?: number | null;
  /**
   * Assessment period rendered as a string (e.g. `"2025Q4"`).
   */
  period: string;
  /**
   * Per-period series (ascending) for trend display.
   */
  series: CreditAssessmentPoint[];
  [k: string]: unknown;
}
/**
 * One period's structured credit metrics. Each metric is `None` when it
 * cannot be computed for that period (e.g. an incomplete TTM window).
 */
export interface CreditAssessmentPoint {
  /**
   * Free cash flow at this period.
   */
  free_cash_flow?: number | null;
  /**
   * TTM EBITDA / TTM interest expense.
   */
  interest_coverage?: number | null;
  /**
   * Total debt / TTM EBITDA.
   */
  leverage_ratio?: number | null;
  /**
   * Period identifier rendered as a string (e.g. `"2025Q4"`).
   */
  period: string;
  [k: string]: unknown;
}
/**
 * Maps node IDs for credit underwriting analysis.
 *
 * Used by [`super::suites::credit_underwriting_checks`] to build leverage,
 * coverage, cash-flow, and trend checks. Unknown JSON keys are rejected, so
 * a misspelled optional key fails instead of silently disabling its check.
 */
export interface CreditMapping {
  /**
   * Monthly cash burn node (for liquidity runway).
   */
  cash_burn_node?: NodeId | null;
  /**
   * Cash / liquidity node.
   */
  cash_node?: NodeId | null;
  /**
   * Minimum coverage ratio that triggers a warning; defaults to `1.5`.
   */
  coverage_min_warn?: number | null;
  /**
   * Total debt node.
   */
  debt_node: NodeId;
  /**
   * EBITDA node.
   */
  ebitda_node: NodeId;
  /**
   * Free cash flow node.
   */
  fcf_node?: NodeId | null;
  /**
   * Interest expense node.
   */
  interest_expense_node: NodeId;
  /**
   * `(min, max)` leverage warning range; defaults to `(0.0, 6.0)`.
   *
   * @minItems 2
   * @maxItems 2
   */
  leverage_warn?: [unknown, unknown] | null;
}
/**
 * Optional configuration for DCF valuation beyond the core WACC/terminal parameters.
 */
export interface DcfOptions {
  /**
   * Explicit discount curve id stamped on the DCF instrument (default:
   * `None`).
   *
   * The curve is used for *risk attribution only* — all DCF components
   * (explicit flows, terminal value, equity) always discount at the
   * WACC on a single consistent basis. When `None`, a model-scoped
   * placeholder id (`"{model_id}-DCF-WACC"`) is used instead of the
   * conventional `"{CCY}-DISCOUNT"` name, so a market context that
   * happens to contain a curve with the conventional name cannot be
   * mistaken for the discounting basis.
   */
  discount_curve_id?: Id | null;
  /**
   * Structured equity bridge; when `Some` it is used as-is and the model's
   * debt/cash (and any net-debt override) are not consulted.
   */
  equity_bridge?: EquityBridge | null;
  /**
   * Exit-multiple sensitivity bump (default: `ExitMultipleBump::Absolute(1.0)`).
   *
   * Use `ExitMultipleBump::Relative` for a proportional shock,
   * e.g. `Relative(0.10)` for ±10% of the base multiple. Absolute
   * bumps are clamped at zero on the downside.
   */
  exit_multiple_bump?: ExitMultipleBump;
  /**
   * Statement flow node whose complete trailing calendar year supplies the
   * [`TerminalValueSpec::ExitMultiple`] metric (default: `None`).
   *
   * When `Some`, sum contiguous actual and forecast periods over the year ending
   * at the last forecast boundary. Values must be finite and monetary. Missing
   * history or a partial boundary requires an explicit annual metric. When `None`, the
   * spec's explicit `terminal_metric` is used. Ignored for Gordon
   * Growth and H-Model terminals.
   */
  exit_multiple_metric_node?: string | null;
  /**
   * Maximum perpetual stable growth rate accepted by Gordon Growth and the
   * H-Model (default `0.05` = 5%).
   *
   * This is a policy ceiling, not merely the mathematical `g < WACC`
   * constraint. Set it to the valuation's defensible long-run nominal
   * economy or risk-free growth assumption.
   */
  max_stable_growth_rate?: number;
  /**
   * Enable mid-year discounting convention (default: false).
   */
  mid_year_convention?: boolean;
  /**
   * Basic shares outstanding for per-share value.
   */
  shares_outstanding?: number | null;
  /**
   * Private company valuation discounts (DLOM, DLOC).
   *
   * Discounts apply after the DCF EV-to-equity bridge. The reported DCF
   * enterprise value remains pre-discount and will not reconcile to
   * discounted equity value plus net debt when discounts are present.
   */
  valuation_discounts?: ValuationDiscounts | null;
  /**
   * Minimum spread between WACC-down and the Gordon / H-Model growth
   * rate in the terminal-value formula (default `0.005` = 50 bp).
   *
   * When `wacc - bump < growth + epsilon`, the down-shock is clamped
   * to `growth + epsilon` so the `1/(wacc - g)` denominator stays
   * well defined. The clamp is reported in the trace so the caller
   * knows the bump was shortened.
   */
  wacc_denominator_epsilon?: number;
  /**
   * WACC sensitivity bump, in decimal (default `0.01` = ±100 bp).
   *
   * The down-shock is clamped so the terminal-value denominator
   * `wacc - g` cannot collapse (the resulting EV would blow up and
   * obscure the true sensitivity). See
   * [`DcfOptions::wacc_denominator_epsilon`].
   */
  wacc_sensitivity_bump?: number;
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
 * Enterprise-value tornado for the headline DCF assumptions.
 */
export interface DcfSensitivityResult {
  /**
   * Unshocked enterprise value the tornado deltas are measured against.
   */
  baseline_enterprise_value: Money;
  /**
   * Tornado entries sorted by descending absolute swing (`NaN` last).
   *
   * `downside` and `upside` are enterprise-value **deltas** versus
   * `baseline_enterprise_value`, following the convention of
   * [`crate::analysis::generate_tornado_entries`]: `downside` is the
   * impact with the parameter at its shocked minimum and `upside` the
   * impact at its shocked maximum. A lower WACC raises enterprise value,
   * so the WACC entry's `downside` is typically positive.
   */
  entries: TornadoEntry[];
  /**
   * Effective up-shocked terminal growth rate, when a growth-perpetuity
   * terminal value is in use (`None` for exit-multiple terminal values).
   */
  terminal_growth_up?: number | null;
  /**
   * `true` when the terminal-growth up-shock was shortened by the clamp.
   */
  terminal_growth_up_clamped: boolean;
  /**
   * Effective down-shocked WACC after the growth-denominator clamp.
   */
  wacc_down: number;
  /**
   * `true` when the WACC down-shock was shortened by the clamp.
   */
  wacc_down_clamped: boolean;
  [k: string]: unknown;
}
/**
 * Entry in a tornado chart representing one parameter's impact on a metric.
 */
export interface TornadoEntry {
  /**
   * Downside impact (metric change when parameter is at its minimum).
   */
  downside: number;
  /**
   * Parameter node identifier.
   */
  parameter_id: string;
  /**
   * Upside impact (metric change when parameter is at its maximum).
   */
  upside: number;
  [k: string]: unknown;
}
/**
 * Hierarchical dependency tree structure.
 */
export interface DependencyTree {
  /**
   * Child dependencies
   */
  children: DependencyTree[];
  /**
   * Formula text (if node is calculated)
   */
  formula?: string | null;
  /**
   * Node identifier
   */
  node_id: string;
}
/**
 * Verifies PP&E(t) = PP&E(t−1) + Capex(t) − D&A(t) − Disposals(t).
 */
export interface DepreciationReconciliation {
  /**
   * Capital expenditures node (cash flow statement / investing).
   */
  capex_node: NodeId;
  /**
   * D&A expense node (income statement).
   */
  depreciation_expense_node: NodeId;
  /**
   * Optional asset disposals node.
   */
  disposals_node?: NodeId | null;
  /**
   * PP&E balance node (balance sheet).
   */
  ppe_node: NodeId;
  /**
   * Sign convention applied to `capex_node`,
   * `depreciation_expense_node`, and `disposals_node`. Defaults to
   * [`SignConventionPolicy::MagnitudePositive`].
   */
  sign_convention?: SignConventionPolicy;
  /**
   * Tolerance override; falls back to
   * [`finstack_quant_statements::checks::CheckConfig::default_tolerance`].
   */
  tolerance?: number | null;
  [k: string]: unknown;
}
/**
 * Decomposed score for a single dimension.
 */
export interface DimensionScore {
  /**
   * Label of the dimension.
   */
  label: string;
  /**
   * Percentile rank of the subject's Y within peers (0-1). Whether a
   * high percentile means rich or cheap depends on the dimension's
   * [`ScoringDimension::direction`].
   */
  percentile: number;
  /**
   * R-squared of the regression (confidence measure).
   */
  r_squared?: number | null;
  /**
   * Raw regression residual in Y units (actual − fitted). Positive
   * means the subject's Y is above the peer fair-value line; the
   * composite uses the standardized, direction-adjusted form.
   */
  regression_residual?: number | null;
  /**
   * Dimension weight in composite.
   */
  weight: number;
  /**
   * Z-score of the subject's Y relative to the peer distribution
   * (raw, before the direction convention is applied).
   */
  z_score: number;
  [k: string]: unknown;
}
/**
 * Verifies that dividends on the cash flow statement equal the dividends charged against equity.
 */
export interface DividendReconciliation {
  /**
   * Dividends paid node (cash flow statement, financing).
   */
  dividends_cf_node: NodeId;
  /**
   * Dividends node (equity / retained earnings schedule).
   */
  dividends_equity_node: NodeId;
  /**
   * Sign convention applied to both dividend inputs. Defaults to
   * [`SignConventionPolicy::MagnitudePositive`].
   */
  sign_convention?: SignConventionPolicy;
  /**
   * Tolerance override; falls back to
   * [`finstack_quant_statements::checks::CheckConfig::default_tolerance`].
   */
  tolerance?: number | null;
  [k: string]: unknown;
}
/**
 * Downturn LGD adjuster.
 *
 * Wraps a base LGD estimate and applies a downturn adjustment method
 * to produce a stressed LGD for capital calculations.
 */
export interface DownturnLgd {
  /**
   * Downturn adjustment method.
   */
  method: DownturnMethod;
}
/**
 * ECL result for a single time bucket.
 */
export interface EclBucket {
  /**
   * Discount factor at the bucket midpoint, or at recovery for Stage 3.
   */
  discount_factor: number;
  /**
   * EAD used for this bucket.
   */
  ead: number;
  /**
   * ECL contribution from this bucket.
   */
  ecl: number;
  /**
   * LGD used for this bucket.
   */
  lgd: number;
  /**
   * Unconditional default probability for the bucket,
   * `cumPD(t_end) - cumPD(t_start)`. This is the quantity that
   * multiplies `LGD * EAD * DF` for performing exposures. Stage 3
   * sets it to one and measures EAD less discounted recoveries.
   */
  marginal_pd: number;
  /**
   * End of the time bucket (years).
   */
  t_end: number;
  /**
   * Start of the time bucket (years).
   */
  t_start: number;
  [k: string]: unknown;
}
/**
 * Configuration for ECL calculation.
 */
export interface EclConfig {
  /**
   * Time bucket width in years for the PD-LGD-EAD integration.
   * Must be finite and at least 0.0001 years, limiting a 100-year
   * exposure to one million buckets. Default: quarterly (0.25).
   */
  bucket_width_years: number;
  /**
   * Downturn LGD adjuster applied when `lgd_type == `[`LgdType::Downturn`].
   */
  downturn_lgd?: DownturnLgd | null;
  /**
   * LGD methodology label; see [`LgdType`] for the formulas each variant
   * applies and the companion field it requires.
   */
  lgd_type: LgdType;
  /**
   * Macro scenario specifications with probability weights.
   * Weights must sum to 1.0 and each weight must lie in \[0, 1\].
   *
   * When non-empty, [`compute_ecl_weighted`] and [`EclEngine`] require
   * the same ids and weights (tolerance 1e-6, same order) as the
   * `pd_sources` argument. The `pd_sources` weights remain the priced
   * weights and are still validated independently.
   */
  scenarios: MacroScenario[];
  /**
   * Assumed time (in years) from the reporting date to recovery
   * realisation for Stage 3 (credit-impaired) exposures. Used to
   * discount expected recoveries `(1 - LGD) x EAD` at the EIR.
   * The allowance is current EAD less those discounted recoveries.
   * Default: 1.0 year, a common practical recovery-lag assumption.
   */
  stage3_time_to_recovery_years?: number;
  /**
   * Staging configuration for IFRS 9.
   */
  staging: StagingConfig;
  /**
   * Cycle-average LGD used when `lgd_type == `[`LgdType::ThroughTheCycle`].
   */
  ttc_lgd?: number | null;
}
/**
 * Configuration for IFRS 9 stage classification.
 *
 * Controls the quantitative and qualitative thresholds used in the staging
 * waterfall, plus curing rules for step-down from higher stages.
 */
export interface StagingConfig {
  /**
   * Curing configuration: minimum consecutive performing periods
   * required to move from Stage 2 back to Stage 1. Default: 3.
   *
   * The default is a pragmatic 3-period observation window; EBA
   * GL on default uses a 3-month probation for Stage 2 → 1 (see
   * EBA/GL/2016/07 §32).
   */
  cure_periods_stage2_to_1: number;
  /**
   * Curing configuration: minimum consecutive performing periods
   * required to move from Stage 3 to Stage 2. Default: 12.
   *
   * The default matches the EBA GL on default probation window of
   * 12 months for cure out of default (EBA/GL/2016/07 §71–§72). If
   * your internal policy uses a shorter probation, override this
   * field explicitly.
   */
  cure_periods_stage3_to_2: number;
  /**
   * DPD threshold for Stage 2 backstop (IFRS 9 B5.5.19 rebuttable
   * presumption). Default: 30.
   */
  dpd_stage2_threshold: number;
  /**
   * DPD threshold for Stage 3 (absolute backstop). Default: 90.
   */
  dpd_stage3_threshold: number;
  /**
   * Absolute PD increase threshold for SICR (e.g., 0.01 = 1 pp).
   * If the lifetime PD has increased by more than this amount since
   * origination, the exposure is classified as Stage 2.
   */
  pd_delta_absolute: number;
  /**
   * Relative PD increase threshold for SICR (e.g., 2.0 = PD doubled).
   * Applied as: current_pd / origination_pd > threshold.
   */
  pd_delta_relative: number;
  /**
   * Whether any qualitative SICR flag triggers Stage 2.
   */
  qualitative_triggers_enabled: boolean;
  /**
   * Rating downgrade notches that trigger Stage 2 (IFRS 9 B5.5.17(f):
   * external credit rating downgrade as a SICR indicator).
   *
   * [`classify_stage`] fires [`StagingTrigger::RatingDowngrade`] when
   * `position(current) - position(origination) >= rating_downgrade_notches`
   * on the configured scale (see [`Self::rating_scale_labels`]). Example:
   * 3 means a 3-notch downgrade from origination triggers SICR. A `0`
   * threshold disables the trigger entirely.
   */
  rating_downgrade_notches: number;
  /**
   * Ordered rating-scale labels (best -> worst) used to count downgrade
   * notches for the `rating_downgrade_notches` trigger. When `None`, the
   * core 10-state S&P/Fitch scale (AAA, AA, A, BBB, BB, B, CCC, CC, C, D)
   * is used. Ratings not present on the scale never fire the trigger.
   */
  rating_scale_labels?: string[] | null;
  /**
   * Whether objective default-evidence flags (bankruptcy,
   * distressed modification, cross-default,
   * `other_default_evidence`) trigger Stage 3 classification,
   * independently of the 90 DPD backstop. Defaults to `true`.
   *
   * Disable only when such triggers are controlled by an upstream
   * credit-event engine that already mapped them into DPD or
   * previous-stage state.
   */
  stage3_qualitative_triggers_enabled: boolean;
}
/**
 * Inputs for probability-weighted ECL from cumulative-PD schedules.
 */
export interface EclRequest {
  /**
   * Optional ECL integration-bucket width in years; `None` uses the
   * canonical IFRS 9 policy default.
   */
  bucket_width_years?: number | null;
  /**
   * Exposure at default in the exposure's base currency.
   */
  ead: number;
  /**
   * Optional exposure-at-default profile represented as
   * `(time_years, ead)` knots.
   */
  ead_schedule?: [unknown, unknown][] | null;
  /**
   * Annual effective interest rate as a decimal used for discounting.
   */
  eir: number;
  /**
   * Stable identifier copied into the calculation result.
   */
  exposure_id: string;
  /**
   * Loss given default as a decimal probability in `[0, 1]`.
   */
  lgd: number;
  /**
   * Remaining contractual or behavioural maturity in years.
   */
  remaining_maturity_years: number;
  /**
   * Probability-weighted scenarios represented as `(weight, schedule)`;
   * each schedule contains `(time_years, cumulative_pd)` knots.
   */
  scenarios: [unknown, unknown][];
  /**
   * Assigned IFRS 9 stage controlling the 12-month or lifetime horizon.
   */
  stage: Stage;
  /**
   * Optional Stage 3 expected-recovery horizon in years; `None` uses the
   * canonical IFRS 9 ECL default.
   */
  stage3_time_to_recovery_years?: number | null;
}
/**
 * ECL result for a single exposure under a single scenario.
 */
export interface EclResult {
  /**
   * Per-bucket breakdown.
   */
  buckets: EclBucket[];
  /**
   * Total ECL for this exposure under this scenario.
   */
  ecl: number;
  /**
   * Exposure identifier.
   */
  exposure_id: string;
  /**
   * ECL horizon in years.
   */
  horizon: number;
  /**
   * Audit stamp: numeric mode, rounding context, and FX policy in force.
   */
  meta?: ResultsMeta;
  /**
   * Assigned IFRS 9 stage.
   */
  stage: Stage;
  [k: string]: unknown;
}
/**
 * Metadata bundle that accompanies valuation outputs.
 *
 * The metadata is intentionally small so it can be attached to reports and
 * downstream data stores for reproducibility and audit trails.
 *
 * # Examples
 * ```rust
 * use finstack_quant_core::config::{results_meta, FinstackConfig, NUMERIC_MODE_F64};
 *
 * let meta = results_meta(&FinstackConfig::default());
 * assert_eq!(meta.numeric_mode, NUMERIC_MODE_F64);
 * assert!(meta.timestamp.is_none()); // deterministic by default
 * ```
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
 *
 * # Examples
 * ```rust
 * use finstack_quant_core::config::ToleranceConfig;
 *
 * let mut tol = ToleranceConfig::default();
 * assert_eq!(tol.rate_epsilon, 1e-12);
 *
 * // Customize for stricter rate comparisons
 * tol.rate_epsilon = 1e-14;
 * ```
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
 * Inputs for the simplified IFRS 9 stage-classification workflow.
 */
export interface EclStageRequest {
  /**
   * Current lifetime probability of default as a decimal probability.
   */
  current_pd: number;
  /**
   * Current days past due; `None` applies the performing-exposure default
   * of zero days.
   */
  days_past_due?: number | null;
  /**
   * Whether to apply the canonical Stage 2 days-past-due backstop; `None`
   * enables the backstop.
   */
  dpd_stage2_trigger?: boolean | null;
  /**
   * Whether to apply the canonical Stage 3 days-past-due backstop; `None`
   * enables the backstop.
   */
  dpd_stage3_trigger?: boolean | null;
  /**
   * Stable identifier used to label the classified exposure.
   */
  exposure_id: string;
  /**
   * Lifetime probability of default at initial recognition as a decimal
   * probability.
   */
  origination_pd: number;
  /**
   * Optional absolute lifetime-PD increase that triggers Stage 2; `None`
   * uses the default IFRS 9 staging-policy threshold.
   */
  pd_delta_absolute?: number | null;
  /**
   * Remaining contractual or behavioural maturity in years, used as the
   * lifetime-PD comparison horizon subject to the canonical SICR cap.
   */
  remaining_maturity_years: number;
}
/**
 * Verifies that the effective tax rate (tax expense / pretax income) falls within an expected range.
 */
export interface EffectiveTaxRateCheck {
  /**
   * Expected (min, max) for the effective tax rate (decimals, e.g. 0.15..0.40).
   *
   * @minItems 2
   * @maxItems 2
   */
  expected_range: [unknown, unknown];
  /**
   * Pretax income node.
   */
  pretax_income_node: NodeId;
  /**
   * Tax expense node.
   */
  tax_expense_node: NodeId;
  [k: string]: unknown;
}
/**
 * Detailed explanation of a node's calculation.
 */
export interface Explanation {
  /**
   * Breakdown of calculation components
   */
  breakdown: ExplanationStep[];
  /**
   * Final calculated value; non-finite values serialize as `"nan"`,
   * `"inf"` or `"-inf"`.
   */
  final_value: NonFiniteF64Wire;
  /**
   * Formula text (if calculated)
   */
  formula_text?: string | null;
  /**
   * Node identifier
   */
  node_id: string;
  /**
   * Type of node (Value, Calculated, etc.)
   */
  node_type: NodeType;
  /**
   * Period being explained
   */
  period_id: PeriodId;
  [k: string]: unknown;
}
/**
 * Step in a calculation breakdown.
 */
export interface ExplanationStep {
  /**
   * Component identifier (e.g., "revenue")
   */
  component: string;
  /**
   * Operation applied (e.g., "+", "-", "*", "/")
   */
  operation?: string | null;
  /**
   * Value of the component; non-finite values serialize as `"nan"`,
   * `"inf"` or `"-inf"`.
   */
  value: NonFiniteF64Wire;
  [k: string]: unknown;
}
/**
 * A single credit exposure for ECL computation.
 */
export interface Exposure {
  /**
   * Credit-conversion factor applied to [`Self::undrawn`], as a decimal
   * in `[0, 1]`. Default [`DEFAULT_REVOLVER_CCF`] (0.75, Basel IRB
   * revolver). Unused when `undrawn` is zero.
   */
  ccf?: number;
  /**
   * Number of consecutive performing periods since last Stage 2/3
   * classification. Used for curing logic.
   */
  consecutive_performing_periods: number;
  /**
   * Current rating label (must match the `PdTermStructure` scale).
   * `None` if the exposure is unrated.
   */
  current_rating?: string | null;
  /**
   * Current days past due (DPD). Used for backstop staging triggers.
   */
  days_past_due: number;
  /**
   * Outstanding balance (drawn amount) at the reporting date.
   */
  ead: number;
  /**
   * Optional EAD amortization profile as `(time_years, ead)` knots.
   *
   * When present, the ECL engines evaluate EAD at each bucket midpoint by
   * linear interpolation between knots with flat extrapolation at both
   * ends. When `None`, the constant [`Exposure::ead`] is used for every
   * bucket. Knot times must be strictly increasing, finite, and
   * non-negative; EAD values must be finite and non-negative.
   */
  ead_schedule?: [unknown, unknown][] | null;
  /**
   * Effective interest rate (annualized, decimal). Used as the IFRS 9
   * discount rate. Example: 0.05 = 5%.
   */
  eir: number;
  /**
   * Unique identifier for the exposure (instrument ID, facility ID, etc.).
   */
  id: string;
  /**
   * Loss given default (decimal, 0..1). Can be point-in-time or
   * downturn LGD depending on methodology. For credit-impaired exposures,
   * this is the undiscounted fraction lost: recovery at the configured
   * horizon is `(1 - lgd) * ead_at(0)`; the engine discounts recovery once.
   */
  lgd: number;
  /**
   * Rating at origination (initial recognition). Used for SICR delta PD
   * comparison. `None` disables the PD delta trigger.
   */
  origination_rating?: string | null;
  /**
   * Previous reporting period's stage (for migration tracking).
   */
  previous_stage?: Stage | null;
  /**
   * Qualitative flags that can trigger Stage 2 classification.
   */
  qualitative_flags: QualitativeFlags;
  /**
   * Remaining maturity in years from reporting date. For revolving
   * facilities, use behavioural maturity.
   */
  remaining_maturity_years: number;
  /**
   * Segment keys for portfolio aggregation (product type, geography, rating
   * bucket, etc.). Empty vec means "unclassified".
   */
  segments: string[];
  /**
   * Undrawn commitment at the reporting date, in the same currency as
   * [`Self::ead`]. Constant across the horizon (no undrawn schedule).
   * Default `0.0` (fully drawn term loan). EAD is
   * `drawn + undrawn × ccf` via core `ead_revolver`.
   */
  undrawn?: number;
}
/**
 * Qualitative triggers for SICR detection (IFRS 9 B5.5.17) and
 * "unlikely-to-pay" evidence of default (IFRS 9 B5.5.37).
 *
 * These flags represent non-quantitative indicators. SICR flags
 * (`watchlist`, `forbearance`, `adverse_conditions`, `custom`) may
 * trigger a Stage 2 classification. The `default_evidence` flags
 * (`bankruptcy`, `distressed_modification`, `cross_default`,
 * `other_default_evidence`) represent objective evidence of default
 * and should trigger Stage 3 independently of the 90-DPD backstop.
 */
export interface QualitativeFlags {
  /**
   * Significant adverse change in business or financial conditions.
   */
  adverse_conditions: boolean;
  /**
   * Obligor has filed or is subject to bankruptcy / insolvency
   * proceedings. Non-rebuttable Stage 3 trigger.
   */
  bankruptcy: boolean;
  /**
   * Cross-default has been triggered on another obligation of the
   * same obligor.
   */
  cross_default: boolean;
  /**
   * Custom user-defined SICR flags (e.g., sector-specific triggers).
   */
  custom: string[];
  /**
   * Distressed restructuring / modification with material NPV loss
   * to the lender. IFRS 9 B5.5.37(e) / EBA GL on default.
   */
  distressed_modification: boolean;
  /**
   * Forbearance or concession granted. Under IFRS 9 forbearance is
   * at minimum a SICR trigger; distressed modifications (deep
   * concessions such as haircut, maturity extension with NPV loss)
   * are tracked separately under `distressed_modification` and
   * trigger Stage 3.
   */
  forbearance: boolean;
  /**
   * Custom user-defined Stage 3 default-evidence flags.
   */
  other_default_evidence: string[];
  /**
   * On internal watchlist.
   */
  watchlist: boolean;
  [k: string]: unknown;
}
/**
 * Combined staging + ECL result for one exposure.
 */
export interface ExposureEclResult {
  /**
   * Reporting-date EAD of the exposure (copied from [`Exposure::ead`]).
   * Used by portfolio aggregation; defaults to 0.0 when deserializing
   * results produced before this field existed.
   */
  ead?: number;
  /**
   * Probability-weighted ECL result.
   */
  ecl_result: WeightedEclResult;
  /**
   * Audit stamp: numeric mode, rounding context, and FX policy in force.
   */
  meta?: ResultsMeta;
  /**
   * Stage classification result with audit trail.
   */
  stage_result: StageResult;
  [k: string]: unknown;
}
/**
 * Probability-weighted ECL result across scenarios.
 */
export interface WeightedEclResult {
  /**
   * Probability-weighted ECL.
   */
  ecl: number;
  /**
   * Exposure identifier.
   */
  exposure_id: string;
  /**
   * Audit stamp: numeric mode, rounding context, and FX policy in force.
   */
  meta?: ResultsMeta;
  /**
   * Per-scenario breakdown: (scenario_id, weight, result).
   */
  scenario_breakdown: [unknown, unknown, unknown][];
  /**
   * Assigned IFRS 9 stage.
   */
  stage: Stage;
  [k: string]: unknown;
}
/**
 * Result of stage classification with audit trail.
 */
export interface StageResult {
  /**
   * Whether the exposure was cured from a higher stage.
   */
  cured: boolean;
  /**
   * Assigned stage.
   */
  stage: Stage;
  /**
   * Which trigger(s) caused the classification.
   */
  triggers: StagingTrigger[];
  [k: string]: unknown;
}
/**
 * Tracks consecutive periods of negative free cash flow and flags at configurable thresholds.
 */
export interface FcfSignCheck {
  /**
   * Number of consecutive negative periods before an error.
   */
  consecutive_negative_error: number;
  /**
   * Number of consecutive negative periods before a warning.
   */
  consecutive_negative_warning: number;
  /**
   * Free cash flow node.
   */
  fcf_node: NodeId;
  [k: string]: unknown;
}
/**
 * Forecast accuracy metrics.
 */
export interface ForecastMetrics {
  /**
   * Mean Absolute Error: average of |actual - forecast|
   *
   * Interpretation: Average magnitude of errors in the same units as the data.
   * Lower is better. Not sensitive to outliers.
   */
  mae: NonFiniteF64Wire;
  /**
   * Mean Absolute Percentage Error: average of |actual - forecast| / |actual| × 100
   *
   * Interpretation: Average error as a percentage. Scale-independent.
   * Be cautious when actual values are near zero (can produce extreme values).
   * The implementation treats `|actual| < ZERO_TOLERANCE` as a zero-weight
   * term and excludes that sample from both the numerator and the count;
   * [`ForecastMetrics::mape_effective_n`] records how many samples were
   * used so callers can tell a low MAPE from a MAPE that was computed on
   * very few points. Very small non-zero actuals can still dominate the
   * metric. If `mape_effective_n == 0`, `mape` is `NaN`; prefer
   * [`ForecastMetrics::smape`] in that case.
   * Lower is better.
   */
  mape: NonFiniteF64Wire;
  /**
   * Number of samples that actually contributed to MAPE (i.e. had
   * `|actual| >= ZERO_TOLERANCE`). Always `<= n`.
   */
  mape_effective_n: number;
  /**
   * Number of data points used in the calculation
   */
  n: number;
  /**
   * Root Mean Squared Error: sqrt(average((actual - forecast)²))
   *
   * Interpretation: Penalizes larger errors more heavily than MAE.
   * Same units as the data. Always >= MAE.
   * Lower is better.
   */
  rmse: NonFiniteF64Wire;
  /**
   * Symmetric Mean Absolute Percentage Error:
   * `mean( |a - f| / ((|a| + |f|) / 2) ) × 100`.
   *
   * Always well-defined when at least one of `|a|` or `|f|` is positive,
   * and always bounded in `[0, 200]`. Preferred over MAPE when the actual
   * series contains zeros or near-zeros. Terms where both `a` and `f`
   * are within `ZERO_TOLERANCE` of zero are skipped.
   */
  smape: NonFiniteF64Wire;
  [k: string]: unknown;
}
/**
 * Free rent (concession) window that zeros out rent for `periods` starting at `start`.
 */
export interface FreeRentWindowSpec {
  /**
   * Number of model periods of free rent.
   */
  periods: number;
  /**
   * Period (inclusive) when free rent starts.
   */
  start: PeriodId;
}
/**
 * Outcome of a `goal_seek` solve.
 */
export interface GoalSeekResult {
  /**
   * Copy of the input model with `solved_value` written into the driver
   * node at the driver period, re-validated; `None` when `update_model`
   * was `false`.
   */
  model?: statements.FinancialModelSpec | null;
  /**
   * Driver value, in the driver node's own units (the currency amount for
   * a monetary node), that brings the target node to the target value
   * within tolerance.
   */
  solved_value: number;
}
/**
 * Flags line items whose period-over-period growth exceeds configurable upper / lower bounds.
 */
export interface GrowthRateConsistency {
  /**
   * Maximum negative growth rate as a decimal (e.g. −0.30 = −30 %).
   */
  max_decline_pct: number;
  /**
   * Maximum positive growth rate as a decimal (e.g. 0.50 = 50 %).
   */
  max_period_growth_pct: number;
  /**
   * Nodes to monitor for growth-rate plausibility.
   */
  nodes: NodeId[];
  /**
   * Which periods to check.
   */
  scope: PeriodScope;
  [k: string]: unknown;
}
/**
 * Reconciles interest expense against debt balances or the capital-structure interest schedule.
 */
export interface InterestExpenseReconciliation {
  /**
   * Optional capital-structure interest node for direct comparison.
   */
  cs_interest_node?: NodeId | null;
  /**
   * Debt-balance / rate pairs: `(balance_node, optional_rate_node)`.
   */
  debt_balance_nodes: [unknown, unknown][];
  /**
   * Interest expense node (income statement).
   */
  interest_expense_node: NodeId;
  /**
   * Tolerance expressed as a fraction (default 0.05 = 5 %).
   */
  tolerance_pct?: number | null;
  [k: string]: unknown;
}
/**
 * Node-id mappings forwarded to the [`lbo_model_checks`] suite.
 *
 * Supplying this runs the crate's existing LBO check suite against the same
 * evaluated statement results used for the transaction arithmetic, so the
 * report and the returns are guaranteed to describe one evaluation.
 */
export interface LboCheckMappings {
  /**
   * Credit node mapping used for the leverage and coverage checks.
   */
  credit: CreditMapping;
  /**
   * Three-statement node mapping (balance sheet, income statement, cash flow).
   */
  three_statement: ThreeStatementMapping;
}
/**
 * Maps node IDs for a three-statement financial model (income statement,
 * balance sheet, cash flow statement).
 *
 * Required nodes must always be populated. Optional nodes (`Option<NodeId>`)
 * enable additional checks when present; the suite factory will skip the
 * corresponding check when the node is `None`. Unknown JSON keys are
 * rejected, so a misspelled optional key fails instead of silently
 * disabling its check.
 */
export interface ThreeStatementMapping {
  /**
   * Total-assets nodes (balance sheet).
   */
  assets_nodes: NodeId[];
  /**
   * Capital expenditures node (cash flow statement).
   */
  capex_node?: NodeId | null;
  /**
   * Cash / cash-equivalents node (balance sheet).
   */
  cash_node: NodeId;
  /**
   * Cash from financing node (cash flow statement).
   */
  cff_node?: NodeId | null;
  /**
   * Cash from investing node (cash flow statement).
   */
  cfi_node?: NodeId | null;
  /**
   * Cash from operations node (cash flow statement).
   */
  cfo_node?: NodeId | null;
  /**
   * Capital-structure interest node; when present the interest
   * reconciliation compares against it directly instead of testing an
   * implied rate.
   */
  cs_interest_node?: NodeId | null;
  /**
   * Current-asset nodes summed into net working capital.
   */
  current_assets_nodes?: NodeId[];
  /**
   * Current-liability nodes summed into net working capital.
   */
  current_liabilities_nodes?: NodeId[];
  /**
   * Debt-balance / optional-rate pairs used to test the implied interest
   * rate against `interest_expense_node`.
   */
  debt_balance_nodes?: [unknown, unknown][];
  /**
   * Depreciation / amortization expense node (income statement).
   */
  depreciation_node?: NodeId | null;
  /**
   * Dividends charged against equity (balance sheet / equity roll-forward);
   * enables the dividend reconciliation together with `dividends_node`.
   */
  dividends_equity_node?: NodeId | null;
  /**
   * Dividends paid node (cash flow statement / financing).
   */
  dividends_node?: NodeId | null;
  /**
   * Total-equity nodes (balance sheet).
   */
  equity_nodes: NodeId[];
  /**
   * Intangible-asset additions node; enables the capex reconciliation
   * together with `capex_node`.
   */
  intangible_additions_node?: NodeId | null;
  /**
   * Interest expense node (income statement).
   */
  interest_expense_node?: NodeId | null;
  /**
   * Total-liabilities nodes (balance sheet).
   */
  liabilities_nodes: NodeId[];
  /**
   * Net income node (income statement).
   */
  net_income_node: NodeId;
  /**
   * PP&E additions node; enables the capex reconciliation together with
   * `capex_node`.
   */
  ppe_additions_node?: NodeId | null;
  /**
   * PP&E balance node (balance sheet).
   */
  ppe_node?: NodeId | null;
  /**
   * Pre-tax income node (income statement).
   */
  pretax_income_node?: NodeId | null;
  /**
   * Retained earnings node (balance sheet).
   */
  retained_earnings_node: NodeId;
  /**
   * Income tax expense node (income statement).
   */
  tax_expense_node?: NodeId | null;
  /**
   * Total cash flow node (cash flow statement).
   */
  total_cf_node?: NodeId | null;
  /**
   * Change-in-working-capital node (cash flow statement, operating).
   */
  wc_change_cf_node?: NodeId | null;
}
/**
 * Transaction assumptions for an LBO evaluation.
 */
export interface LboConfig {
  /**
   * Optional mappings enabling the [`lbo_model_checks`] suite.
   */
  check_mappings?: LboCheckMappings | null;
  /**
   * Node id supplying the entry valuation metric (typically `"ebitda"`),
   * read at the model's first period.
   */
  entry_metric_node: string;
  /**
   * Entry valuation multiple applied to `entry_metric_node`, e.g. `8.5`
   * for 8.5x EBITDA.
   */
  entry_multiple: number;
  /**
   * Node id supplying the exit valuation metric, read at `exit_period`.
   */
  exit_metric_node: string;
  /**
   * Exit valuation multiple applied to `exit_metric_node`.
   */
  exit_multiple: number;
  /**
   * Node id supplying net debt outstanding at exit, read at `exit_period`.
   * This is where a modelled amortisation schedule lands.
   */
  exit_net_debt_node: string;
  /**
   * Period at which the sponsor exits. Periods are half-open `[start, end)`.
   */
  exit_period: PeriodId;
  /**
   * Funded debt tranches at close. The sponsor equity check is solved as
   * the residual that balances sources against uses.
   */
  sources: LboTranche[];
  /**
   * Transaction fees and expenses funded at close, in the model currency.
   */
  transaction_fees: number;
}
/**
 * One funded debt tranche in the day-one capital structure.
 *
 * Amounts are in the model currency and are the amounts drawn at close, not
 * commitments. Tranche paydown over the hold period is modelled inside the
 * statement model (see the module-level note on roll-forwards).
 */
export interface LboTranche {
  /**
   * Amount funded at close, in the model currency. Must be finite and
   * non-negative.
   */
  amount: number;
  /**
   * Tranche label, e.g. `"term_loan_a"` or `"mezzanine"`.
   */
  name: string;
}
/**
 * Outputs of an LBO evaluation.
 */
export interface LboResult {
  /**
   * Report from the [`lbo_model_checks`] suite when
   * [`LboConfig::check_mappings`] was supplied.
   */
  checks?: CheckReport | null;
  /**
   * Total funded debt at close (sum of [`LboConfig::sources`]).
   */
  debt_total: Money;
  /**
   * Entry enterprise value: `entry_multiple × entry metric`.
   */
  entry_enterprise_value: Money;
  /**
   * Entry metric value read from the model at the first period.
   */
  entry_metric: number;
  /**
   * Sponsor equity required to close: `uses_total − debt_total`.
   */
  equity_check: Money;
  /**
   * Exit enterprise value: `exit_multiple × exit metric`.
   */
  exit_enterprise_value: Money;
  /**
   * Equity proceeds at exit: `exit_enterprise_value − exit_net_debt`.
   */
  exit_equity_proceeds: Money;
  /**
   * Exit metric value read from the model at `exit_period`.
   */
  exit_metric: number;
  /**
   * Net debt outstanding at `exit_period`.
   */
  exit_net_debt: Money;
  /**
   * Multiple of invested capital: `exit_equity_proceeds / equity_check`.
   */
  moic: number;
  /**
   * Total sources: funded debt plus the sponsor equity check.
   */
  sources_total: Money;
  /**
   * `true` when sources reconcile to uses within a relative tolerance of
   * `1e-9` scaled by deal size. False signals a
   * numerically degenerate capital structure (for example, tranche amounts
   * so large relative to the purchase price that the residual equity check
   * loses all significance).
   */
  sources_uses_balanced: boolean;
  /**
   * Total uses: entry enterprise value plus transaction fees.
   */
  uses_total: Money;
  [k: string]: unknown;
}
/**
 * Richer lease spec for rent roll generation: - rent steps (explicit bumps) - arbitrary free-rent windows - optional renewal with downtime + probability
 */
export interface LeaseSpec {
  /**
   * Base rent per period at `start`.
   */
  base_rent: number;
  /**
   * Last period (inclusive) of the initial term. `None` means through model end (no renewal modeling).
   */
  end?: PeriodId | null;
  /**
   * Number of free rent periods from `start`.
   */
  free_rent_periods?: number;
  /**
   * Additional free rent windows (beyond the initial `free_rent_periods`).
   */
  free_rent_windows?: FreeRentWindowSpec[];
  /**
   * Convention for compounding `growth_rate`.
   *
   * Defaults to [`LeaseGrowthConvention::AnnualEscalator`]. Set
   * [`LeaseGrowthConvention::PerPeriod`] explicitly for per-period
   * compounding.
   */
  growth_convention?: LeaseGrowthConvention;
  /**
   * Growth rate applied per period or annually (depending on `growth_convention`)
   * within a rent segment (between steps).
   */
  growth_rate?: number;
  /**
   * Base id used to derive node ids:
   * `{node_id}.pgi`, `{node_id}.free_rent`, `{node_id}.vacancy_loss`, `{node_id}.effective_rent`.
   */
  node_id: string;
  /**
   * Occupancy factor in `0..=1` applied to non-free contractual rent.
   */
  occupancy?: number;
  /**
   * Optional renewal modeling after `end`.
   */
  renewal?: RenewalSpec | null;
  /**
   * Rent steps that reset rent levels at their start periods.
   */
  rent_steps?: RentStepSpec[];
  /**
   * First period (inclusive) when the lease is active.
   */
  start: PeriodId;
}
/**
 * Renewal specification for a lease.
 *
 * This is modeled in an **expected value** sense via `probability`.
 */
export interface RenewalSpec {
  /**
   * Downtime (no rent) after the initial term ends.
   */
  downtime_periods?: number;
  /**
   * Free rent periods at renewal start.
   */
  free_rent_periods?: number;
  /**
   * Probability of renewal in `0..=1`.
   */
  probability: number;
  /**
   * Rent multiplier applied to the last contractual rent of the initial term.
   *
   * Example: `1.05` means renewal starts at +5% vs prior rent level.
   */
  rent_factor?: number;
  /**
   * Renewal term length in model periods.
   */
  term_periods: number;
}
/**
 * Rent step that resets the base rent starting at `start` (inclusive).
 *
 * The lease `growth_rate` then applies from this step forward until the next step.
 */
export interface RentStepSpec {
  /**
   * Rent per model period starting at `start`.
   */
  rent: number;
  /**
   * Period (inclusive) when this rent level becomes effective.
   */
  start: PeriodId;
}
/**
 * Flags periods where Debt / TTM EBITDA falls outside configurable warning and error ranges.
 */
export interface LeverageRangeCheck {
  /**
   * Total debt node.
   */
  debt_node: NodeId;
  /**
   * EBITDA node.
   */
  ebitda_node: NodeId;
  /**
   * `(min, max)` range that triggers an error when exceeded.
   *
   * @minItems 2
   * @maxItems 2
   */
  error_range: [unknown, unknown];
  /**
   * `(min, max)` range that triggers a warning when exceeded.
   *
   * @minItems 2
   * @maxItems 2
   */
  warn_range: [unknown, unknown];
  [k: string]: unknown;
}
/**
 * Estimates the liquidity runway in months and flags periods that fall below configurable warning and error thresholds.
 */
export interface LiquidityRunwayCheck {
  /**
   * Cash burn rate node (positive = burning cash).
   */
  cash_burn_node: NodeId;
  /**
   * Cash balance node.
   */
  cash_node: NodeId;
  /**
   * Minimum runway (in months) before an error.
   */
  min_months_error: number;
  /**
   * Minimum runway (in months) before a warning.
   */
  min_months_warning: number;
  [k: string]: unknown;
}
/**
 * Management fee specification.
 */
export interface ManagementFeeSpec {
  /**
   * Fee base for calculation.
   */
  base?: ManagementFeeBase;
  /**
   * Management fee rate as a decimal fraction (e.g., 0.03 for 3%).
   */
  rate: number;
}
/**
 * Parameter to vary in sensitivity analysis.
 */
export interface ParameterSpec {
  /**
   * Base value
   */
  base_value: number;
  /**
   * Node identifier
   */
  node_id: string;
  /**
   * Period to vary
   */
  period_id: PeriodId;
  /**
   * Perturbations to apply (e.g., [-10%, 0%, +10%])
   */
  perturbations: number[];
  [k: string]: unknown;
}
/**
 * Criteria for filtering companies into a peer set.
 */
export interface PeerFilter {
  /**
   * ISO country codes to include (meta key: "country").
   */
  countries: string[];
  /**
   * Excluded attribute tags (none may be present).
   */
  excluded_tags: string[];
  /**
   * GICS industry codes to include (meta key: "gics_industry").
   */
  gics_industries: string[];
  /**
   * GICS sector codes to include (meta key: "gics_sector").
   */
  gics_sectors: string[];
  /**
   * Market cap ceiling (inclusive).
   */
  market_cap_max?: number | null;
  /**
   * Market cap floor (inclusive).
   */
  market_cap_min?: number | null;
  /**
   * Credit rating bands to include (meta key: "rating").
   */
  ratings: string[];
  /**
   * Required attribute tags (all must be present).
   */
  required_tags: string[];
  /**
   * Arbitrary attribute selector strings (uses `Attributes::matches_selector`).
   */
  selectors: string[];
  [k: string]: unknown;
}
/**
 * A set of comparable companies with their metrics.
 */
export interface PeerSet {
  /**
   * Peer companies in the comparison set.
   */
  peers: CompanyMetrics[];
  /**
   * Period basis for all metrics in this set.
   */
  period_basis: PeriodBasis;
  /**
   * The subject company being evaluated.
   */
  subject: CompanyMetrics;
  [k: string]: unknown;
}
/**
 * Descriptive statistics for a peer set metric.
 */
export interface PeerStats {
  /**
   * Number of observations.
   */
  count: number;
  /**
   * Interquartile range (`q3 - q1`).
   */
  iqr: number;
  /**
   * Maximum value.
   */
  max: number;
  /**
   * Arithmetic mean.
   */
  mean: number;
  /**
   * Median (50th percentile).
   */
  median: number;
  /**
   * Minimum value.
   */
  min: number;
  /**
   * 25th percentile.
   */
  q1: number;
  /**
   * 75th percentile.
   */
  q3: number;
  /**
   * Sample standard deviation.
   */
  std_dev: number;
  [k: string]: unknown;
}
/**
 * Portfolio-level ECL result with stage migration and segment breakdown.
 */
export interface PortfolioEclResult {
  /**
   * Exposure count by stage.
   */
  count_by_stage: {
    [k: string]: number;
  };
  /**
   * EAD by stage (total outstanding balance per stage).
   */
  ead_by_stage: {
    [k: string]: number;
  };
  /**
   * ECL by segment (product, geography, rating, etc.).
   */
  ecl_by_segment: {
    [k: string]: number;
  };
  /**
   * ECL by stage.
   */
  ecl_by_stage: {
    [k: string]: number;
  };
  /**
   * Per-exposure results.
   */
  exposure_results: ExposureEclResult[];
  /**
   * Stage migration matrix: previous_stage -> current_stage -> count.
   * Only populated for exposures that have a `previous_stage`.
   */
  migration_matrix: {
    [k: string]: {
      [k: string]: number;
    };
  };
  /**
   * Total ECL across all exposures.
   */
  total_ecl: number;
  /**
   * Exposure ids in `exposure_results` that could not be matched against
   * the exposure slice passed to
   * [`PortfolioEclResult::from_results_with_exposures`]. Unmatched
   * results still contribute their ECL and EAD to the stage totals, but
   * carry no segment or migration data.
   */
  unmatched_exposure_ids?: string[];
  [k: string]: unknown;
}
/**
 * Standard node ids for a full property operating statement template.
 */
export interface PropertyTemplateNodes {
  /**
   * Total CapEx node id.
   */
  capex_total_node?: string;
  /**
   * Effective gross income (EGI) node id.
   */
  egi_node?: string;
  /**
   * Management fee node id (if configured).
   */
  management_fee_node?: string;
  /**
   * Net cash flow (NCF) node id: `noi - capex_total`.
   */
  ncf_node?: string;
  /**
   * Net operating income (NOI) node id.
   */
  noi_node?: string;
  /**
   * Total operating expenses node id (includes management fee if enabled).
   */
  opex_total_node?: string;
  /**
   * Total other income node id.
   */
  other_income_total_node?: string;
  /**
   * Rent roll output nodes (PGI/free rent/vacancy/effective rent).
   */
  rent_roll?: RentRollOutputNodes;
}
/**
 * Standard output node ids for a rent roll.
 *
 * The richer rent-roll helper also emits fixed transparency nodes
 * `vacancy_loss_physical` and `renewal_prob_loss` in addition to these
 * configurable aggregate outputs.
 */
export interface RentRollOutputNodes {
  /**
   * Total free rent concessions.
   */
  free_rent_node: string;
  /**
   * Total effective rent (EGI rent component): `rent_pgi - free_rent - vacancy_loss`.
   */
  rent_effective_node: string;
  /**
   * Total contractual rent (PGI) from all leases.
   */
  rent_pgi_node: string;
  /**
   * Total vacancy loss (includes occupancy and renewal probability effects).
   */
  vacancy_loss_node: string;
}
/**
 * Provision movement waterfall between two reporting dates.
 */
export interface ProvisionWaterfall {
  /**
   * Closing ECL balance.
   */
  closing: number;
  /**
   * Cured back to Stage 1 (net ECL release) — both Stage 2 → 1 and
   * Stage 3 → 1.
   */
  cured_to_stage1: number;
  /**
   * Partial cure Stage 3 → Stage 2 (net ECL change, typically a
   * release as Stage 3 lifetime ECL is de-recognised, partly offset
   * by Stage 2 lifetime ECL).
   */
  cured_to_stage2: number;
  /**
   * Derecognitions (maturities, repayments, sales).
   */
  derecognitions: number;
  /**
   * New originations (new exposures added this period).
   */
  new_originations: number;
  /**
   * Opening ECL balance (previous period closing).
   */
  opening: number;
  /**
   * Model/parameter changes (remeasurement, i.e., the residual).
   */
  remeasurement: number;
  /**
   * Transfers to Stage 2 (net ECL change from stage upgrades).
   */
  transfers_to_stage2: number;
  /**
   * Transfers to Stage 3 (net ECL change from stage downgrades).
   */
  transfers_to_stage3: number;
  [k: string]: unknown;
}
/**
 * Rating-keyed map of cumulative PD curves.
 */
export interface RatingPdMap {
  /**
   * Cumulative PD curves keyed by the rating label
   * [`PdTermStructure::cumulative_pd`] looks up. Lookup uses the map
   * key; the curve's own rating label need not match the key.
   */
  curves: {
    [k: string]: RawPdCurve;
  };
}
/**
 * Raw user-supplied PD term structure with linear interpolation.
 *
 * Use this when you have a discrete set of cumulative PD observations
 * (e.g., from internal rating model output) rather than a parametric
 * hazard curve or transition matrix.
 *
 * # Interpolation
 *
 * - Knots must be sorted by time and monotonically increasing in PD.
 * - For `t` before the first knot, cumulative PD is 0.
 * - For `t` after the last knot, cumulative PD is flat-extrapolated.
 * - Between knots, linear interpolation is applied.
 *
 * # Deserialization
 *
 * Deserialization is validating: JSON inputs go through the same invariant
 * checks as [`RawPdCurve::new`] (first knot at `(0.0, 0.0)`, strictly
 * increasing times, monotone PDs in `[0, 1]`) and unknown fields are
 * rejected.
 */
export interface RawPdCurve {
  knots: [unknown, unknown][];
  rating: string;
}
/**
 * OLS regression result for fair-value estimation.
 */
export interface RegressionResult {
  /**
   * Fitted (fair) value for the subject.
   */
  fitted_value: number;
  /**
   * Intercept (alpha).
   */
  intercept: number;
  /**
   * Number of observations used.
   */
  n: number;
  /**
   * R-squared goodness of fit.
   */
  r_squared: number;
  /**
   * Residual: actual - fitted. Positive = cheap, negative = rich.
   */
  residual: number;
  /**
   * Slope coefficient (beta).
   */
  slope: number;
  [k: string]: unknown;
}
/**
 * Composite relative value result.
 */
export interface RelativeValueResult {
  /**
   * Company being scored.
   */
  company_id: CompanyId;
  /**
   * Composite rich/cheap score.
   *
   * Positive = cheap (trading below fair value across dimensions).
   * Negative = rich (trading above fair value).
   * Magnitude indicates conviction (bounded by number of dimensions
   * and their weights).
   */
  composite_score: number;
  /**
   * Confidence in the composite score (average R-squared across
   * regression-based dimensions, weighted by dimension weight).
   */
  confidence: number;
  /**
   * Per-dimension decomposition.
   */
  dimensions: DimensionScore[];
  /**
   * Number of peers used in the analysis.
   */
  peer_count: number;
  [k: string]: unknown;
}
/**
 * Definition for a single named scenario.
 *
 * Scenarios are attached to a base [`FinancialModelSpec`] and specify an
 * optional parent plus typed scalar or monetary overrides for named nodes.
 *
 * The JSON representation mirrors the design docs example:
 *
 * ```json
 * "downside": {
 *   "parent": "base",
 *   "overrides": { "revenue_growth": -0.05, "margin": -0.02 }
 * }
 * ```
 */
export interface ScenarioDefinition {
  /**
   * Typed overrides for model nodes.
   *
   * Overrides apply to forecast periods only and must match the target
   * node's scalar or monetary type and currency.
   */
  overrides?: {
    [k: string]: AmountOrScalar;
  };
  /**
   * Optional parent scenario to inherit overrides from.
   *
   * Parent chains can be arbitrarily deep but must be acyclic. Later
   * scenarios in the chain override earlier ones for the same `node_id`.
   */
  parent?: string | null;
  /**
   * Typed per-period overrides for model nodes, keyed `node_id` then
   * period id.
   *
   * A per-period override applies only to the named forecast period and
   * takes precedence over a model-wide `overrides` entry for the same node
   * in that period. Across a parent chain the child's per-period value wins
   * for that (node, period), but a child's model-wide override does not
   * clear a parent's per-period overrides for other periods.
   */
  period_overrides?: {
    [k: string]: {
      [k: string]: AmountOrScalar;
    };
  };
}
/**
 * Variance-style diff between two evaluated scenarios.
 */
export interface ScenarioDiff {
  /**
   * Baseline scenario name.
   */
  baseline: string;
  /**
   * Comparison scenario name.
   */
  comparison: string;
  /**
   * Underlying variance report.
   */
  variance: VarianceReport;
  [k: string]: unknown;
}
/**
 * Full variance report between a baseline and comparison.
 */
export interface VarianceReport {
  /**
   * Label for the baseline scenario (e.g. `"management_case"`).
   */
  baseline_label: string;
  /**
   * Label for the comparison scenario (e.g. `"bank_case"`).
   */
  comparison_label: string;
  /**
   * Per-metric, per-period variance rows.
   */
  rows: VarianceRow[];
  [k: string]: unknown;
}
/**
 * One row of variance output.
 *
 * Represents variance for a single `(metric, period)` pair.
 */
export interface VarianceRow {
  /**
   * Absolute variance: `comparison - baseline`.
   */
  abs_var: number;
  /**
   * Baseline value.
   */
  baseline: number;
  /**
   * Comparison value.
   */
  comparison: number;
  /**
   * Optional driver breakdown: `driver → contribution` in the same units
   * as the underlying metric.
   *
   * This is populated by bridge decomposition helpers and can be rendered
   * as a "bridge" column in reports.
   */
  driver_contribution?: {
    [k: string]: number;
  };
  /**
   * Metric / node identifier (e.g. `"revenue"`).
   */
  metric: string;
  /**
   * Percentage variance (fraction): `abs_var / baseline`.
   *
   * When the baseline is effectively zero this is `None` (Options
   * serialize as JSON `null`) rather than a spurious `0.0`, which
   * previously hid meaningful absolute deltas whenever the
   * denominator was small. Reporting layers should interpret a
   * `None` as "undefined / not meaningful" and fall back to the
   * absolute variance.
   */
  pct_var?: number | null;
  /**
   * Period identifier (e.g. `"2025Q1"`).
   */
  period: PeriodId;
  [k: string]: unknown;
}
/**
 * Evaluated results for all scenarios in a `ScenarioSet`.
 */
export interface ScenarioResults {
  [k: string]: statements.StatementResult;
}
/**
 * Registry of named scenarios built on top of a base model.
 */
export interface ScenarioSet {
  /**
   * Map of scenario name → definition.
   */
  scenarios: {
    [k: string]: ScenarioDefinition;
  };
}
/**
 * Configuration for credit scorecard analysis.
 */
export interface ScorecardConfig {
  /**
   * List of metrics to evaluate
   */
  metrics?: ScorecardMetric[];
  /**
   * Minimum acceptable rating (optional)
   */
  min_rating?: string | null;
  /**
   * Period to rate, as a parseable `PeriodId` string (e.g. `"2025Q4"`).
   *
   * When `None`, the scorecard rates the last *actual* period in the model
   * if any exists, otherwise the last model period. Metric formulas see
   * only periods up to (and including) the rated period as history.
   */
  period?: string | null;
  /**
   * Rating scale to use (e.g., "S&P", "Moody's", "Fitch")
   */
  rating_scale?: string;
}
/**
 * Definition of a scorecard metric.
 */
export interface ScorecardMetric {
  /**
   * Description
   */
  description?: string | null;
  /**
   * Formula to calculate the metric (DSL syntax)
   */
  formula: string;
  /**
   * Metric name
   */
  name: string;
  /**
   * Rating thresholds: rating → [min, max]
   */
  thresholds?: {
    /**
     * @minItems 2
     * @maxItems 2
     */
    [k: string]: [unknown, unknown];
  };
  /**
   * Weight in overall score (0.0 to 1.0)
   */
  weight?: number;
}
/**
 * Report produced by `CreditScorecardExtension::execute`.
 */
export interface ScorecardReport {
  /**
   * Structured output (rating, total_score, metric_scores, rating_scale).
   * With no usable positive-weight evidence, rating and total_score are
   * null, partial is true, and status is Failed.
   */
  data?: {
    [k: string]: unknown;
  };
  /**
   * Errors (per-metric failures)
   */
  errors?: string[];
  /**
   * Human-readable summary
   */
  message: string;
  /**
   * Overall execution status
   */
  status: ScorecardStatus;
  /**
   * Warnings (non-fatal)
   */
  warnings?: string[];
  [k: string]: unknown;
}
/**
 * Configuration for a single rich/cheap scoring dimension.
 */
export interface ScoringDimension {
  /**
   * Rich/cheap sign convention for the Y metric (default:
   * [`ScoreDirection::HigherIsCheap`], i.e. spread-like).
   */
  direction?: ScoreDirection;
  /**
   * Human-readable label (e.g., "Spread vs Leverage").
   */
  label: string;
  /**
   * Weight of this dimension in the composite score (0.0 to 1.0).
   */
  weight: number;
  /**
   * Optional explanatory metric for single-factor regression.
   * Omit it to score the dependent metric against its peer distribution.
   */
  x_extractor?: MetricExtractor | null;
  /**
   * How to extract the Y variable (dependent) from CompanyMetrics.
   */
  y_extractor: MetricExtractor;
}
/**
 * Sensitivity analysis configuration.
 */
export interface SensitivityConfig {
  /**
   * Analysis mode
   */
  mode: SensitivityMode;
  /**
   * Parameters to vary
   */
  parameters: ParameterSpec[];
  /**
   * Target metrics to track
   */
  target_metrics: string[];
}
/**
 * Results of sensitivity analysis.
 */
export interface SensitivityResult {
  /**
   * Unperturbed baseline evaluation of the model (populated by tornado
   * runs). Used by tornado chart generation as the reference metric
   * when a parameter's base value is not part of the perturbation grid.
   */
  baseline?: statements.StatementResult | null;
  /**
   * Configuration used
   */
  config: SensitivityConfig;
  /**
   * All scenario results
   */
  scenarios: SensitivityScenario[];
  [k: string]: unknown;
}
/**
 * Result of a single sensitivity scenario.
 */
export interface SensitivityScenario {
  /**
   * Parameter values for this scenario keyed as `node_id@period_id`.
   */
  parameter_values: {
    [k: string]: number;
  };
  /**
   * Full evaluation results
   */
  results: statements.StatementResult;
  [k: string]: unknown;
}
/**
 * Flags a metric that has been deteriorating for `lookback_periods` consecutive periods.
 */
export interface TrendCheck {
  /**
   * Which direction is "good".
   */
  direction: TrendDirection;
  /**
   * Number of consecutive deteriorating periods before flagging.
   */
  lookback_periods: number;
  /**
   * Node to monitor.
   */
  node: NodeId;
  /**
   * Severity to assign to the finding.
   */
  severity: CheckSeverity;
  [k: string]: unknown;
}
/**
 * Configuration for variance analysis between two `StatementResult`.
 */
export interface VarianceConfig {
  /**
   * Human-readable name for the baseline (e.g. `"management_case"`).
   */
  baseline_label: string;
  /**
   * Human-readable name for the comparison (e.g. `"bank_case"`).
   */
  comparison_label: string;
  /**
   * Node identifiers to compare (e.g. `["revenue", "ebitda"]`).
   */
  metrics: string[];
  /**
   * Periods to include in the variance report.
   */
  periods: PeriodId[];
}
/**
 * Verifies that the change in working capital on the cash flow statement equals the negative delta of net working capital on the balance sheet.
 */
export interface WorkingCapitalConsistency {
  /**
   * Current-asset nodes from the balance sheet.
   */
  current_assets_nodes: NodeId[];
  /**
   * Current-liability nodes from the balance sheet.
   */
  current_liabilities_nodes: NodeId[];
  /**
   * Tolerance override; falls back to
   * [`finstack_quant_statements::checks::CheckConfig::default_tolerance`].
   */
  tolerance?: number | null;
  /**
   * Working-capital change node from the cash flow statement.
   */
  wc_change_cf_node: NodeId;
  [k: string]: unknown;
}
