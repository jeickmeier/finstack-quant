// Generated from the finstack-quant-covenants JSON schemas by scripts/generate-contract-types.mjs. Do not edit.

/**
 * Direction of inequality for numeric covenants.
 */
export type BoundKind = "at_most" | "at_least";
/**
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
/**
 * Consequence of covenant breach
 */
export type CovenantConsequence =
  | "default"
  | {
      rate_increase: {
        /**
         * Increase in basis points
         */
        bp_increase: number;
      };
    }
  | {
      cash_sweep: {
        /**
         * Percentage of cash flow to sweep
         */
        sweep_percentage: number;
      };
    }
  | "block_distributions"
  | {
      require_collateral: {
        /**
         * Description of collateral requirement
         */
        description: string;
      };
    }
  | {
      accelerate_maturity: {
        /**
         * New accelerated maturity date
         */
        new_maturity: DateWire;
      };
    };
/**
 * Type of financial or operational covenant
 */
export type CovenantType =
  | {
      max_debt_to_ebitda: {
        /**
         * Maximum allowed ratio
         */
        threshold: number;
      };
    }
  | {
      min_interest_coverage: {
        /**
         * Minimum required ratio
         */
        threshold: number;
      };
    }
  | {
      min_fixed_charge_coverage: {
        /**
         * Minimum required coverage
         */
        threshold: number;
      };
    }
  | {
      max_total_leverage: {
        /**
         * Maximum allowed leverage
         */
        threshold: number;
      };
    }
  | {
      max_senior_leverage: {
        /**
         * Maximum allowed senior leverage
         */
        threshold: number;
      };
    }
  | {
      min_asset_coverage: {
        /**
         * Minimum required coverage
         */
        threshold: number;
      };
    }
  | {
      negative: {
        /**
         * Description of restriction
         */
        restriction: string;
      };
    }
  | {
      affirmative: {
        /**
         * Description of requirement
         */
        requirement: string;
      };
    }
  | {
      custom: {
        /**
         * Name of metric to test
         */
        metric: string;
        /**
         * Threshold test (min or max)
         */
        test: ThresholdTest;
      };
    }
  | {
      basket: {
        /**
         * Maximum allowed utilization of the basket
         */
        limit: number;
        /**
         * Basket identifier/metric name
         */
        name: string;
      };
    }
  | {
      min_dscr: {
        /**
         * Minimum required coverage
         */
        threshold: number;
      };
    }
  | {
      max_net_debt_to_ebitda: {
        /**
         * Maximum allowed ratio
         */
        threshold: number;
      };
    }
  | {
      max_capex: {
        /**
         * Maximum allowed capex amount
         */
        threshold: number;
      };
    }
  | {
      min_liquidity: {
        /**
         * Minimum required liquidity
         */
        threshold: number;
      };
    };
/**
 * Threshold test type (maximum or minimum bound)
 */
export type ThresholdTest =
  | {
      maximum: number;
    }
  | {
      minimum: number;
    };
/**
 * Whether a covenant is tested periodically or only upon an action.
 */
export type CovenantScope = "maintenance" | "incurrence";
/**
 * String-backed identifier for a covenant operating metric.
 *
 * These identifiers are conventionally aligned with `finstack-quant-statements`
 * node IDs such as `debt_to_ebitda`, `ebitda`, `interest_coverage`, and
 * `dscr`, but this crate intentionally has no compile-time dependency on
 * `finstack-quant-statements`.
 */
export type CovenantMetricId = string;
/**
 * Unit of a tenor period.
 */
export type TenorUnit = "days" | "weeks" | "months" | "years";
/**
 * Piecewise-constant threshold schedule for covenants.
 *
 * Entries are stored sorted by date ascending. The effective threshold for a
 * test date is the last entry with date <= test_date. If no entry applies,
 * [`ThresholdSchedule::threshold_for`] returns `None`.
 */
export type ThresholdSchedule = [unknown, unknown][];
/**
 * Rounding modes supported by the library.
 *
 * The variants mirror the most common conventions found in pricing engines.
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
 * Result of applying a covenant consequence.
 */
export interface ConsequenceApplication {
  /**
   * Date when applied
   */
  applied_date: DateWire;
  /**
   * Type of consequence applied
   */
  consequence_type: string;
  /**
   * Details about the application
   */
  details: string;
  [k: string]: unknown;
}
/**
 * Financial covenant specification with test frequency and consequences.
 */
export interface Covenant {
  /**
   * Actions taken if covenant is breached
   */
  consequences: CovenantConsequence[];
  /**
   * Type of covenant (leverage, coverage, etc.)
   */
  covenant_type: CovenantType;
  /**
   * Optional cure period in days before default
   */
  cure_period_days?: number | null;
  /**
   * Whether the covenant is currently active
   */
  is_active: boolean;
  /**
   * Optional instance label disambiguating covenants of the same type.
   *
   * [`CovenantType::covenant_id`] is discriminant-only, so two covenants of
   * the same type (e.g. a senior and a total leverage test, or two baskets)
   * would collide in compliance reports and breach tracking without a
   * distinct label. Waivers and breaches key off it.
   */
  label: string;
  /**
   * Whether the covenant is maintenance or incurrence.
   */
  scope: CovenantScope;
  /**
   * Optional activation condition for springing covenants.
   */
  springing_condition?: SpringingCondition | null;
  /**
   * How frequently the covenant is tested.
   *
   * Descriptive metadata only: the engine does **not** enforce this
   * schedule. Callers control test dates by choosing when to invoke
   * [`super::CovenantEngine::evaluate`].
   */
  test_frequency: Tenor;
}
/**
 * Optional activation condition for springing covenants.
 */
export interface SpringingCondition {
  /**
   * Metric that controls activation (e.g., revolver utilization).
   */
  metric_id: CovenantMetricId;
  /**
   * Threshold test applied to the metric.
   */
  test: ThresholdTest;
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
 * Covenant breach tracking.
 */
export interface CovenantBreach {
  /**
   * Actual value that caused the breach
   */
  actual_value?: number | null;
  /**
   * Successfully applied prefix of `consequences`; retries resume after it.
   */
  applied_consequences: CovenantConsequence[];
  /**
   * Date of the breach
   */
  breach_date: DateWire;
  /**
   * Consequences captured from the effective specification at the breach date.
   * This immutable execution order survives later amendments and window changes.
   */
  consequences: CovenantConsequence[];
  /**
   * Stable identifier matching [`Covenant::instance_key`].
   */
  covenant_id?: string;
  /**
   * Human-readable description (from `Display`).
   */
  covenant_type: string;
  /**
   * Cure period end date (if applicable)
   */
  cure_deadline?: DateWire | null;
  /**
   * Whether the breach has been cured
   */
  is_cured: boolean;
  /**
   * Required threshold
   */
  threshold?: number | null;
}
/**
 * Covenant engine for evaluation and consequence application.
 */
export interface CovenantEngine {
  /**
   * Historical breaches
   */
  breach_history?: CovenantBreach[];
  /**
   * Active covenant specifications
   */
  specs: CovenantSpec[];
  /**
   * Active waivers and amendments
   */
  waivers?: CovenantWaiver[];
  /**
   * Covenant testing windows
   */
  windows?: CovenantWindow[];
}
/**
 * Covenant evaluation specification.
 */
export interface CovenantSpec {
  /**
   * The covenant to evaluate
   */
  covenant: Covenant;
  /**
   * Earnings denominator used to validate a leverage ratio. Required for net
   * debt/EBITDA, where a negative ratio can mean net cash or negative earnings.
   * Values must be finite and in the same reporting-period convention as the
   * ratio. A non-positive denominator makes the test a breach with no headroom.
   */
  denominator_metric_id?: CovenantMetricId | null;
  /**
   * Metric ID to use for evaluation (for financial covenants)
   */
  metric_id?: CovenantMetricId | null;
  /**
   * Time-varying threshold schedule that overrides the static threshold in
   * [`CovenantType`] when present. Enables leverage step-down schedules.
   */
  threshold_schedule?: ThresholdSchedule | null;
}
/**
 * A covenant waiver or amendment granted by lenders.
 */
export interface CovenantWaiver {
  /**
   * Amended threshold (if this is an amendment rather than a full waiver).
   */
  amended_threshold?: number | null;
  /**
   * Stable instance identifier of the waived covenant
   * (from [`Covenant::instance_key`]).
   */
  covenant_id: string;
  /**
   * Free-text description of the waiver terms.
   */
  description: string;
  /**
   * Start date of the waiver period.
   */
  effective_date: DateWire;
  /**
   * End date of the waiver period (None = permanent amendment).
   */
  expiry_date?: DateWire | null;
}
/**
 * Covenant window for scheduled testing.
 */
export interface CovenantWindow {
  /**
   * Covenants active during this window
   */
  covenants: CovenantSpec[];
  /**
   * End date of the window
   */
  end: DateWire;
  /**
   * Start date of the window
   */
  start: DateWire;
}
/**
 * Forecast output with headroom analytics.
 */
export interface CovenantForecast {
  /**
   * Marginal breach probability at each test date; not a first-passage probability.
   */
  breach_probability: number[];
  /**
   * Standard error of the breach probability estimate.
   *
   * Analytic stochastic probabilities have zero estimator error.
   */
  breach_probability_stderr?: number[];
  /**
   * Comparison direction for the covenant threshold test.
   */
  comparator: BoundKind;
  /**
   * Human-readable covenant description.
   */
  covenant_description: string;
  /**
   * Stable covenant instance identifier.
   */
  covenant_id: string;
  /**
   * Date of first projected breach (if any)
   */
  first_breach_date?: DateWire | null;
  /**
   * Headroom (distance from breach) at each test date.
   *
   * `None` means the covenant is inactive for the period or the headroom is
   * not meaningful under the applicable covenant convention.
   */
  headroom: (number | null)[];
  /**
   * Date with minimum finite headroom.
   */
  min_headroom_date?: DateWire | null;
  /**
   * Minimum finite headroom value across all active test dates.
   */
  min_headroom_value?: number | null;
  /**
   * Projected metric values at each test date.
   *
   * `None` means the covenant is inactive or its earnings denominator makes
   * the ratio indeterminate. Non-finite caller observations are rejected.
   */
  projected_values: (number | null)[];
  /**
   * Future test dates for covenant evaluation
   */
  test_dates: DateWire[];
  /**
   * Covenant thresholds at each test date
   */
  thresholds: number[];
}
/**
 * Covenant forecast configuration.
 */
export interface CovenantForecastConfig {
  /**
   * Enables antithetic variate pairing in Monte Carlo mode.
   *
   * Each simulated unit draws `Z` and reuses `-Z` as a paired path,
   * halving the number of independent RNG draws and reducing estimator
   * variance for payoffs that are monotone in `Z` (Glasserman 2003,
   * *Monte Carlo Methods in Financial Engineering*, §4.1). Only
   * meaningful when `num_paths > 0`; `antithetic == true` with
   * `num_paths == 0` is rejected during validation as an inert flag.
   */
  antithetic?: boolean;
  /**
   * Minimum stochastic breach probability to include in batch breach output.
   */
  breach_probability_threshold?: number;
  /**
   * Selects the stochastic sub-mode: closed-form analytic vs. Monte Carlo.
   *
   * `0` (the default) uses the closed-form analytic lognormal
   * probabilities via [`norm_cdf`]. A value greater than `0` switches to
   * path-consistent Monte Carlo simulation with that many paths; with
   * `antithetic` enabled, paths are simulated in `(Z, -Z)` pairs and this
   * count is rounded up to a whole number of pairs. Ignored when
   * `stochastic` is `false`.
   */
  num_paths?: number;
  /**
   * RNG seed for Monte Carlo mode (`num_paths > 0`).
   *
   * `None` uses the crate-internal `DEFAULT_MC_SEED` (`0`). Ignored in analytic mode
   * (`num_paths == 0`) and when `stochastic` is `false`.
   */
  random_seed?: number | null;
  /**
   * Reference date for time-scaling lognormal shocks. When set, shocks scale with
   * `sqrt(T)` where T is the year-fraction from this date to the test date.
   * When `None`, the engine uses the end date of the period immediately
   * preceding the first forecast period, so the first simulated point still
   * has a non-zero forecast horizon.
   */
  reference_date?: DateWire | null;
  /**
   * Scope selected by batch forecasts: maintenance by default. Incurrence
   * forecasts are hypothetical capacity checks, not actual breach events.
   */
  scope?: CovenantScope;
  /**
   * Whether to use stochastic (as opposed to deterministic) breach probabilities.
   */
  stochastic?: boolean;
  /**
   * Volatility for stochastic scenarios (annualized).
   */
  volatility?: number | null;
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
 * Covenant reports produced for one test date of a series evaluation.
 */
export interface DatedCovenantReports {
  /**
   * Test date the covenants were evaluated on.
   */
  as_of: DateWire;
  /**
   * Reports keyed by stable covenant instance key, in engine order.
   */
  reports: {
    [k: string]: CovenantReport;
  };
}
/**
 * Covenant metric values observed on one date.
 */
export interface DatedMetrics {
  /**
   * Covenant test date the metric values belong to.
   */
  date: DateWire;
  /**
   * Metric values keyed by covenant metric identifier (for example
   * `debt_to_ebitda`), in the units the covenant tests expect: ratios in
   * turns (`4.5` means 4.5x), amounts in the reporting currency.
   */
  metrics: {
    [k: string]: number;
  };
}
/**
 * A projected covenant breach.
 */
export interface FutureBreach {
  /**
   * Date of the breach
   */
  breach_date: DateWire;
  /**
   * Probability of breach (if stochastic)
   */
  breach_probability: number;
  /**
   * Human-readable covenant description.
   */
  covenant_description: string;
  /**
   * Stable covenant instance identifier.
   */
  covenant_id: string;
  /**
   * Headroom (negative means breach), if meaningful and finite.
   */
  headroom?: number | null;
  /**
   * Projected value, if finite.
   */
  projected_value?: number | null;
  /**
   * Threshold value
   */
  threshold: number;
}
/**
 * Map-backed metric source for tests, bindings, and simple callers.
 */
export interface HashMapMetricSource {
  metrics: {
    [k: string]: number;
  };
  [k: string]: unknown;
}
