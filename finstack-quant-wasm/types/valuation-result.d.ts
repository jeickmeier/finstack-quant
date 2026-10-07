// Generated from the Rust facade host schema by scripts/generate-contract-types.mjs. Do not edit.

/**
 * Model-specific typed valuation details.
 *
 * These details are for rich structured outputs that do not fit the scalar
 * `measures` map while still belonging in the standard valuation envelope.
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "ValuationDetails".
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
 * Domain-specific trace entry types.
 *
 * Each variant captures relevant details for different types of computations:
 * - Calibration: solver steps as generic computation steps
 * - Pricing: cashflow-level PV breakdowns
 * - Waterfall: step-by-step payment allocations
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "TraceEntry".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "TraceEntry".
 */
export type TraceEntry =
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
      date: string;
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
      /**
       * Conditional survival probability to the payment date, `S(date) / S(as_of)`.
       *
       * Present only for credit-risky valuations. `pv_amount` then also
       * includes the recovery leg, so it is not `cashflow_amount ×
       * discount_factor × survival_probability`.
       */
      survival_probability?: number | null;
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
      period: bigint;
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
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "Id".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "Id".
 */
export type Id = string;
/**
 * ISO 4217 currency enumeration
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "Currency".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "Currency".
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
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "DateWire".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "DateWire".
 */
export type DateWire = string;
/**
 * Exact decimal encoded only as a JSON string.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "DecimalWire".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "DecimalWire".
 */
export type DecimalWire = string;
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
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "ModelKey".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "ModelKey".
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
 * Finite JSON number that is strictly greater than zero.
 *
 * This type is used by serde field adapters so runtime deserialization and
 * generated schemas enforce the same positive-number contract.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "PositiveF64Wire".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "PositiveF64Wire".
 */
export type PositiveF64Wire = number;
/**
 * Rounding modes supported by the library.
 *
 * The variants mirror the most common conventions found in pricing engines.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "RoundingMode".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "RoundingMode".
 */
export type RoundingMode = "bankers" | "away_from_zero" | "toward_zero" | "floor" | "ceil";
/**
 * Numeric schema revision for contracts whose sole supported revision is v1.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "SchemaVersion".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "SchemaVersion".
 */
export type SchemaVersion = number;
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
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "StructuredCreditPricingMode".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "StructuredCreditPricingMode".
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
         * Number of independent estimators; must be at least two to estimate
         * sampling uncertainty. With `antithetic` each estimator averages a
         * `(Z, -Z)` pair, so the engine prices `2 × num_paths` scenario paths.
         * Sample standard error and the Student-t interval use this estimator
         * count, with `num_paths - 1` degrees of freedom.
         */
        num_paths: bigint;
      };
    }
  | {
      hybrid: {
        /**
         * Monte Carlo continuation paths per tree prefix
         */
        num_paths: bigint;
        /**
         * Tree periods before switching to MC
         */
        tree_periods: bigint;
      };
    };
/**
 * Tranche seniority levels
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "TrancheSeniority".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "TrancheSeniority".
 */
export type TrancheSeniority = "senior" | "mezzanine" | "subordinated" | "equity";

/**
 * Facade valuation result. int64/uint64 fields are host bigint; JSON export uses integer tokens.
 */
export interface ValuationResult {
  /**
   * Valuation date (T+0) for the calculation.
   */
  as_of: string;
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
   * Requested with `PricingOptions::with_explain`. For discounting and
   * hazard-rate valuations it holds one `cashflow_pv` entry per projected
   * cashflow, reconciled to `value`; for other models it holds a single
   * `computation_step` entry saying no per-flow decomposition exists.
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
  meta: ResultsMeta1;
  /**
   * How the result was produced: model, requested date, declared market
   * dependencies, scenario adjustment and sensitivity bump sizes.
   *
   * Stamped by the pricer registry. Absent on results constructed directly.
   */
  provenance?: ValuationProvenance | null;
  /**
   * Required wire-format schema version. Only numeric `1` is accepted.
   */
  schema_version: number;
  value: Money5;
}
/**
 * Covenant check result.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "CovenantReport".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "CovenantReport".
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
 * Audit stamp: numeric mode, rounding context, and FX policy in force.
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
 * Rounding context snapshot applied to IO boundaries.
 */
export interface RoundingContext {
  /**
   * Validated ingest scale snapshot by currency code, with values in `0..=28`.
   */
  ingest_scale_by_currency: {
    [k: string]: number;
  };
  /**
   * Active rounding mode.
   */
  mode: "bankers" | "away_from_zero" | "toward_zero" | "floor" | "ceil";
  /**
   * Validated output scale snapshot by currency code, with values in `0..=28`.
   */
  output_scale_by_currency: {
    [k: string]: number;
  };
  tolerances?: ToleranceConfig;
  /**
   * Schema version for forward compatibility.
   */
  version: number;
  [k: string]: unknown;
}
/**
 * Tolerance settings snapshot for floating-point comparisons.
 */
export interface ToleranceConfig {
  /**
   * Epsilon for generic floating-point comparisons (default: 1e-10).
   *
   * Used for general numerical comparisons where higher tolerance is acceptable.
   */
  generic_epsilon?: number;
  /**
   * Epsilon for rate comparisons (default: 1e-12).
   *
   * Used when comparing interest rates, yields, and other small ratios.
   */
  rate_epsilon?: number;
}
/**
 * Rich structured details attached to composite valuation results.
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "CompositeValuationDetails".
 */
export interface CompositeValuationDetails {
  exposures: CompositeExposureReport;
  /**
   * Native and reporting-currency results for every top-level leg.
   */
  leg_results: CompositeLegValuation[];
  /**
   * Currency of all reported values and additive risk measures.
   */
  reporting_currency:
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
   * Frozen top-level quantities used for this valuation.
   */
  resolved_legs: ResolvedCompositeLeg[];
  /**
   * Effective date of the frozen holdings state used for pricing.
   */
  state_effective_date: string;
  /**
   * Scalar inputs retained when the state was resolved.
   */
  weighting_inputs: {
    [k: string]: number;
  };
}
/**
 * Recursive path-level and net/gross primitive exposures.
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
  reporting_currency:
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
}
/**
 * Net and gross exposure aggregated by primitive instrument identifier.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "PrimitiveAggregate".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "PrimitiveAggregate".
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
  gross_value: Money;
  /**
   * Primitive instrument identifier.
   */
  instrument_id: string;
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
  net_value: Money1;
}
/**
 * Sum of absolute reporting-currency path values.
 */
export interface Money {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Algebraic reporting-currency value.
 */
export interface Money1 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Path-level primitive exposure in a resolved composite tree.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "PrimitiveExposure".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "PrimitiveExposure".
 */
export interface PrimitiveExposure {
  /**
   * Primitive instrument identifier.
   */
  instrument_id: string;
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
  value: Money2;
}
/**
 * Reporting-currency signed value for this path.
 */
export interface Money2 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * One top-level leg result retained in composite valuation details.
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "CompositeLegValuation".
 */
export interface CompositeLegValuation {
  /**
   * Identifier of the top-level leg specification.
   */
  instrument_id: string;
  native_value: Money3;
  /**
   * Signed frozen leg quantity applied to the unit valuation.
   */
  quantity: number;
  reporting_value: Money4;
  valuation: ValuationResult1;
}
/**
 * Quantity-scaled value in the underlying instrument's native currency.
 */
export interface Money3 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Quantity-scaled value converted to the composite reporting currency.
 */
export interface Money4 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Complete unit-instrument valuation, including nested details where applicable.
 */
export interface ValuationResult1 {
  /**
   * Valuation date (T+0) for the calculation.
   */
  as_of: string;
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
   * Requested with `PricingOptions::with_explain`. For discounting and
   * hazard-rate valuations it holds one `cashflow_pv` entry per projected
   * cashflow, reconciled to `value`; for other models it holds a single
   * `computation_step` entry saying no per-flow decomposition exists.
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
  meta: ResultsMeta1;
  /**
   * How the result was produced: model, requested date, declared market
   * dependencies, scenario adjustment and sensitivity bump sizes.
   *
   * Stamped by the pricer registry. Absent on results constructed directly.
   */
  provenance?: ValuationProvenance | null;
  /**
   * Required wire-format schema version. Only numeric `1` is accepted.
   */
  schema_version: number;
  value: Money5;
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
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "ExplanationTrace".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "ExplanationTrace".
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
 * Calculation metadata and policy stamps.
 *
 * Contains:
 * - Numeric mode (Decimal vs f64)
 * - Rounding context and precision
 * - FX policy for cross-currency calculations
 * - Calculation timing information
 */
export interface ResultsMeta1 {
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
 * How a valuation result was produced.
 *
 * Stamped by [`crate::pricer::PricerRegistry::price_with_metrics`] on every
 * result so the number can be reproduced from the result plus the archived
 * market: the model that priced it, the date the caller asked for, the market
 * data the instrument declares, any scenario adjustment applied to the value,
 * and the bump sizes behind its sensitivities.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "ValuationProvenance".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "ValuationProvenance".
 */
export interface ValuationProvenance {
  market_dependencies: MarketDependencies;
  /**
   * Registered pricing model that produced `value`.
   */
  model:
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
   * Valuation date the caller requested.
   *
   * The result's `as_of` is the effective date after the instrument resolved
   * it (for example to the market's spot date); the two differ only when
   * the instrument moved it.
   */
  requested_as_of: string;
  /**
   * Scenario price shock already applied to `value`, as a decimal
   * (`-0.10` = the model value was multiplied by 0.90). Absent when the
   * value is the unadjusted model value.
   */
  scenario_price_shock_decimal?: number | null;
  /**
   * Bump sizes in force for the requested metrics. Absent when no metric
   * was requested.
   */
  sensitivity_bumps?: SensitivityBumps | null;
}
/**
 * Curves, surfaces, scalars, FX pairs and series the instrument declares
 * it reads. Identifiers refer to the `MarketContext` the result was
 * priced against.
 */
export interface MarketDependencies {
  /**
   * Credit-index aggregates resolved through `MarketContext::get_credit_index`.
   *
   * These identifiers are distinct from direct hazard-curve IDs because a
   * credit index also carries base correlation and optional issuer curves.
   */
  credit_index_ids?: Id[];
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
 * Curve dependencies grouped by type.
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
 * FX pair identifier using base/quote currency ordering.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "FxPair".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "FxPair".
 */
export interface FxPair {
  /**
   * Base currency (numerator).
   */
  base:
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
   * Quote currency (denominator).
   */
  quote:
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
}
/**
 * A volatility-surface dependency with the context needed for diagnostics.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "VolatilityDependency".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "VolatilityDependency".
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
  vol_surface_id: string;
}
/**
 * Finite-difference bump sizes in force for a metric request.
 *
 * These are the values after layering the `valuations.sensitivities.v1`
 * configuration extension and the instrument's
 * `metric_pricing_overrides.bump_config` over the library defaults. Bumped
 * sensitivities are reported per unit bump (per 1bp, per 1 vol point), so
 * these sizes describe how the difference was taken, not the reporting unit.
 *
 * A calculator with a fixed, documented shock of its own (for example an
 * analytic greek, or a metric whose rustdoc names its shock) does not read
 * these values.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "SensitivityBumps".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "SensitivityBumps".
 */
export interface SensitivityBumps {
  /**
   * Whether spot and volatility bumps are rescaled by volatility, time to
   * expiry and moneyness instead of applied at the fixed sizes above.
   */
  adaptive_bumps?: boolean;
  /**
   * Credit-spread bump in basis points (1.0 = 1bp).
   */
  credit_spread_bump_bp: number;
  /**
   * Parallel interest-rate bump in basis points (1.0 = 1bp).
   */
  rate_bump_bp: number;
  /**
   * Spot bump as a decimal fraction of spot (0.01 = 1%).
   */
  spot_bump_decimal: number;
  /**
   * Absolute volatility bump in decimal volatility (0.01 = 1 vol point).
   */
  vol_bump_decimal: number;
  /**
   * Yield bump in basis points for numerical yield duration and convexity.
   * Absent when each calculator keeps its own default shock.
   */
  ytm_bump_bp?: number | null;
}
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
export interface Money5 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Complete primitive decomposition with path, net, and gross views.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "CompositeExposureReport".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "CompositeExposureReport".
 */
export interface CompositeExposureReport1 {
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
  reporting_currency:
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
}
/**
 * Metadata for CDS-family valuation paths.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "CreditDerivativeValuationDetails".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "CreditDerivativeValuationDetails".
 */
export interface CreditDerivativeValuationDetails {
  /**
   * CDS integration method actually used, when applicable.
   */
  integration_method?: string | null;
  /**
   * Registered model key used by the pricer.
   */
  model_key:
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
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "FxValuationDetails".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "FxValuationDetails".
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
 * Collection of curves used by an instrument, categorized by market role.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "InstrumentCurves".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "InstrumentCurves".
 */
export interface InstrumentCurves1 {
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
 * Unified dependency container for instrument market data requirements.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "MarketDependencies".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "MarketDependencies".
 */
export interface MarketDependencies1 {
  /**
   * Credit-index aggregates resolved through `MarketContext::get_credit_index`.
   *
   * These identifiers are distinct from direct hazard-curve IDs because a
   * credit index also carries base correlation and optional issuer curves.
   */
  credit_index_ids?: Id[];
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
 * Currency-tagged monetary amount with safe arithmetic.
 *
 * Values retain decimal precision independently of ISO 4217 display precision.
 *
 * When you need configurable rounding during ingestion, use
 * [`Money::new_with_config`].
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "Money".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "Money".
 */
export interface Money6 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Reproducibility and convergence diagnostics for a Monte Carlo valuation.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "MonteCarloValuationDetails".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "MonteCarloValuationDetails".
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
  model_key:
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
   * Deterministic random seed used for the run.
   */
  seed: bigint;
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
 * Immutable resolved quantity for one top-level composite leg.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "ResolvedCompositeLeg".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "ResolvedCompositeLeg".
 */
export interface ResolvedCompositeLeg {
  /**
   * Identifier of the corresponding leg specification.
   */
  instrument_id: string;
  /**
   * Signed quantity held from the state's effective date until the next rebalance.
   */
  quantity: number;
}
/**
 * Metadata bundle that accompanies valuation outputs.
 *
 * The metadata is intentionally small so it can be attached to reports and
 * downstream data stores for reproducibility and audit trails.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "ResultsMeta".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "ResultsMeta".
 */
export interface ResultsMeta2 {
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
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "RoundingContext".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "RoundingContext".
 */
export interface RoundingContext1 {
  /**
   * Validated ingest scale snapshot by currency code, with values in `0..=28`.
   */
  ingest_scale_by_currency: {
    [k: string]: number;
  };
  /**
   * Active rounding mode.
   */
  mode: "bankers" | "away_from_zero" | "toward_zero" | "floor" | "ceil";
  /**
   * Validated output scale snapshot by currency code, with values in `0..=28`.
   */
  output_scale_by_currency: {
    [k: string]: number;
  };
  tolerances?: ToleranceConfig;
  /**
   * Schema version for forward compatibility.
   */
  version: number;
  [k: string]: unknown;
}
/**
 * Stochastic pricing result for a structured credit deal.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "StochasticPricingResult".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "StochasticPricingResult".
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
  draw_option_cost: Money7;
  /**
   * Per-path draw option cost in path order (same currency as `npv`),
   * populated only for pools with stochastic revolvers.
   */
  draw_option_cost_paths?: number[];
  /**
   * ES confidence level used
   */
  es_confidence: number;
  expected_collateral_draws: Money8;
  expected_loss: Money9;
  expected_shortfall: Money10;
  npv: Money11;
  /**
   * Number of simulated scenario paths (`2 × pricing_mode.num_paths` for
   * antithetic Monte Carlo, prefixes × suffixes for Hybrid).
   */
  num_paths: bigint;
  /**
   * Pricing mode used.
   */
  pricing_mode:
    | "tree"
    | {
        monte_carlo: {
          /**
           * Pair each estimator's path with its sign-flipped mirror.
           */
          antithetic: boolean;
          /**
           * Number of independent estimators; must be at least two to estimate
           * sampling uncertainty. With `antithetic` each estimator averages a
           * `(Z, -Z)` pair, so the engine prices `2 × num_paths` scenario paths.
           * Sample standard error and the Student-t interval use this estimator
           * count, with `num_paths - 1` degrees of freedom.
           */
          num_paths: bigint;
        };
      }
    | {
        hybrid: {
          /**
           * Monte Carlo continuation paths per tree prefix
           */
          num_paths: bigint;
          /**
           * Tree periods before switching to MC
           */
          tree_periods: bigint;
        };
      };
  /**
   * Two-sided 95% Student-t confidence interval for the mean PV, in the
   * NPV currency, using one fewer degree of freedom than the independent
   * estimator count. Coverage is approximate for non-Gaussian payoffs.
   *
   * @minItems 2
   * @maxItems 2
   */
  pv_confidence_interval: [unknown, unknown];
  /**
   * Sample standard error of the mean PV estimate, in the NPV currency.
   *
   * Uses Bessel-corrected variance over independent estimators; each
   * antithetic pair contributes one mean observation.
   */
  pv_std_error: number;
  /**
   * Tranche-level results
   */
  tranche_results: TranchePricingResult[];
  unexpected_loss: Money17;
  /**
   * Fraction of paths on which a collateral draw could not be funded from
   * the reserve account and that period's principal collections (`0.0`
   * for pools without draws).
   */
  unfunded_draw_path_fraction?: number;
  [k: string]: unknown;
}
/**
 * Mean over paths of the draw option cost: the value to the deal of its
 * revolvers' simulated draws having been made at the contractual margin
 * instead of each path's fair spread, obtained per path as the present
 * value of the actual tranche cashflows less that of a counterfactual
 * run where every draw accrues at its fair spread. Negative when spreads
 * widen; zero for pools without stochastic revolvers.
 */
export interface Money7 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Mean over paths of the collateral draws funded through the reserve
 * account and principal collections (revolver utilization increases,
 * delayed draws and loan-equivalent draws at default).
 */
export interface Money8 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Expected loss (probability-weighted average loss)
 */
export interface Money9 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Expected shortfall (tail risk metric)
 */
export interface Money10 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Net present value of the deal
 */
export interface Money11 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Tranche-level pricing result.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "TranchePricingResult".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "TranchePricingResult".
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
  draw_option_cost: Money12;
  expected_loss: Money13;
  expected_shortfall: Money14;
  npv: Money15;
  /**
   * Number of simulated paths on which the tranche received any
   * principal (the paths `wal` averages over).
   */
  paths_with_principal?: bigint;
  /**
   * Mean present value as a percent of the tranche's current balance at
   * the valuation date (100 = par); `0.0` for a fully retired tranche.
   */
  price_pct: number;
  /**
   * Tranche seniority level.
   */
  seniority: "senior" | "mezzanine" | "subordinated" | "equity";
  /**
   * Tranche identifier
   */
  tranche_id: string;
  unexpected_loss: Money16;
  /**
   * Weighted-average life (WAL, years) averaged over the paths on which
   * the tranche received principal; `0.0` when it never did.
   */
  wal: number;
  [k: string]: unknown;
}
/**
 * This tranche's share of the deal's draw option cost: the mean over
 * paths of its present value on the actual run less that on the
 * counterfactual run where revolver draws accrue at their fair spread.
 * Tranche shares sum to the deal's `draw_option_cost` on every path.
 */
export interface Money12 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Expected loss
 */
export interface Money13 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Expected shortfall
 */
export interface Money14 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Net present value
 */
export interface Money15 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Unexpected loss
 */
export interface Money16 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Unexpected loss (loss standard deviation)
 */
export interface Money17 {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: string;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency:
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
}
/**
 * Numerical tolerance configuration for floating-point comparisons.
 *
 * Provides configurable epsilon values for zero-checks in rate calculations
 * and generic floating-point comparisons. These defaults are chosen to balance
 * numerical stability with practical precision requirements.
 *
 * This interface was referenced by `ValuationResult1`'s JSON-Schema
 * via the `definition` "ToleranceConfig".
 *
 * This interface was referenced by `ValuationResult`'s JSON-Schema
 * via the `definition` "ToleranceConfig".
 */
export interface ToleranceConfig1 {
  /**
   * Epsilon for generic floating-point comparisons (default: 1e-10).
   *
   * Used for general numerical comparisons where higher tolerance is acceptable.
   */
  generic_epsilon?: number;
  /**
   * Epsilon for rate comparisons (default: 1e-12).
   *
   * Used when comparing interest rates, yields, and other small ratios.
   */
  rate_epsilon?: number;
}
