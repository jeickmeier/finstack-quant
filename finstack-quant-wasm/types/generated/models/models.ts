// Generated from the finstack-quant-models JSON schemas by scripts/generate-contract-types.mjs. Do not edit.

/**
 * Source provenance of an issuer's idiosyncratic vol estimate.
 */
export type AdderVolSource =
  | "from_history"
  | {
      bucket_peer_proxy: {
        /**
         * Dotted bucket path used as proxy (e.g. `"IG.EU.FIN"`).
         */
        peer_bucket: string;
        [k: string]: unknown;
      };
    }
  | "caller_supplied"
  | "default";
/**
 * Categorical severity of an arbitrage violation.
 *
 * Severity is a *function of magnitude relative to tolerance*, not of
 * arbitrage type. The same `Butterfly` violation can be `Negligible` (a
 * rounding artifact at deep wings) or `Critical` (negative density at ATM).
 * Use the `Ord` derive to filter or sort: `Negligible` < `Minor` < `Major`
 * < `Critical`.
 */
export type ArbitrageSeverity = "negligible" | "minor" | "major" | "critical";
/**
 * Classification of the arbitrage condition that was violated.
 */
export type ArbitrageType =
  | "butterfly"
  | "calendar_spread"
  | "local_vol_density"
  | "svi_moment_bound"
  | "svi_butterfly_condition"
  | "svi_calendar_spread";
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
 * Asian averaging method.
 */
export type AveragingMethod = "arithmetic" | "geometric";
/**
 * Supported regression basis families for LSMC pricers.
 */
export type BasisKind = "laguerre" | "polynomial" | "normalized_polynomial";
/**
 * OLS β shrinkage rule.
 */
export type BetaShrinkage =
  | "none"
  | {
      toward_one: {
        /**
         * Shrinkage weight in `[0, 1]`.
         */
        alpha: number;
        [k: string]: unknown;
      };
    };
/**
 * How bucket factor means are weighted across issuers in the bucket.
 *
 * DTS (duration-times-spread) is the desk-standard credit-factor weight
 * (Ben Dor / Barclays): it avoids overweighting tight or short-duration
 * names. Position risk exposure remains `β × CS01`; DTS does not replace
 * CS01. [`BucketWeighting::Equal`] is the opt-out for tests and simple
 * equally-weighted books.
 *
 * The historical peel weights each date by **contemporaneous** DTS
 * (`SD × begin-of-period spread` in Returns space, `SD × same-date spread`
 * in Levels space), so no as-of information enters historical factor
 * construction. The anchor peel and decomposition use the as-of / current
 * cross-section DTS. Spread durations are a single per-issuer value across
 * the window; duration drift within the window is a documented
 * simplification.
 */
export type BucketWeighting = "equal" | "dts";
/**
 * Units for the bump magnitude. These control normalization to fraction or factor.
 */
export type BumpUnits = "rate_bp" | "percent" | "fraction" | "factor";
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
 * Finite JSON number in the closed interval `[0, 1]`.
 */
export type ClosedUnitIntervalF64Wire = number;
/**
 * Collateral asset class with associated liquidation haircut.
 *
 * Haircuts represent the discount from book value realized in a
 * forced-sale / workout scenario. Values are in \[0, 1\] where 0 means
 * full recovery of book value and 1 means total loss.
 */
export type WorkoutCollateralType =
  "cash" | "securities" | "receivables" | "inventory" | "equipment" | "real_estate" | "intellectual_property" | "other";
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
 * Correlation structure specification.
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
 * Finite correlation coefficient in the closed interval `[-1, 1]`.
 */
export type CorrelationWire = number;
/**
 * Strategy for assembling the factor covariance matrix.
 */
export type CovarianceStrategy =
  | "diagonal"
  | {
      ridge: {
        /**
         * Ridge regularisation in annualized **bp²**; must be `>= 0`.
         */
        alpha: number;
        [k: string]: unknown;
      };
    }
  | "full_sample_repaired"
  | "ledoit_wolf";
/**
 * A single level in the credit factor hierarchy.
 *
 * Built-in variants (`Rating`, `Region`, `Sector`) have canonical tag keys.
 * `Custom(key)` reads `issuer_tags[key]` for arbitrary user-defined dimensions
 * such as `"Currency"` or `"AssetType"`.
 */
export type HierarchyDimension =
  | "rating"
  | "region"
  | "sector"
  | {
      custom: string;
    };
/**
 * Observation frequency of a complete, regular credit history panel.
 *
 * Annualization used for sample/EWMA variance and Ledoit-Wolf covariance
 * is derived from this enum (`252` / `12` / `4`). There is no free
 * annualization float.
 */
export type PanelFrequency = "daily" | "monthly" | "quarterly";
/**
 * Calibration policy governing which issuers receive a per-issuer regression.
 *
 * - `Dynamic` — apply a minimum-history threshold and honour per-issuer overrides.
 * - `GloballyOff` — every issuer is treated as `BucketOnly`; no per-issuer
 *   regression is run.  Useful for simpler factor models or data-sparse periods.
 */
export type IssuerBetaPolicy =
  | {
      dynamic: {
        /**
         * Minimum number of monthly return observations needed to attempt OLS.
         *
         * Default is 24 months.
         */
        min_history: number;
        /**
         * Per-issuer overrides that can force or suppress per-issuer regression.
         *
         * Keys without an entry default to [`IssuerBetaOverride::Auto`].
         */
        overrides: {
          [k: string]: IssuerBetaOverride;
        };
        [k: string]: unknown;
      };
    }
  | "globally_off";
/**
 * Per-issuer regression behavior override supplied by the user before calibration.
 *
 * This is the *input* override; the *resolved* outcome is [`IssuerBetaMode`].
 *
 * - `Auto` — let the calibration decide based on `min_history`.
 * - `ForceIssuerBeta` — always run per-issuer regression regardless of history.
 * - `ForceBucketOnly` — never run per-issuer regression for this issuer.
 */
export type IssuerBetaOverride = "auto" | "force_issuer_beta" | "force_bucket_only";
/**
 * Whether the calibrator works in price-difference (return) or raw-level space.
 *
 * `Returns` (the default) matches the spec's reference math: `r_i(t) =
 * S_i(t) - S_i(t-1)` and the generic factor is differenced the same way.
 */
export type PanelSpace = "returns" | "levels";
/**
 * Volatility model selector for the per-factor variance forecast.
 *
 * `Sample` is the plain (unbiased) sample variance. `Ewma` is the RiskMetrics
 * finite-window exponentially weighted variance estimator (Longerstaey &
 * Spencer, 1996, §5.2): both are fully supported by the calibrator.
 *
 * The two differ in centering: `Sample` demeans the series before squaring
 * (the usual `Var(x) = E[(x − x̄)²]`), while `Ewma` does not — it recurses
 * directly on squared observations (`σ²_t = λσ²_{t−1} + (1−λ)r²_{t−1}`),
 * matching the RiskMetrics convention of treating financial return series as
 * zero-mean. Both estimators always operate on factor and adder *moves*:
 * under [`PanelSpace::Levels`] the calibrator first-differences the peeled
 * level series before estimating variance, so the zero-mean convention is
 * sound in either panel space.
 */
export type VolModelChoice =
  | "sample"
  | {
      ewma: {
        /**
         * Smoothing parameter λ ∈ (0, 1) (RiskMetrics daily default 0.94).
         */
        lambda: number;
        [k: string]: unknown;
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
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
/**
 * Unique identifier for a risk factor.
 */
export type FactorId = string;
/**
 * Broad classification of a risk factor.
 */
export type FactorType =
  | "rates"
  | "credit"
  | "equity"
  | "fx"
  | "volatility"
  | "commodity"
  | "inflation"
  | {
      custom: string;
    };
/**
 * How a factor movement translates to market-data perturbations.
 */
export type MarketMapping =
  | {
      curve_parallel: {
        /**
         * Curves bumped by the factor.
         */
        curve_ids: Id[];
        /**
         * Units used for the bump magnitude.
         */
        units: BumpUnits;
        [k: string]: unknown;
      };
    }
  | {
      curve_bucketed: {
        /**
         * Curve receiving the bucketed bump.
         */
        curve_id: Id;
        /**
         * `(tenor_years, weight)` pairs describing the bucketed shift.
         *
         * Weights are typically chosen to sum to `1.0` across the buckets used
         * by the factor so that the factor move preserves the intended overall
         * shock size while localizing it along the tenor axis.
         */
        tenor_weights: [unknown, unknown][];
        [k: string]: unknown;
      };
    }
  | {
      equity_spot: {
        /**
         * Tickers moved by the factor.
         */
        tickers: string[];
        [k: string]: unknown;
      };
    }
  | {
      fx_rate: {
        /**
         * Currency pair moved by the factor.
         *
         * @minItems 2
         * @maxItems 2
         */
        pair: [unknown, unknown];
        [k: string]: unknown;
      };
    }
  | {
      vol_shift: {
        /**
         * Units used for the bump magnitude.
         */
        units: BumpUnits;
        /**
         * Volatility surfaces moved by the factor.
         */
        vol_surface_ids: string[];
        [k: string]: unknown;
      };
    };
/**
 * Declarative matcher configuration that can be serialized and rebuilt.
 */
export type MatchingConfig =
  | {
      mapping_table: MappingRule[];
    }
  | {
      cascade: MatchingConfig[];
    }
  | {
      hierarchical: HierarchicalConfig;
    }
  | {
      credit_hierarchical: CreditHierarchicalConfig;
    };
/**
 * Classification of a curve dependency's role.
 */
export type CurveType = "discount" | "forward" | "hazard" | "inflation" | "base_correlation";
/**
 * Classification used by dependency filters and declarative matching config.
 */
export type DependencyType = "discount" | "forward" | "credit" | "spot" | "vol" | "fx" | "series";
/**
 * Resolved regression mode stored in the calibrated artifact.
 *
 * A `BucketOnly` issuer's betas are all 1.0 and carry no fit statistics.
 */
export type IssuerBetaMode = "issuer_beta" | "bucket_only";
/**
 * Strategy used when extracting factor sensitivities.
 */
export type PricingMode = "delta_based" | "full_repricing";
/**
 * Risk measure used when aggregating factor exposures.
 */
export type RiskMeasure =
  | "variance"
  | "volatility"
  | {
      var: {
        /**
         * Confidence level in the open interval `(0.5, 1)`.
         */
        confidence: number;
        [k: string]: unknown;
      };
    }
  | {
      expected_shortfall: {
        /**
         * Confidence level in the open interval `(0.5, 1)`.
         */
        confidence: number;
        [k: string]: unknown;
      };
    };
/**
 * Policy for handling dependencies that do not match any factor.
 *
 * Serializes in `snake_case`, matching the crate-wide wire convention and
 * this type's own `Display` representation.
 */
export type UnmatchedPolicy = "strict" | "residual" | "warn";
/**
 * Sole supported credit-factor-model contract marker.
 */
export type CreditFactorModelSchema = "finstack_quant.credit_factor_model/1";
/**
 * Volatility model for a single factor.
 *
 * The `Sample` variant stores a single variance estimate; `Ewma` additionally
 * persists the smoothing parameter used at calibration time.
 */
export type FactorVolModel =
  | {
      sample: {
        /**
         * Annualized variance estimate for this factor.
         */
        variance: number;
      };
    }
  | {
      ewma: {
        /**
         * Smoothing parameter λ ∈ (0, 1) used at calibration time.
         */
        lambda: number;
        /**
         * Annualized one-step-ahead variance forecast.
         */
        variance: number;
      };
    };
/**
 * Volatility model for an issuer's idiosyncratic adder.
 *
 * Mirrors [`FactorVolModel`] in structure; kept separate so per-issuer and
 * per-factor models can diverge independently in later PRs.
 */
export type IdiosyncraticVolModel =
  | {
      sample: {
        /**
         * Annualized variance of the issuer's idiosyncratic adder.
         */
        variance: number;
        [k: string]: unknown;
      };
    }
  | {
      ewma: {
        /**
         * Smoothing parameter λ ∈ (0, 1) used at calibration time.
         */
        lambda: number;
        /**
         * Annualized one-step-ahead variance forecast for the idiosyncratic
         * adder.
         */
        variance: number;
        [k: string]: unknown;
      };
    };
/**
 * Which credit metric drives the toggle decision.
 */
export type CreditStateVariable = "hazard_rate" | "distance_to_default" | "leverage";
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
 * Exact decimal encoded only as a JSON string.
 */
export type DecimalWire = string;
/**
 * Method used for position-level VaR/ES decomposition.
 */
export type DecompositionMethod = "parametric" | "historical";
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
 * Structure of a distressed exchange offer.
 *
 * The variant records how the new instrument ranks against the existing claim
 * and is reported back on [`ExchangeOfferAnalysis`] for audit purposes; it
 * does not change the arithmetic.
 */
export type ExchangeType = "par_for_par" | "discount" | "uptier" | "downtier";
/**
 * Exercise schedule convention for option models.
 */
export type ExerciseStyle = "european" | "american" | "bermudan";
/**
 * Exact schema marker accepted by [`FactorModelConfigEnvelope`].
 */
export type FactorModelConfigSchema = "finstack_quant.factor_model_config/1";
/**
 * Factor model specification for configuration and serialization.
 */
export type LatentFactorSpec =
  | {
      /**
       * Mean reversion speed (0 = random walk)
       */
      mean_reversion: number;
      type: "single_factor";
      /**
       * Factor volatility (std dev of innovations)
       */
      volatility: number;
    }
  | {
      /**
       * Correlation between prepayment and credit factors
       */
      correlation: number;
      /**
       * Credit factor volatility
       */
      credit_vol: number;
      /**
       * Prepayment factor volatility
       */
      prepay_vol: number;
      type: "two_factor";
    }
  | {
      /**
       * Correlation matrix (flattened row-major)
       */
      correlations: number[];
      /**
       * Number of factors
       */
      num_factors: number;
      type: "multi_factor";
      /**
       * Factor volatilities
       */
      volatilities: number[];
    };
/**
 * How [`LiquidityProfile::spread_volatility`] should be interpreted.
 *
 * Bangia et al. (1999) phrase the LVaR add-on in terms of the volatility of the
 * **relative** (proportional) spread. Some data providers quote spread
 * volatility in absolute price units instead. This enum selects the convention;
 * the LVaR calculator normalizes absolute spread volatilities to relative
 * before combining with `z_alpha` and position value.
 *
 * The default is [`SpreadVolatilityKind::Relative`] to match the original
 * Bangia convention.
 */
export type SpreadVolatilityKind = "relative" | "absolute";
/**
 * Liquidity tier classification based on days-to-liquidate.
 */
export type LiquidityTier = "tier1" | "tier2" | "tier3" | "tier4" | "tier5";
/**
 * Structure of a liability management exercise.
 *
 * Each variant reinterprets the `repurchase_price_pct` argument of
 * [`analyze_lme`]; see that function's documentation for the per-variant
 * meaning and admissible range.
 */
export type LmeType = "open_market_repurchase" | "tender_offer" | "amend_and_extend" | "dropdown";
/**
 * A single market dependency extracted from an instrument.
 */
export type MarketDependency =
  | {
      curve: {
        /**
         * Role played by the curve.
         */
        curve_type: CurveType;
        /**
         * Curve identifier.
         */
        id: Id;
      };
    }
  | {
      credit_curve: {
        /**
         * Curve identifier.
         */
        id: Id;
      };
    }
  | {
      credit_index: {
        /**
         * Credit-index aggregate identifier.
         */
        id: Id;
      };
    }
  | {
      spot: {
        /**
         * Spot identifier or ticker.
         */
        id: string;
      };
    }
  | {
      vol_surface: {
        /**
         * Surface identifier.
         */
        id: string;
      };
    }
  | {
      fx_pair: {
        /**
         * Base currency.
         */
        base: Currency;
        /**
         * Quote currency.
         */
        quote: Currency;
      };
    }
  | {
      series: {
        /**
         * Series identifier.
         */
        id: string;
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
 * Finite JSON number greater than or equal to zero.
 */
export type NonNegativeF64Wire = number;
/**
 * Vanilla option kind for barrier payoff evaluation.
 */
export type OptionKind = "call" | "put";
/**
 * Option payoff direction used by analytical and numerical model engines.
 */
export type OptionType = "call" | "put";
/**
 * AssetPool-granularity policy for the structured-credit default engine.
 */
export type PoolGranularity = "per_name" | "large_homogeneous";
/**
 * Source of a per-position residual variance contribution.
 *
 * Distinguishes residuals derived from a credit factor model (where the
 * idiosyncratic adder is calibrated per issuer) from generic / unattributed
 * residual sources used by other decomposers.
 */
export type ResidualContributionSource =
  | {
      /**
       * Issuer whose idiosyncratic vol drives this residual contribution.
       */
      issuer_id: string;
      kind: "from_credit_model";
      [k: string]: unknown;
    }
  | {
      kind: "other";
      [k: string]: unknown;
    };
/**
 * Finite JSON number that is strictly greater than zero.
 *
 * This type is used by serde field adapters so runtime deserialization and
 * generated schemas enforce the same positive-number contract.
 */
export type PositiveF64Wire = number;
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
 * Zone classification across all scoring models.
 *
 * Represents the risk category derived from a model's score:
 * - `Safe`: low bankruptcy probability.
 * - `Grey`: ambiguous / requires further analysis.
 * - `Distress`: high bankruptcy probability.
 */
export type ScoringZone = "safe" | "grey" | "distress";
/**
 * Debt seniority classification for recovery rate modeling.
 *
 * Recovery rates vary significantly by position in the capital structure.
 * These classes align with rating agency (Moody's, S&P) reporting categories.
 */
export type SeniorityClass =
  | "1st_lien_secured"
  | "2nd_lien_secured"
  | "senior_secured"
  | "senior_unsecured"
  | "subordinated"
  | "junior_subordinated";
/**
 * Stochastic default model specification.
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
      base_spec: DefaultModelSpec;
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
 * Stochastic prepayment model specification.
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
      base_spec: PrepaymentModelSpec;
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
 * Direction for threshold comparison.
 */
export type ThresholdDirection = "above" | "below";
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
 * Volatility quoting convention.
 */
export type VolatilityConvention =
  | "normal"
  | "lognormal"
  | {
      shifted_lognormal: {
        /**
         * Additive displacement in the same decimal rate or price units as forward and strike
         */
        shift: number;
      };
    };

/**
 * Almgren-Chriss (2001) market impact model.
 */
export interface AlmgrenChrissModel {
  /**
   * Power-law exponent for temporary impact (delta).
   * Typically 0.5-0.6 for equities.
   */
  delta: number;
  /**
   * Temporary impact coefficient (eta).
   */
  eta: number;
  /**
   * Permanent impact coefficient (gamma).
   */
  gamma: number;
  [k: string]: unknown;
}
/**
 * Input ratios for the Altman Z''-Score (non-manufacturing firms).
 */
export interface AltmanZDoublePrimeInput {
  /**
   * X4: Book Value of Equity / Book Value of Total Liabilities.
   */
  book_equity_to_total_liabilities: number;
  /**
   * X3: EBIT / Total Assets.
   */
  ebit_to_total_assets: number;
  /**
   * X2: Retained Earnings / Total Assets.
   */
  retained_earnings_to_total_assets: number;
  /**
   * X1: Working Capital / Total Assets.
   */
  working_capital_to_total_assets: number;
  [k: string]: unknown;
}
/**
 * Input ratios for the Altman Z'-Score (private firms).
 */
export interface AltmanZPrimeInput {
  /**
   * X4: Book Value of Equity / Book Value of Total Liabilities.
   */
  book_equity_to_total_liabilities: number;
  /**
   * X3: EBIT / Total Assets.
   */
  ebit_to_total_assets: number;
  /**
   * X2: Retained Earnings / Total Assets.
   */
  retained_earnings_to_total_assets: number;
  /**
   * X5: Sales / Total Assets.
   */
  sales_to_total_assets: number;
  /**
   * X1: Working Capital / Total Assets.
   */
  working_capital_to_total_assets: number;
  [k: string]: unknown;
}
/**
 * Input ratios for the original Altman Z-Score (1968).
 */
export interface AltmanZScoreInput {
  /**
   * X3: EBIT / Total Assets.
   */
  ebit_to_total_assets: number;
  /**
   * X4: Market Value of Equity / Book Value of Total Liabilities.
   */
  market_equity_to_total_liabilities: number;
  /**
   * X2: Retained Earnings / Total Assets.
   */
  retained_earnings_to_total_assets: number;
  /**
   * X5: Sales / Total Assets.
   */
  sales_to_total_assets: number;
  /**
   * X1: Working Capital / Total Assets.
   */
  working_capital_to_total_assets: number;
  [k: string]: unknown;
}
/**
 * Configuration for the arbitrage detection suite.
 */
export interface ArbitrageCheckConfig {
  /**
   * Run butterfly (strike convexity) check.
   */
  check_butterfly: boolean;
  /**
   * Run calendar spread (expiry monotonicity) check.
   */
  check_calendar_spread: boolean;
  /**
   * Run Dupire local vol density check.
   */
  check_local_vol_density: boolean;
  /**
   * Optional per-expiry forward prices used by butterfly, calendar, and
   * local-vol density checks.
   *
   * Required when any check is enabled. Supply one finite positive value
   * to broadcast across expiries or one such value per expiry.
   */
  forward_prices?: number[] | null;
  /**
   * Minimum severity to include in the report.
   * Violations below this severity are filtered out.
   */
  min_severity: ArbitrageSeverity;
  /**
   * Tolerance for all checks (total-variance units).
   */
  tolerance: number;
}
/**
 * Aggregated arbitrage report for a volatility surface.
 */
export interface ArbitrageReport {
  /**
   * Count of violations by severity.
   */
  counts_by_severity: {
    [k: string]: number;
  };
  /**
   * Count of violations by type.
   */
  counts_by_type: {
    [k: string]: number;
  };
  /**
   * Whether the surface passes all checks (no violations above Negligible).
   */
  passed: boolean;
  /**
   * All violations found, sorted by severity (critical first).
   */
  violations: ArbitrageViolation[];
  /**
   * Identifier of the volatility surface that was checked.
   */
  vol_surface_id: string;
  [k: string]: unknown;
}
/**
 * A single arbitrage violation detected on a volatility surface.
 */
export interface ArbitrageViolation {
  /**
   * Human-readable description of the violation.
   */
  description: string;
  /**
   * Where on the surface the violation occurs.
   */
  location: ViolationLocation;
  /**
   * Magnitude of the violation in total-variance units.
   * For butterfly: the negative second derivative value.
   * For calendar spread: the variance decrease (w1 - w2) where w2 < w1.
   * For local vol density: the negative local variance value.
   */
  magnitude: number;
  /**
   * Categorical severity.
   */
  severity: ArbitrageSeverity;
  /**
   * Suggested adjustment to the implied vol at this point to
   * remove the violation (in vol units, additive). `None` if no
   * simple fix is available.
   */
  suggested_fix?: number | null;
  /**
   * What type of arbitrage condition was violated.
   */
  violation_type: ArbitrageType;
  [k: string]: unknown;
}
/**
 * A point on the volatility surface where an arbitrage condition is violated.
 */
export interface ViolationLocation {
  /**
   * For calendar spread violations: the adjacent expiry involved.
   */
  adjacent_expiry?: number | null;
  /**
   * Expiry (years) at which the violation occurs.
   */
  expiry: number;
  /**
   * Strike or log-moneyness at which the violation occurs.
   */
  strike: number;
  [k: string]: unknown;
}
/**
 * Result of arbitrage validation, containing any violations found.
 */
export interface ArbitrageValidationResult {
  /**
   * `true` when both violation lists are empty.
   */
  arbitrage_free: boolean;
  /**
   * Strikes where butterfly spread is negative (convexity violation)
   */
  butterfly_violations: ButterflyViolation[];
  /**
   * Pairs of strikes where call prices increase (monotonicity violation)
   */
  monotonicity_violations: MonotonicityViolation[];
  [k: string]: unknown;
}
/**
 * A butterfly spread violation at a specific strike.
 */
export interface ButterflyViolation {
  /**
   * Butterfly spread value (negative indicates violation)
   */
  butterfly_value: number;
  /**
   * Severity as percentage of mid-strike price
   */
  severity_pct: number;
  /**
   * Strike at which the violation occurs
   */
  strike: number;
  [k: string]: unknown;
}
/**
 * A monotonicity violation between two strikes.
 */
export interface MonotonicityViolation {
  /**
   * Call price at higher strike (should be lower)
   */
  price_high: number;
  /**
   * Call price at lower strike
   */
  price_low: number;
  /**
   * Higher strike
   */
  strike_high: number;
  /**
   * Lower strike
   */
  strike_low: number;
  [k: string]: unknown;
}
/**
 * Filters on instrument metadata.
 *
 * All configured conditions are combined with logical AND.
 */
export interface AttributeFilter {
  /**
   * Metadata key/value pairs that must all match exactly.
   */
  meta?: [unknown, unknown][];
  /**
   * Tags that must all be present on the instrument.
   */
  tags?: string[];
}
/**
 * Beta distribution parameterization for recovery rates.
 *
 * The Beta distribution is defined on \[0,1\] and parameterized here by
 * its mean and standard deviation, which map to shape parameters:
 *
 * ```text
 * alpha = mean * ((mean * (1 - mean) / variance) - 1)
 * beta = (1 - mean) * ((mean * (1 - mean) / variance) - 1)
 * ```
 *
 * Constraint: variance < mean * (1 - mean), ensuring alpha > 0 and beta > 0.
 *
 * # References
 *
 * - Altman, E. I., Resti, A., & Sironi, A. (2005). "Recovery Risk." Risk Books. `docs/REFERENCES.md#altman-et-al-2005-recovery`
 * - Schuermann, T. (2004). "What Do We Know About Loss Given Default?"
 *   Wharton Financial Institutions Center Working Paper 04-01. `docs/REFERENCES.md#schuermann-2004-lgd`
 */
export interface BetaRecovery {
  alpha: number;
  beta_param: number;
  mean: number;
  std_dev: number;
}
/**
 * Black-Scholes characteristic function.
 */
export interface BlackScholesCf {
  /**
   * Dividend yield.
   */
  q: number;
  /**
   * Risk-free rate.
   */
  r: number;
  /**
   * Volatility (annualized).
   */
  sigma: number;
  [k: string]: unknown;
}
/**
 * Parameters for one-dimensional Brownian motion with drift.
 */
export interface BrownianParams {
  /**
   * Constant drift per year.
   */
  mu: number;
  /**
   * Constant diffusion scale per square root year.
   */
  sigma: number;
  [k: string]: unknown;
}
/**
 * Black–Scholes/Garman–Kohlhagen Greeks (per unit, not scaled by contract size).
 */
export interface BsGreeks {
  /**
   * Delta sensitivity per unit.
   */
  delta: number;
  /**
   * Gamma sensitivity per unit.
   */
  gamma: number;
  /**
   * Rho to the foreign/dividend yield per 1%.
   */
  rho_q: number;
  /**
   * Rho to the domestic/risk-free rate per 1%.
   */
  rho_r: number;
  /**
   * Theta per day (scaled by provided day-count basis).
   */
  theta: number;
  /**
   * Vega per 1% volatility move.
   */
  vega: number;
}
/**
 * Per-level minimum-bucket-size thresholds used to gate fold-up of sparse
 * hierarchy buckets.
 */
export interface BucketSizeThresholds {
  /**
   * Threshold per hierarchy level. Levels beyond `per_level.len()` use the
   * default of 5.
   */
  per_level: number[];
}
/**
 * Per-factor-type bump magnitudes for finite-difference sensitivity engines.
 *
 * Unknown fields are rejected on deserialization: every field here has a
 * serde default, so a typo'd key (e.g. `"credit_bp"`) would otherwise be
 * silently dropped and the bump would silently revert to 1.0.
 */
export interface BumpSizeConfig {
  /**
   * Default credit bump in basis points.
   */
  credit_bp?: number;
  /**
   * Default equity spot bump in percent.
   */
  equity_pct?: number;
  /**
   * Default FX spot bump in percent.
   */
  fx_pct?: number;
  /**
   * Per-factor overrides that take precedence over factor-type defaults.
   */
  overrides?: {
    [k: string]: number;
  };
  /**
   * Default rates bump in basis points.
   */
  rates_bp?: number;
  /**
   * Default volatility bump in vol points (`1.0` = one vol point =
   * `0.01` absolute vol).
   */
  vol_points?: number;
}
/**
 * Cheyette + rough stochastic volatility model parameters.
 */
export interface CheyetteRoughVolParams {
  /**
   * Vol-of-vol scaling (η > 0).
   */
  eta: number;
  /**
   * Hurst exponent H ∈ (0, 0.5) for the rough vol driver.
   */
  hurst: HurstExponent;
  /**
   * Mean reversion of the short rate (κ > 0).
   */
  kappa: number;
  /**
   * Correlation between rate and vol innovations ρ ∈ [-1, 1].
   */
  rho: number;
  [k: string]: unknown;
}
/**
 * Validated Hurst exponent H ∈ (0, 1).
 *
 * The Hurst exponent determines the roughness of fractional Brownian motion:
 *
 * - H < 0.5 — rough (anti-persistent increments)
 * - H = 0.5 — standard Brownian motion
 * - H > 0.5 — smooth (persistent increments)
 */
export interface HurstExponent {
  /**
   * The Hurst parameter value.
   */
  h: number;
  [k: string]: unknown;
}
/**
 * CIR process parameters.
 */
export interface CirParams {
  /**
   * Mean reversion speed (κ)
   */
  kappa: number;
  /**
   * Volatility of volatility (σ)
   */
  sigma: number;
  /**
   * Long-term mean (θ)
   */
  theta: number;
  [k: string]: unknown;
}
/**
 * A single piece of collateral in the recovery waterfall.
 */
export interface CollateralPiece {
  /**
   * Book value (pre-haircut) of the collateral.
   */
  book_value: number;
  /**
   * Collateral asset class.
   */
  collateral_type: WorkoutCollateralType;
  /**
   * Liquidation haircut in \[0, 1\]. Applied as: liquidation_value = book_value * (1 - haircut).
   */
  haircut: number;
  [k: string]: unknown;
}
/**
 * COS method configuration.
 */
export interface CosConfig {
  /**
   * Number of cosine terms in `1..=`[`CosConfig::MAX_TERMS`] (default: 128).
   * More terms = higher accuracy for non-smooth or heavy-tailed densities.
   */
  num_terms: number;
  /**
   * Finite positive truncation range multiplier L (default: 10.0).
   * Integration domain is [c1 - L*sqrt(c2 + sqrt(|c4|)), c1 + L*sqrt(c2 + sqrt(|c4|))].
   */
  truncation_l: number;
  [k: string]: unknown;
}
/**
 * Configuration for the deterministic credit factor-model calibrator.
 */
export interface CreditCalibrationConfig {
  /**
   * Optional shrinkage applied to OLS β estimates.
   */
  beta_shrinkage?: BetaShrinkage;
  /**
   * Bucket-mean weighting. Default [`BucketWeighting::Dts`].
   *
   * [`BucketWeighting::Dts`] requires [`super::inputs::CreditCalibrationInputs::spread_durations`].
   */
  bucket_weighting?: BucketWeighting;
  /**
   * Covariance assembly strategy.
   */
  covariance_strategy?: CovarianceStrategy;
  /**
   * Hierarchy specification (broadest → narrowest).
   */
  hierarchy?: CreditHierarchySpec;
  /**
   * Per-level minimum-bucket-size thresholds.
   */
  min_bucket_size_per_level?: BucketSizeThresholds;
  /**
   * Regular observation frequency of the history panel.
   *
   * Derives the annualization used for variance and Ledoit-Wolf
   * (`252` / `12` / `4`). The panel dates must form a complete regular
   * grid of this frequency from `dates[0]`.
   */
  panel_frequency?: PanelFrequency;
  /**
   * Issuer-beta classification policy.
   */
  policy?: IssuerBetaPolicy;
  /**
   * Whether to differentiate the panel before peeling.
   */
  use_returns_or_levels?: PanelSpace;
  /**
   * Vol-model choice for the per-factor variance forecast (sample or EWMA).
   */
  vol_model?: VolModelChoice;
}
/**
 * Ordered list of hierarchy dimensions, broadest → narrowest.
 *
 * The ordering is significant: factor IDs and beta vectors are indexed
 * positionally from level 0 (broadest) to `levels.len()-1` (narrowest).
 */
export interface CreditHierarchySpec {
  /**
   * Ordered hierarchy levels, broadest first.
   */
  levels: HierarchyDimension[];
}
/**
 * Structured diagnostics attached to every calibrated artifact.
 *
 * Consumers can programmatically check coverage (e.g. "≥ 95 % of buckets
 * had ≥ 5 issuers") without parsing free-form log messages.
 *
 * This struct omits `#[serde(deny_unknown_fields)]` to allow additive
 * diagnostic fields in future calibration versions.
 */
export interface CreditCalibrationDiagnostics {
  /**
   * One entry per hierarchy level: `BTreeMap<bucket_path, member_count>`.
   * Counts every issuer assigned to the bucket regardless of
   * [`IssuerBetaMode`]; the same full-membership count gates fold-up.
   */
  bucket_sizes_per_level: {
    [k: string]: number;
  }[];
  /**
   * Log of all fold-up events triggered by insufficient bucket coverage.
   *
   * **Load-bearing, not purely diagnostic:**
   * [`decompose_levels`][crate::factor::credit::decomposition::decompose_levels]
   * reads these records to reconstruct which `(issuer, level)` pairs were
   * folded during calibration. Stripping or editing them changes
   * decomposition results.
   */
  fold_ups: FoldUpRecord[];
  /**
   * Count of resolved [`IssuerBetaMode`] values.
   *
   * Keys are `"issuer_beta"` and `"bucket_only"`.
   */
  mode_counts: {
    [k: string]: number;
  };
  /**
   * Optional histogram of per-issuer R² values (bucketed as string ranges).
   */
  r_squared_histogram?: {
    [k: string]: number;
  } | null;
  /**
   * Canonical tag taxonomy observed during calibration.
   *
   * Keys are dimension names (e.g. `"rating"`, `"region"`, `"sector"`);
   * values are the set of distinct observed tag values.
   */
  tag_taxonomy: {
    [k: string]: string[];
  };
  [k: string]: unknown;
}
/**
 * Record of a single fold-up event during calibration.
 *
 * Fold-up means **omit the sparse child factor** and set `β_k = 0` at that
 * level. The issuer already sits in the parent bucket, so its residual
 * continues to contribute to the parent mean. This is not a re-tagging of
 * the issuer into a different leaf.
 *
 * # Where the folded risk lands
 *
 * No factor is re-assigned: the variation the sparse bucket's factor would
 * have carried stays in each member's residual and flows into the
 * per-issuer **adder** (idiosyncratic) vol. Members of the same folded
 * bucket therefore share a common component that Σ represents as
 * *uncorrelated* idiosyncratic risk — holding several names from one folded
 * bucket overstates diversification for that component. Lower the level's
 * [`BucketSizeThresholds`][crate::factor::credit::calibration::BucketSizeThresholds]
 * entry if that correlation materially matters for a thin bucket.
 */
export interface FoldUpRecord {
  /**
   * Deepest surviving ancestor bucket path (e.g. `"IG.EU"`), recorded for
   * diagnostics only. The member's risk is **not** re-assigned to this
   * bucket's factor — the parent level was already peeled at `k − 1`; the
   * sparse bucket's own variation moves into the issuer adder instead.
   */
  folded_to: string;
  /**
   * Issuer that was folded up.
   */
  issuer_id: Id;
  /**
   * Hierarchy level at which the fold-up occurred.
   */
  level_index: number;
  /**
   * Bucket path before the fold-up (e.g. `"IG.EU.FIN"`).
   */
  original_bucket: string;
  /**
   * Human-readable reason for the fold-up (e.g. `"fewer than 5 issuers"`).
   */
  reason: string;
}
/**
 * Typed issuer histories, tags, generic factor series, anchor date, and overrides for one credit calibration run.
 */
export interface CreditCalibrationInputs {
  /**
   * Calibration anchor date (must appear in `history_panel.dates`).
   */
  as_of: DateWire;
  /**
   * Issuer spreads at `as_of` in decimal units (level space).
   */
  as_of_spreads: {
    [k: string]: number;
  };
  /**
   * Generic factor series + spec.
   */
  generic_factor: GenericFactorSeries;
  /**
   * Complete regular issuer-spread history in decimal units.
   */
  history_panel: HistoryPanel;
  /**
   * Optional annualized idiosyncratic volatility overrides, in decimal
   * spread per square-root year (`0.001` means 10 bp per square-root year).
   *
   * Caller-supplied values take precedence over history, peer-proxy, and
   * global-default adder-vol estimates. Values must be finite and
   * non-negative; calibration converts them to the model's bp units.
   */
  idiosyncratic_overrides: {
    [k: string]: number;
  };
  /**
   * Per-issuer hierarchy tags (point-in-time).
   */
  issuer_tags: IssuerTagPanel;
  /**
   * Option-adjusted spread duration in **years** (`> 0`) per issuer.
   *
   * Required when
   * [`CreditCalibrationConfig::bucket_weighting`][super::config::CreditCalibrationConfig::bucket_weighting]
   * is [`BucketWeighting::Dts`][super::config::BucketWeighting::Dts].
   * The historical peel weights each date by contemporaneous DTS
   * (`SD × panel spread_bp` at that date); the anchor uses as-of DTS.
   * Persisted on each
   * [`IssuerBetaRow`][crate::factor::credit::hierarchy::IssuerBetaRow] so
   * decompose can rebuild DTS from the current spread. The duration is a
   * single value across the calibration window (no per-date duration
   * series).
   */
  spread_durations?: {
    [k: string]: number;
  };
}
/**
 * Generic (PC) factor reference and aligned values.
 */
export interface GenericFactorSeries {
  /**
   * Reference (name + series_id) embedded into the artifact.
   */
  spec: GenericFactorSpec;
  /**
   * Generic factor values aligned with [`HistoryPanel::dates`].
   *
   * Decimal units (`0.01` = 100 bp), same convention as issuer spreads.
   */
  values: number[];
}
/**
 * Reference to the generic (PC) time series used as the first factor.
 *
 * Values are not stored here; they live in
 * [`FactorHistories`] under the key `"credit::generic"`.
 */
export interface GenericFactorSpec {
  /**
   * Human-readable name for the generic factor (e.g. `"CDX IG 5Y"`).
   */
  name: string;
  /**
   * Caller's time-series identifier, used to look up the input data.
   */
  series_id: string;
}
/**
 * Issuer-spread history aligned to a complete regular date grid.
 *
 * `dates` is the sorted observation grid. `spreads[issuer]` has length
 * `dates.len()`. Every entry must be `Some(decimal_spread)` — gaps and
 * `None` are rejected at calibration. Callers pass **decimal** spreads
 * (`0.01` = 100 bp).
 *
 * Every spread must lie in the open decimal band `(-0.5, 2.0)` — i.e.
 * below 20,000 bp. Deeply distressed quotes at or above 200% running-spread
 * equivalents are rejected as looking like basis points; such names must be
 * excluded from the calibration universe.
 */
export interface HistoryPanel {
  /**
   * Observation dates (sorted ascending).
   */
  dates: DateWire[];
  /**
   * Per-issuer decimal spread series aligned with [`dates`][Self::dates].
   *
   * Each vector must be fully observed (`Some` at every date). Values are
   * decimal (`0.01` = 100 bp), converted to bp at calibrate entry.
   */
  spreads: {
    [k: string]: (number | null)[];
  };
}
/**
 * Point-in-time issuer tags at the calibration `as_of`.
 */
export interface IssuerTagPanel {
  /**
   * Tag map keyed by issuer.
   */
  tags: {
    [k: string]: IssuerTags;
  };
}
/**
 * Flat key-value taxonomy tags for an issuer.
 *
 * Uses `BTreeMap` so that serialization is deterministic and two artifacts
 * built from identical inputs produce byte-identical JSON.
 */
export interface IssuerTags {
  [k: string]: string;
}
/**
 * Credit Conversion Factor (CCF) for off-balance-sheet exposures.
 *
 * Represents the fraction of undrawn commitments expected to be drawn
 * at the time of default.
 */
export interface CreditConversionFactor {
  /**
   * CCF value in \[0, 1\]. Basel IRB: typically 0.75 for revolvers.
   */
  ccf: number;
  [k: string]: unknown;
}
/**
 * One name in a finite credit portfolio.
 */
export interface CreditExposure {
  /**
   * Unconditional default probability in `[0, 1]`.
   */
  default_probability: number;
  /**
   * Systematic-factor loadings `β`; the squared norm must not exceed one.
   */
  factor_loadings: number[];
  /**
   * Stable exposure identifier.
   */
  id: string;
  /**
   * Loss given default in `[0, 1]`.
   */
  lgd: number;
  /**
   * Non-negative exposure notional in one caller-defined unit.
   */
  notional: number;
  [k: string]: unknown;
}
/**
 * Fully self-contained credit factor hierarchy model artifact.
 */
export interface CreditFactorModel {
  /**
   * Factor level values at the calibration anchor date.
   */
  anchor_state: LevelsAtAnchor;
  /**
   * Calibration anchor date (`as_of`).
   */
  as_of: DateWire;
  /**
   * Bucket-mean weighting used at calibration and required at decompose.
   *
   * [`BucketWeighting::Equal`] artifacts must not be DTS-weighted at
   * decompose; [`BucketWeighting::Dts`] artifacts rebuild weights from
   * persisted [`IssuerBetaRow::spread_duration`] × current spread (bp).
   */
  bucket_weighting: BucketWeighting;
  /**
   * History window consumed by calibration.
   */
  calibration_window: DateRange;
  /**
   * Existing factor-model config (factors, covariance, matching).
   */
  config: FactorModelConfig;
  /**
   * Structured calibration diagnostics for programmatic coverage checks.
   */
  diagnostics: CreditCalibrationDiagnostics;
  /**
   * Embedded factor histories (recommended for self-contained artifacts).
   *
   * `None` indicates an externally-referenced history store.
   */
  factor_histories?: FactorHistories | null;
  /**
   * Reference to the generic PC factor series.
   */
  generic_factor: GenericFactorSpec;
  /**
   * Ordered hierarchy specification (broadest → narrowest).
   */
  hierarchy: CreditHierarchySpec;
  /**
   * Per-issuer beta rows, sorted by `issuer_id` for wire stability.
   */
  issuer_betas: IssuerBetaRow[];
  /**
   * Regular observation frequency used to annualize variance (`252`/`12`/`4`).
   */
  panel_frequency: PanelFrequency;
  /**
   * Beta regression policy used during calibration.
   */
  policy: IssuerBetaPolicy;
  /**
   * Exact namespaced v1 schema marker.
   */
  schema: CreditFactorModelSchema;
  /**
   * Static factor correlation matrix `ρ` for `Σ(t) = D(t)·ρ·D(t)`.
   *
   * **Which matrix is authoritative:** vol forecasting rebuilds
   * `Σ(t, h) = D·ρ·D` from this matrix plus `vol_state`; point-in-time
   * risk uses `config.covariance` directly. Under
   * [`CovarianceStrategy::Ridge`][crate::factor::credit::calibration::CovarianceStrategy::Ridge]
   * the two deliberately differ —
   * `config.covariance = D·ρ·D + α·I`, so its implied correlations are
   * shrunk relative to `ρ` by `σᵢσⱼ/√((σᵢ²+α)(σⱼ²+α))`.
   *
   * Under
   * [`CovarianceStrategy::LedoitWolf`][crate::factor::credit::calibration::CovarianceStrategy::LedoitWolf]
   * the divergence is larger still, and affects both the diagonal and the
   * off-diagonal: `config.covariance` is the shrinkage estimator's own
   * `periods_per_year · (δ*·μ·I + (1 − δ*)·S)`, computed once over the
   * complete-case rows (dates where every factor is observed), and is
   * authoritative for point-in-time risk. The rebuilt `D·ρ·D` instead
   * combines this same `ρ` with `vol_state` variances — which are
   * estimated per-factor over all available observations (not just the
   * complete-case subset) via whichever
   * [`VolModelChoice`][crate::factor::credit::calibration::VolModelChoice] was
   * configured (`Sample` or `Ewma`). Because the diagonals come from two
   * different estimators over two different observation sets, `D·ρ·D`
   * deliberately differs from `config.covariance` on **both** the
   * diagonal and the off-diagonal; treat it as an approximation for
   * horizon scaling, not as a substitute for `config.covariance`.
   */
  static_correlation: FactorCorrelationMatrix;
  /**
   * Whether calibration peeled a return panel or a raw level panel.
   */
  use_returns_or_levels: PanelSpace;
  /**
   * EWMA or sample vol state at the anchor date.
   */
  vol_state: VolState;
}
/**
 * Snapshot of all factor levels at the calibration anchor date.
 *
 * Used as the carry term in attribution: `L(t) = L_anchor + ΔL(t)`.
 */
export interface LevelsAtAnchor {
  /**
   * Per-level anchor values in hierarchy spec order.
   */
  by_level: LevelAnchor[];
  /**
   * Value of the generic PC factor at `as_of`.
   */
  pc: number;
}
/**
 * Factor level values for a single hierarchy level at the calibration anchor date.
 */
export interface LevelAnchor {
  /**
   * Dimension identifier for this level.
   */
  dimension: HierarchyDimension;
  /**
   * Zero-based index of this level in [`CreditHierarchySpec::levels`].
   */
  level_index: number;
  /**
   * Factor level values keyed by dotted bucket path (e.g. `"IG.EU.FIN"`).
   *
   * `BTreeMap` for deterministic serialization order.
   */
  values: {
    [k: string]: number;
  };
}
/**
 * A closed calendar-date interval `[start, end]`.
 *
 * Used to record the history window consumed by calibration.
 */
export interface DateRange {
  /**
   * Last date of the window (inclusive).
   */
  end: DateWire;
  /**
   * First date of the window (inclusive).
   */
  start: DateWire;
}
/**
 * Serializable configuration bundle for constructing a factor-model workflow.
 *
 * The `factors` vector defines the canonical factor ordering. The covariance
 * matrix must use the same factor IDs and ordering, and the matching
 * configuration is expected to emit exposures against that same universe.
 */
export interface FactorModelConfig {
  /**
   * Optional finite-difference bump overrides for sensitivity engines;
   * `None` uses [`BumpSizeConfig::default`] (1 bp rates/credit, 1 %
   * equity/FX, one vol point).
   */
  bump_config?: BumpSizeConfig | null;
  /**
   * Covariance matrix aligned to `factors`.
   */
  covariance: FactorCovarianceMatrix;
  /**
   * Factor definitions spanning the model universe.
   */
  factors: FactorDefinition[];
  /**
   * Declarative dependency-to-factor matching configuration.
   */
  matching: MatchingConfig;
  /**
   * Sensitivity extraction strategy used by the analysis pipeline.
   */
  pricing_mode: PricingMode;
  /**
   * Risk measure used when aggregating factor sensitivities.
   */
  risk_measure?: RiskMeasure;
  /**
   * Policy used when a dependency does not map to a configured factor.
   */
  unmatched_policy?: UnmatchedPolicy | null;
}
/**
 * User-supplied factor covariance matrix with row-major storage.
 *
 * The factor ID order is part of the contract: row `i`, column `j`
 * corresponds to `factor_ids[i]` and `factor_ids[j]`.
 *
 * # Units contract
 *
 * Entries are **annualized (co)variances of factor moves expressed in each
 * factor's canonical bump unit** — the same unit the sensitivity engines use
 * for deltas (see [`crate::factor::BumpSizeConfig`] / [`crate::factor::FactorType::bump_units`]):
 * basis points for rates/credit/inflation, percent for equity/commodity/FX,
 * vol points for volatility. With sensitivities `s` in P&L-per-canonical-unit
 * and `Σ` in canonical-unit², `sᵀΣs` is directly an annual P&L variance.
 * Mixing conventions — e.g. a covariance in decimal² (`0.0001²` per bp²)
 * against per-bp sensitivities — mis-scales portfolio variance by `1e8`.
 * The credit calibrator produces matrices in this convention from the spread
 * panel's native units; hand-built matrices must match it.
 */
export interface FactorCovarianceMatrix {
  /**
   * Annualized (co)variances in row-major order, `n * n` entries, each in
   * its factors' canonical bump units squared. The matrix is symmetric, so
   * `data[i * n + j]` equals `data[j * n + i]`.
   */
  data: number[];
  /**
   * Factor identifiers, in the row and column order of `data`.
   */
  factor_ids: FactorId[];
  /**
   * Matrix dimension. Equals `factor_ids.len()`.
   */
  n: number;
}
/**
 * Complete definition of a risk factor.
 */
export interface FactorDefinition {
  /**
   * Optional free-form description.
   */
  description?: string | null;
  /**
   * Broad factor classification.
   */
  factor_type: FactorType;
  /**
   * Unique factor identifier.
   */
  id: FactorId;
  /**
   * Mapping from factor move to market-data perturbation.
   */
  market_mapping: MarketMapping;
}
/**
 * A single matching rule from dependency and attribute filters to a factor.
 */
export interface MappingRule {
  /**
   * Instrument metadata filter.
   */
  attribute_filter: AttributeFilter;
  /**
   * Dependency-side filter.
   */
  dependency_filter: DependencyFilter;
  /**
   * Factor assigned when both filters match.
   */
  factor_id: FactorId;
}
/**
 * Filters on an individual market dependency.
 */
export interface DependencyFilter {
  /**
   * Specific curve role that the dependency must match, when present.
   *
   * This is only evaluated for curve-like dependencies.
   */
  curve_type?: CurveType | null;
  /**
   * Dependency classification that the dependency must match, when present.
   */
  dependency_type?: DependencyType | null;
  /**
   * Exact dependency identifier that must match, when present.
   *
   * FX pairs use the canonical `BASE/QUOTE` form, for example `USD/EUR`.
   */
  id?: string | null;
}
/**
 * Declarative configuration for a dependency-scoped hierarchical matcher.
 */
export interface HierarchicalConfig {
  /**
   * Dependency filter applied before any tree traversal.
   */
  dependency_filter?: DependencyFilter;
  /**
   * Root of the attribute classification tree.
   */
  root: FactorNode;
}
/**
 * A node in a hierarchical factor classification tree.
 */
export interface FactorNode {
  /**
   * Child nodes representing more specific classifications.
   */
  children?: FactorNode[];
  /**
   * Factor assigned at this node when it is a valid classification level.
   */
  factor_id?: FactorId | null;
  /**
   * Filter that must match for this node to participate in traversal.
   */
  filter: AttributeFilter;
}
/**
 * Declarative configuration for a calibrated credit-hierarchy matcher.
 *
 * The matcher emits PC + per-level credit factors with calibrated betas
 * looked up from `issuer_betas`. `issuer_betas` must be sorted by
 * `issuer_id` (binary search is used). `hierarchy` defines the level
 * ordering and dimension keys used to build factor IDs.
 */
export interface CreditHierarchicalConfig {
  /**
   * Dependency filter; defaults to "any credit-curve dependency".
   */
  dependency_filter?: DependencyFilter;
  /**
   * Hierarchy specification (level ordering and dimension keys).
   */
  hierarchy: CreditHierarchySpec;
  /**
   * Issuer beta rows, sorted by `issuer_id`.
   */
  issuer_betas?: IssuerBetaRow[];
  /**
   * Require the [`ISSUER_ID_META_KEY`] meta key on every credit dependency.
   *
   * When `true`, a credit dependency whose attributes omit the issuer id
   * is rejected with [`FactorMatchError::MissingRequiredTag`] instead of
   * being silently downgraded to the PC-only proxy — an absent key is
   * usually a data-plumbing failure, and the proxy fallback drops both
   * hierarchy exposure and idiosyncratic risk. Calibrated artifacts set
   * this to `true`; hand-built configs default to `false` (`serde`
   * default) so index-proxy workflows without issuer identities keep
   * working.
   */
  require_issuer_id?: boolean;
}
/**
 * Per-issuer beta row in the calibrated artifact.
 *
 * Rows are stored sorted by `issuer_id` for wire stability: two calibrations
 * on identical inputs serialize to byte-identical JSON regardless of
 * iteration order inside the calibration loop.
 */
export interface IssuerBetaRow {
  /**
   * Value of the issuer's idiosyncratic adder at `as_of` (carry component).
   */
  adder_at_anchor: number;
  /**
   * Annualized idiosyncratic adder volatility (for vol forecasting).
   */
  adder_vol_annualized: number;
  /**
   * Provenance of `adder_vol_annualized`.
   */
  adder_vol_source: AdderVolSource;
  /**
   * Factor beta loadings (all `1.0` for `BucketOnly` issuers).
   *
   * For `IssuerBeta` mode, each level loading is the with-intercept OLS
   * slope of the issuer residual on the **leave-one-out** bucket mean.
   * Peel and stored factor histories use the **full-bucket** mean, so
   * the level identity `S_i = β g + Σ β_k L_k + adder` has no drift term.
   */
  betas: IssuerBetas;
  /**
   * PC-regression fit statistics; `None` when `mode == BucketOnly`.
   */
  fit_quality?: FitQuality | null;
  /**
   * Unique issuer identifier (e.g. LEI or internal code).
   */
  issuer_id: Id;
  /**
   * Per-level regression fit statistics, aligned with `betas.levels`.
   *
   * `Some` where a per-level OLS fit ran (`IssuerBeta` mode, level not
   * folded, regressor not degenerate); `None` otherwise. Empty for
   * `BucketOnly` rows.
   */
  level_fit_quality: (FitQuality | null)[];
  /**
   * Resolved regression mode for this issuer.
   */
  mode: IssuerBetaMode;
  /**
   * Option-adjusted spread duration in **years** used for DTS weights.
   *
   * Calibration persists the caller-supplied duration. Decompose rebuilds
   * `DTS = spread_duration × current_spread_bp` when the artifact was
   * calibrated with [`BucketWeighting::Dts`].
   * Equal-weighted artifacts still store the supplied duration (or `1.0`
   * when none was given); it is not used at peel time.
   */
  spread_duration: number;
  /**
   * Taxonomy tags used to assign the issuer to hierarchy buckets.
   */
  tags: IssuerTags;
}
/**
 * Factor beta loadings for a single issuer.
 *
 * `pc` is the loading on the generic (PC) factor.
 * `levels[i]` is the loading on the bucket factor at hierarchy level `i`.
 *
 * For `BucketOnly` issuers every component is `1.0` by convention.
 *
 * # The `0.0` level-beta sentinel
 *
 * `levels[i] == 0.0` marks a level that was **folded** during calibration
 * (the issuer's bucket was below the size threshold). The matcher and
 * `enumerate_factor_ids` skip such levels. A *fitted* beta of exactly `0.0`
 * is indistinguishable from the sentinel, and that is deliberate: every
 * consumer scales by the beta (exposure `= β·CS01`, stress shift `= β·shock`,
 * attribution `= β·ΔL`), so skipping the level and emitting a zero-beta
 * entry produce identical numbers. The degenerate-regressor guard in
 * calibration additionally maps near-zero-information fits to the unit-beta
 * fallback rather than to `0.0`.
 */
export interface IssuerBetas {
  /**
   * Betas on each hierarchy-level factor, in spec order.
   */
  levels: number[];
  /**
   * Beta on the generic credit PC factor.
   */
  pc: number;
}
/**
 * Regression quality statistics for a single issuer.
 *
 * Only present for `IssuerBeta` mode; `None` for `BucketOnly`.
 */
export interface FitQuality {
  /**
   * Number of monthly observations used in the regression.
   */
  n_obs: number;
  /**
   * In-sample coefficient of determination (R²).
   */
  r_squared: number;
  /**
   * Residual standard deviation of the through-origin peel residual
   * `y − β x` in basis points of spread move.
   */
  residual_std: number;
}
/**
 * Embedded time-series of factor **moves in bp**.
 *
 * These are the official series for rebuilding vol/correlation and for
 * historical-simulation factor P&L. Every date is a real observation
 * (no `None → 0.0` holes). Under [`PanelSpace::Returns`]
 * the stored values are already period moves; under
 * [`PanelSpace::Levels`]
 * they are peeled levels and must be first-differenced before vol or P&L.
 *
 * `BTreeMap<FactorId, Vec<f64>>` for deterministic serialization. All value
 * vectors must have the same length as `dates`.
 */
export interface FactorHistories {
  /**
   * Ordered sequence of observation dates (aligned with value vectors).
   */
  dates: DateWire[];
  /**
   * Factor return series keyed by factor ID.
   *
   * Each vector must have `dates.len()` entries.
   */
  values: {
    [k: string]: number[];
  };
  [k: string]: unknown;
}
/**
 * Static factor correlation matrix `ρ` for the covariance decomposition
 * `Σ(t) = D(t) · ρ · D(t)` where `D(t)` is the diagonal vol matrix.
 *
 * `factor_ids` defines the row/column ordering; `data[i][j]` is
 * `ρ_{factor_ids[i], factor_ids[j]}`. The matrix must be square, symmetric,
 * and have unit diagonal.
 */
export interface FactorCorrelationMatrix {
  /**
   * Row-major correlation data. `data[i]` is row `i`.
   */
  data: number[][];
  /**
   * Factor IDs in row/column order.
   */
  factor_ids: FactorId[];
}
/**
 * Complete vol state for all factors and all issuers at the calibration date.
 *
 * Feeds `Σ(t) = D(t) · ρ · D(t)` and per-issuer idiosyncratic vol forecasts.
 */
export interface VolState {
  /**
   * EWMA or sample vol model for each systematic factor.
   *
   * Keys are factor IDs from [`crate::factor::FactorModelConfig`].
   * `BTreeMap` for deterministic serialization order.
   */
  factors: {
    [k: string]: FactorVolModel;
  };
  /**
   * Idiosyncratic vol model for each issuer.
   *
   * `BTreeMap` for deterministic serialization order.
   */
  idiosyncratic: {
    [k: string]: IdiosyncraticVolModel;
  };
  [k: string]: unknown;
}
/**
 * Observable credit state at a point in time.
 */
export interface CreditState {
  /**
   * Accreted (PIK-augmented) notional outstanding.
   */
  accreted_notional: number;
  /**
   * Fair value of the firm's assets, if available.
   */
  asset_value?: number | null;
  /**
   * Cash coupon amount due at this decision date.
   */
  coupon_due?: number;
  /**
   * Distance-to-default (number of standard deviations from the default point).
   */
  distance_to_default?: number | null;
  /**
   * Current hazard rate (annualised instantaneous default intensity).
   */
  hazard_rate: number;
  /**
   * Leverage ratio (debt / assets).
   */
  leverage: number;
  [k: string]: unknown;
}
/**
 * Configuration for position-level VaR decomposition.
 */
export interface DecompositionConfig {
  /**
   * Whether to compute incremental VaR (expensive: one full repricing
   * per position).
   */
  compute_incremental: boolean;
  /**
   * Confidence level for VaR and ES (e.g. 0.95, 0.99).
   */
  confidence: number;
  /**
   * Decomposition method.
   */
  method: DecompositionMethod;
}
/**
 * Default model specification.
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
 * Diebold-Li (2006) dynamic Nelson-Siegel model.
 */
export interface DieboldLi {
  /**
   * Extracted factor time series.
   */
  factors?: FactorTimeSeries | null;
  /**
   * Decay parameter lambda.
   */
  lambda: number;
  /**
   * VAR(1) intercept vector mu.
   *
   * @minItems 3
   * @maxItems 3
   */
  mu?: [unknown, unknown, unknown] | null;
  /**
   * VAR(1) coefficient matrix Phi.
   *
   * @minItems 3
   * @maxItems 3
   */
  phi?: [unknown, unknown, unknown] | null;
  /**
   * VAR(1) residual covariance Q.
   *
   * @minItems 3
   * @maxItems 3
   */
  q_cov?: [unknown, unknown, unknown] | null;
  /**
   * Tenor grid from the input panel.
   */
  tenors: number[];
}
/**
 * Time series of extracted Nelson-Siegel factors.
 */
export interface FactorTimeSeries {
  /**
   * Observation dates copied from the source [`YieldPanel`] (length T)
   * when the panel carried them; `None` for unlabeled panels.
   */
  dates?: DateWire[] | null;
  /**
   * Factor matrix: T rows x 3 columns [beta1, beta2, beta3].
   * beta1 = level, beta2 = slope, beta3 = curvature.
   *
   * @minItems 3
   * @maxItems 3
   */
  factors: [unknown, unknown, unknown];
  /**
   * R-squared per tenor (length N).
   */
  r_squared: number[];
  /**
   * Overall cross-sectional R-squared (average across tenors).
   */
  r_squared_avg: number;
  /**
   * Residuals from OLS factor extraction: T x N.
   *
   * @minItems 3
   * @maxItems 3
   */
  residuals: [unknown, unknown, unknown];
  [k: string]: unknown;
}
/**
 * Downturn LGD adjuster.
 */
export interface DownturnLgd {
  /**
   * Downturn adjustment method.
   */
  method: DownturnMethod;
}
/**
 * Specification for dynamic (notional-dependent) recovery rate.
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
 * Exposure at Default calculator.
 */
export interface EadCalculator {
  /**
   * Credit conversion factor for the undrawn portion.
   */
  ccf: CreditConversionFactor;
  /**
   * Currently drawn amount.
   */
  drawn: number;
  /**
   * Undrawn (available) commitment.
   */
  undrawn: number;
  [k: string]: unknown;
}
/**
 * Specification for endogenous (leverage-dependent) hazard rate.
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
 * Numeric Monte Carlo estimate for discounted path values.
 */
export interface Estimate {
  /**
   * 95% confidence interval for the discounted mean.
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
   * Mean of the discounted path values.
   */
  mean: number;
  /**
   * Optional median of captured discounted path values.
   */
  median?: number | null;
  /**
   * Optional minimum of captured discounted path values.
   */
  min?: number | null;
  /**
   * Number of independent path estimators contributing to the estimate.
   *
   * With variance reduction disabled (`antithetic = false`), this equals the
   * number of simulated sample paths. With antithetic variates enabled each
   * estimator is the mean of an antithetic pair, so
   * `num_simulated_paths == 2 * num_paths`. The engine records both so
   * downstream consumers can distinguish statistical sample size from
   * simulation work.
   */
  num_paths: number;
  /**
   * Total number of simulated sample paths driving the estimator.
   *
   * Equal to `num_paths` without variance reduction. When
   * [`crate::monte_carlo::engine::McEngineConfig::antithetic`] is enabled this is twice
   * `num_paths` because each estimator averages an antithetic pair of
   * paths.
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
   * Standard error of the discounted mean.
   */
  stderr: number;
}
/**
 * Hold-versus-tender economics of a distressed exchange offer.
 */
export interface ExchangeOfferAnalysis {
  /**
   * Hold-out recovery, as a fraction of the existing claim's present value,
   * at which tendering and holding out break even. Capped at `1.0`.
   */
  breakeven_recovery: number;
  /**
   * Cash consent or early-tender fee.
   */
  consent_fee: number;
  /**
   * Tender consideration less the hold-out present value.
   */
  delta_npv: number;
  /**
   * Estimated value of attached equity or warrants.
   */
  equity_sweetener_value: number;
  /**
   * Structure of the offer, echoed back in canonical form.
   */
  exchange_type: ExchangeType;
  /**
   * Present value of the new instrument received on tendering.
   */
  new_npv: number;
  /**
   * Present value of the existing claim if the holder does not tender.
   */
  old_npv: number;
  /**
   * Whether the tender consideration clears the
   * [`TENDER_RECOMMENDATION_HURDLE`] cushion over the hold-out value.
   */
  tender_recommended: boolean;
  /**
   * Total tender consideration: new instrument plus fee plus sweetener.
   */
  tender_total: number;
  [k: string]: unknown;
}
/**
 * Optimal execution schedule for a trade.
 */
export interface ExecutionTrajectory {
  /**
   * Variance of the cost under the optimal trajectory.
   */
  cost_variance: number;
  /**
   * Expected cost of the optimal trajectory.
   */
  expected_cost: number;
  /**
   * Quantity to trade in each time bucket.
   */
  quantities: number[];
  /**
   * Remaining position after each bucket.
   */
  remaining: number[];
  /**
   * Time points (in trading days) for each bucket boundary.
   */
  time_points: number[];
  [k: string]: unknown;
}
/**
 * Contribution of a single factor to portfolio risk.
 *
 * See [`RiskDecomposition`] for the sign convention applied to each field.
 */
export interface FactorContribution {
  /**
   * Absolute contribution of the factor to the chosen risk measure.
   * Sign follows the measure's convention.
   */
  absolute_risk: number;
  /**
   * Identifier of the factor being reported.
   */
  factor_id: FactorId;
  /**
   * Marginal sensitivity of portfolio risk to the factor. Same sign
   * convention as `absolute_risk`.
   */
  marginal_risk: number;
  /**
   * Contribution expressed as a share of total portfolio risk. Dimensionless,
   * non-negative for standard long-risk portfolios (signs of numerator and
   * denominator cancel).
   */
  relative_risk: number;
  [k: string]: unknown;
}
/**
 * Versioned factor-model configuration with typed factors, covariance, matching, pricing, and risk settings.
 */
export interface FactorModelConfigEnvelope {
  /**
   * Bare configuration used by in-process analysis APIs.
   */
  config: FactorModelConfig;
  /**
   * Exact factor-model configuration contract marker.
   */
  schema: FactorModelConfigSchema;
}
/**
 * Undiscounted forward Greeks of a European option on a forward.
 */
export interface ForwardGreeks {
  /**
   * Sensitivity of the undiscounted premium to the forward (`dV/dF`).
   */
  delta: number;
  /**
   * Second derivative of the undiscounted premium to the forward (`d2V/dF2`).
   */
  gamma: number;
  /**
   * Sensitivity of the undiscounted premium to a unit (1.0) change in vol.
   */
  vega: number;
}
/**
 * Geometric Brownian Motion parameters.
 */
export interface GbmParams {
  /**
   * Dividend/foreign rate (annual)
   */
  q: number;
  /**
   * Risk-free rate (annual)
   */
  r: number;
  /**
   * Volatility (annual)
   */
  sigma: number;
  [k: string]: unknown;
}
/**
 * Compact captured GBM paths for plotting and diagnostics.
 */
export interface GbmPathSummary {
  /**
   * Number of independent estimators requested.
   */
  num_paths: number;
  /**
   * Total number of sample paths simulated.
   */
  num_simulated_paths: number;
  /**
   * Captured spot paths in deterministic path-id order.
   */
  paths: number[][];
  /**
   * Shared path times in year fractions, including time zero.
   */
  times: number[];
  [k: string]: unknown;
}
/**
 * Continuous-time generator (intensity) matrix for a CTMC.
 *
 * Off-diagonal entry `q_ij` (i ≠ j) is the instantaneous rate of transitioning
 * from state i to state j. Diagonal entry `q_ii = -Σ_{j≠i} q_ij` so rows sum
 * to zero.
 *
 * # Validation
 *
 * - Off-diagonal entries ≥ 0
 * - Diagonal entries ≤ 0
 * - Each row sums to 0 (tolerance: 1e-8)
 * - If a default state is set, its row must be zero (absorbing)
 *
 * # References
 *
 * - Lando, D., & Skodeberg, T. M. (2002). "Analyzing Rating Transitions and
 *   Rating Drift with Continuous Observations." *Journal of Banking & Finance*,
 *   26(2-3), 423-444. `docs/REFERENCES.md#lando-skodeberg-2002`
 */
export interface GeneratorMatrix {
  /**
   * @minItems 3
   * @maxItems 3
   */
  data: [unknown, unknown, unknown];
  regularization_l1?: number;
  round_trip_error?: number;
  scale: RatingScale;
}
/**
 * An ordered set of states defining a transition matrix's row/column layout.
 *
 * States are identified by string labels for flexibility across rating
 * granularities (coarse, notched, with/without NR, or custom). The scale
 * defines which index is the absorbing default state (if any).
 */
export interface RatingScale {
  default_state?: number | null;
  index_map: {
    [k: string]: number;
  };
  labels: string[];
}
/**
 * Heston stochastic volatility model parameters.
 */
export interface HestonParams {
  /**
   * Mean reversion speed.
   */
  kappa: number;
  /**
   * Spot-variance correlation.
   */
  rho: number;
  /**
   * Vol-of-vol.
   */
  sigma_v: number;
  /**
   * Long-run variance.
   */
  theta: number;
  /**
   * Initial variance.
   */
  v0: number;
}
/**
 * Market inputs for closed-form Heston pricing.
 */
export interface HestonPricingParams {
  /**
   * Mean reversion speed.
   */
  kappa: number;
  /**
   * Continuously compounded dividend or foreign yield as an annual decimal.
   */
  q: number;
  /**
   * Continuously compounded risk-free rate as an annual decimal.
   */
  r: number;
  /**
   * Spot-variance correlation.
   */
  rho: number;
  /**
   * Vol-of-vol.
   */
  sigma_v: number;
  /**
   * Long-run variance.
   */
  theta: number;
  /**
   * Initial variance.
   */
  v0: number;
  [k: string]: unknown;
}
/**
 * Validated Hull-White one-factor process parameters.
 */
export interface HullWhite1FParams {
  kappa: number;
  theta_curve: number[];
  theta_times: number[];
  volatility: PiecewiseConstantCurve;
  [k: string]: unknown;
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
 * Validated constant-parameter Hull-White one-factor model.
 */
export interface HullWhiteCalibrationParams {
  kappa: number;
  sigma: number;
  [k: string]: unknown;
}
/**
 * Hull-White parameters with piecewise-constant short-rate volatility.
 */
export interface HullWhiteParams {
  kappa: number;
  volatility: PiecewiseConstantCurve;
  [k: string]: unknown;
}
/**
 * Estimated market-impact execution *costs* from a trade.
 */
export interface ImpactEstimate {
  /**
   * Cost as basis points of notional value.
   */
  cost_bp: number;
  /**
   * Execution risk (standard deviation of cost).
   */
  execution_risk: number;
  /**
   * Permanent-impact component of the expected execution cost, in
   * currency units (information leakage, irreversible). Despite the
   * name, this is a cost, not a price displacement.
   */
  permanent_impact: number;
  /**
   * Temporary-impact component of the expected execution cost, in
   * currency units (order-flow pressure, mean-reverts). Despite the
   * name, this is a cost, not a price displacement.
   */
  temporary_impact: number;
  /**
   * Total expected execution cost (permanent + temporary), in currency
   * units.
   */
  total_cost: number;
  [k: string]: unknown;
}
/**
 * Kyle (1985) price impact model.
 */
export interface KyleLambdaModel {
  /**
   * Price impact per unit of order flow.
   */
  lambda: number;
  [k: string]: unknown;
}
/**
 * Per-level bucket values produced by a single decomposition.
 *
 * The `values` map is keyed by the dotted bucket path (e.g. `"IG.EU.FIN"`),
 * matching the convention used elsewhere in the credit hierarchy artifact.
 */
export interface LevelValuesAtDate {
  /**
   * Dimension identifier for this level, copied from the hierarchy spec.
   */
  dimension: HierarchyDimension;
  /**
   * Zero-based index of this level inside [`crate::factor::credit::hierarchy::CreditHierarchySpec::levels`].
   */
  level_index: number;
  /**
   * Bucket → mean residual at this level, computed across all issuers
   * in the input spread set whose tags placed them in that bucket.
   */
  values: {
    [k: string]: number;
  };
  [k: string]: unknown;
}
/**
 * Per-level bucket-value deltas produced by [`decompose_period`].
 */
export interface LevelValuesDelta {
  /**
   * Bucket → `(to.values[bucket] - from.values[bucket])`. Only buckets
   * present in **both** snapshots are included.
   */
  deltas: {
    [k: string]: number;
  };
  /**
   * Dimension for this level.
   */
  dimension: HierarchyDimension;
  /**
   * Zero-based level index, mirroring [`LevelValuesAtDate::level_index`].
   */
  level_index: number;
  [k: string]: unknown;
}
/**
 * Snapshot of all hierarchy-level factor values at a single date, produced from observed issuer spreads.
 */
export interface LevelsAtDate {
  /**
   * Per-issuer residual after peeling generic + every level.
   */
  adder: {
    [k: string]: number;
  };
  /**
   * Per-level bucket values, in hierarchy spec order.
   */
  by_level: LevelValuesAtDate[];
  /**
   * Date the spreads were observed.
   */
  date: string;
  /**
   * Generic (PC) factor value at `date`. Equal to the input
   * `observed_generic` and propagated unchanged.
   */
  generic: number;
  [k: string]: unknown;
}
/**
 * Gross-leverage impact of a liability management exercise.
 *
 * Leverage is gross debt over EBITDA, so a value of `8.0` reads as 8.0x.
 * Only debt retired at par reduces leverage; consent fees and collateral
 * transfers leave gross debt unchanged.
 */
export interface LeverageImpact {
  /**
   * Turns of leverage removed: `pre_leverage - post_leverage`.
   */
  leverage_reduction: number;
  /**
   * Gross debt over EBITDA after the exercise, as a multiple.
   */
  post_leverage: number;
  /**
   * Gross debt of the target instrument after the exercise.
   */
  post_total_debt: number;
  /**
   * Gross debt over EBITDA before the exercise, as a multiple.
   */
  pre_leverage: number;
  /**
   * Gross debt of the target instrument before the exercise.
   */
  pre_total_debt: number;
  [k: string]: unknown;
}
/**
 * Configuration for liquidity calculations.
 */
export interface LiquidityConfig {
  /**
   * Days-to-liquidate thresholds for tier boundaries.
   *
   * Array of 4 thresholds: \[tier1_max, tier2_max, tier3_max, tier4_max\].
   * Default: \[1.0, 5.0, 20.0, 60.0\].
   *
   * @minItems 4
   * @maxItems 4
   */
  tier_thresholds: [number, number, number, number];
}
/**
 * Market microstructure data for a single instrument.
 *
 * Users supply this data from their market data systems; the module does not
 * fetch it.
 *
 * # Units
 *
 * - Prices (`mid`, `bid`, `ask`) are in the instrument's native currency.
 * - `avg_daily_volume` is in shares/contracts per day.
 * - `avg_trade_size` is in shares/contracts per trade.
 * - `spread_volatility` is the standard deviation of the relative spread
 *   (spread / mid) over the observation window.
 *
 * # References
 *
 * - Bid-ask spread conventions: `docs/REFERENCES.md#hasbrouck-2007`
 */
export interface LiquidityProfile {
  /**
   * Best ask price.
   */
  ask: number;
  /**
   * Average daily trading volume in shares/contracts.
   */
  avg_daily_volume: number;
  /**
   * Average trade size in shares/contracts.
   */
  avg_trade_size: number;
  /**
   * Best bid price.
   */
  bid: number;
  /**
   * Instrument identifier (must match `Position::instrument_id`).
   */
  instrument_id: string;
  /**
   * Mid-price (average of bid and ask).
   */
  mid: number;
  /**
   * Observation window in trading days for volume/spread statistics.
   *
   * Defaults to 20 (one calendar month). Used to qualify the
   * statistical reliability of ADV and spread estimates.
   */
  observation_days: number;
  /**
   * Standard deviation of the bid-ask spread.
   *
   * Used in the Bangia et al. (1999) LVaR formula. Set to 0.0 if
   * spread volatility data is unavailable (degrades to exogenous LVaR).
   *
   * The interpretation (relative vs. absolute) is controlled by
   * [`Self::spread_volatility_kind`], which defaults to
   * [`SpreadVolatilityKind::Relative`] (the original Bangia convention).
   */
  spread_volatility: number;
  /**
   * Interpretation of [`Self::spread_volatility`]: relative or absolute.
   */
  spread_volatility_kind: SpreadVolatilityKind;
}
/**
 * Issuer-side economics of a liability management exercise.
 */
export interface LmeAnalysis {
  /**
   * Cash paid by the issuer, in the caller's monetary unit.
   */
  cost: number;
  /**
   * Par retired less cash paid — the discount captured by the issuer.
   */
  discount_capture: number;
  /**
   * Discount captured as a fraction of par retired; zero when no par is
   * retired.
   */
  discount_capture_pct: number;
  /**
   * Gross-leverage impact, present only when a positive EBITDA is supplied.
   */
  leverage_impact?: LeverageImpact | null;
  /**
   * Structure of the exercise, echoed back in canonical form.
   */
  lme_type: LmeType;
  /**
   * Face amount retired; zero for structures that do not extinguish debt.
   */
  notional_reduction: number;
  /**
   * Fraction of value diverted away from non-participating holders; nonzero
   * only for a [`LmeType::Dropdown`].
   */
  remaining_holder_impact_pct: number;
  [k: string]: unknown;
}
/**
 * Parameters for the LMM/BGM model.
 */
export interface LmmParams {
  /**
   * Accrual factors τ_i = T_{i+1} − T_i (length N).
   */
  accrual_factors: number[];
  /**
   * Displacement per forward for negative-rate support (length N).
   */
  displacements: number[];
  /**
   * Initial forward rates F_i(0) from the curve (length N).
   */
  initial_forwards: number[];
  /**
   * Number of Brownian factors (2 or 3).
   */
  num_factors: number;
  /**
   * Number of forward rates (N).
   */
  num_forwards: number;
  /**
   * Tenor dates T_0, T_1, ..., T_N (N+1 dates, year fractions).
   */
  tenors: number[];
  /**
   * Piecewise-constant vol breakpoints (ascending, length M).
   */
  vol_times: number[];
  /**
   * Factor loadings λ_{i,k}(t) per forward, per time period.
   *
   * Outer: M+1 time periods.
   * Inner: N forwards, each with up to 3 factor loadings.
   */
  vol_values: [number, number, number][][];
  [k: string]: unknown;
}
/**
 * Scalar Bangia LVaR outputs for an isolated position where relative spread statistics are already known.
 */
export interface LvarBangiaScalar {
  /**
   * Bangia-adjusted LVaR (non-positive loss number, `lvar <= var <= 0`).
   */
  lvar: number;
  /**
   * Ratio `lvar / var`. `NaN` when `var == 0`.
   */
  lvar_ratio: number;
  /**
   * Non-negative magnitude of the Bangia spread-cost add-on.
   */
  spread_cost: number;
  /**
   * Input VaR (non-positive loss number), echoed back for convenience.
   */
  var: number;
  [k: string]: unknown;
}
/**
 * A master scale mapping continuous PDs to discrete rating grades.
 */
export interface MasterScale {
  grades: MasterScaleGrade[];
}
/**
 * A single grade in a master scale.
 */
export interface MasterScaleGrade {
  /**
   * Central (representative) PD for the grade.
   *
   * Typically the geometric mean of the grade's PD range.
   */
  central_pd: number;
  /**
   * Grade label (e.g., "AAA", "Aaa", "1", etc.).
   */
  label: string;
  /**
   * Upper PD boundary for this grade (**inclusive**).
   *
   * A PD <= this value maps to this grade (checked in order), so a PD
   * exactly on the boundary maps to the better grade.
   */
  upper_pd: number;
  [k: string]: unknown;
}
/**
 * Result of mapping a PD to a master scale grade.
 */
export interface MasterScaleResult {
  /**
   * The central PD for the assigned grade.
   */
  central_pd: number;
  /**
   * The assigned rating grade label.
   */
  grade: string;
  /**
   * Index of the grade in the scale (0 = best).
   */
  grade_index: number;
  /**
   * The input PD that was mapped.
   */
  input_pd: number;
  [k: string]: unknown;
}
/**
 * Merton (1976) jump-diffusion characteristic function.
 */
export interface MertonJumpCf {
  /**
   * Jump intensity (expected number of jumps per year).
   */
  lambda: number;
  /**
   * Mean of log-jump size.
   */
  mu_j: number;
  /**
   * Dividend yield.
   */
  q: number;
  /**
   * Risk-free rate.
   */
  r: number;
  /**
   * Diffusion volatility.
   */
  sigma: number;
  /**
   * Standard deviation of log-jump size.
   */
  sigma_j: number;
  [k: string]: unknown;
}
/**
 * Merton structural credit model.
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
 * Simulator for generating rating paths from a generator matrix.
 */
export interface MigrationSimulator {
  generator: GeneratorMatrix;
  horizon: number;
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
 * Parameters for a multi-dimensional Ornstein-Uhlenbeck process.
 */
export interface MultiOuParams {
  /**
   * Optional row-major `n x n` correlation matrix.
   */
  correlation?: number[] | null;
  /**
   * Mean-reversion speeds `κ_i` per year.
   */
  kappas: number[];
  /**
   * Diffusion scales `σ_i` per square root year.
   */
  sigmas: number[];
  /**
   * Long-run means `θ_i` in state units.
   */
  thetas: number[];
  [k: string]: unknown;
}
/**
 * Input for the Ohlson O-Score logistic model (1980).
 */
export interface OhlsonOScoreInput {
  /**
   * Current Liabilities / Current Assets.
   */
  current_liabilities_to_current_assets: number;
  /**
   * Funds from Operations / Total Liabilities.
   */
  funds_from_operations_to_total_liabilities: number;
  /**
   * 1 if Total Liabilities > Total Assets, else 0.
   */
  liabilities_exceed_assets: number;
  /**
   * Ohlson's SIZE variable: `ln(Total Assets / GNP price-level index)`.
   *
   * The published coefficient `-0.407` is calibrated to this specific
   * scale using the US GNP deflator with Ohlson's 1970s base period
   * (Ohlson 1980, Table 4). The deflator normalises for inflation so
   * firms are compared on a constant-dollar basis.
   *
   * # Unit sensitivity (IMPORTANT)
   *
   * Because the coefficient is calibrated to Ohlson's specific scale,
   * substituting a different unit shifts the score by a constant and
   * therefore shifts every implied PD and every zone boundary. For
   * example, `ln(total_assets_millions)` differs from Ohlson's SIZE
   * by `+ ln(10^6 / GNP_deflator_1970s)`, which multiplied by
   * `-0.407` shifts the O-score by roughly `-2.8` - i.e. every firm
   * looks ~60%+ "safer" than it should.
   *
   * Recommended inputs, in order of fidelity:
   * 1. `ln(total_assets_nominal_USD / GNP_deflator)` using a deflator
   *    with an explicit base period close to 1968-70. This reproduces
   *    Ohlson's original scale.
   * 2. `ln(total_assets_deflated_to_Ohlson_base_USD)` if you have
   *    pre-deflated real dollars.
   * 3. Any other rescaling, together with a re-estimated intercept and
   *    re-calibrated zone thresholds on your own sample.
   *
   * Do **not** feed raw `ln(total_assets_millions)` or
   * `ln(total_assets_billions)` unless you have re-estimated the model
   * - the out-of-the-box thresholds will mis-rank.
   */
  log_total_assets_adjusted: number;
  /**
   * 1 if net income was negative for the last two years, else 0.
   */
  negative_net_income_two_years: number;
  /**
   * (NI_t - NI_{t-1}) / (|NI_t| + |NI_{t-1}|) -- change in net income.
   */
  net_income_change: number;
  /**
   * Net Income / Total Assets (ROA).
   */
  net_income_to_total_assets: number;
  /**
   * Total Liabilities / Total Assets.
   */
  total_liabilities_to_total_assets: number;
  /**
   * Working Capital / Total Assets.
   */
  working_capital_to_total_assets: number;
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
 * Serializable Expected Shortfall decomposition view.
 */
export interface ParametricEsDecompositionView {
  /**
   * Confidence level used for ES.
   */
  confidence: number;
  /**
   * Per-position ES contributions.
   */
  contributions: PositionEsContributionView[];
  /**
   * Number of positions in the decomposition.
   */
  n_positions: number;
  /**
   * Total portfolio Expected Shortfall.
   */
  portfolio_es: number;
  /**
   * Total portfolio VaR.
   */
  portfolio_var: number;
  [k: string]: unknown;
}
/**
 * Serializable Expected Shortfall contribution row.
 */
export interface PositionEsContributionView {
  /**
   * Component Expected Shortfall allocated to the position.
   */
  component_es: number;
  /**
   * Marginal Expected Shortfall, when available.
   */
  marginal_es?: number | null;
  /**
   * Fraction of total ES contributed by this position.
   */
  pct_contribution: number;
  /**
   * Position identifier.
   */
  position_id: string;
  [k: string]: unknown;
}
/**
 * Parameters for the Merton-Vasicek single-factor PiT/TtC conversion.
 */
export interface PdCycleParams {
  /**
   * Asset correlation rho in (0, 1).
   *
   * Basel II uses rho in [0.12, 0.24] for corporates.
   */
  asset_correlation: number;
  /**
   * Systematic risk factor (cycle index).
   *
   * - z = 0 corresponds to average conditions (PiT == TtC).
   * - z < 0 corresponds to downturn (stressed PiT > TtC).
   * - z > 0 corresponds to benign conditions (PiT < TtC).
   */
  cycle_index: number;
  [k: string]: unknown;
}
/**
 * Difference between two `LevelsAtDate` snapshots.
 */
export interface PeriodDecomposition {
  /**
   * Per-level bucket deltas. Same length and same `level_index` /
   * `dimension` ordering as [`LevelsAtDate::by_level`].
   */
  by_level: LevelValuesDelta[];
  /**
   * Per-issuer adder deltas (`to - from`), restricted to issuers present
   * in both snapshots.
   */
  d_adder: {
    [k: string]: number;
  };
  /**
   * `to.generic - from.generic`.
   */
  d_generic: number;
  /**
   * Earlier snapshot date.
   */
  from: string;
  /**
   * Later snapshot date.
   */
  to: string;
  [k: string]: unknown;
}
/**
 * Portfolio credit-loss simulation settings.
 */
export interface PortfolioLossConfig {
  /**
   * Loss-positive VaR and expected-shortfall confidence in `(0, 1)`.
   */
  confidence: number;
  /**
   * Gaussian or Student-t copula specification.
   */
  copula: CopulaSpec;
  /**
   * Number of simulated paths in `1..=MAX_PORTFOLIO_LOSS_PATHS`.
   */
  num_paths: number;
  /**
   * Root seed for path-indexed Philox streams.
   */
  seed: number;
  [k: string]: unknown;
}
/**
 * Portfolio credit-loss distribution and tail statistics.
 */
export interface PortfolioLossResult {
  /**
   * Loss-positive confidence used for `var` and `expected_shortfall`, in
   * `(0, 1)`.
   */
  confidence: number;
  /**
   * Arithmetic mean path loss.
   */
  expected_loss: number;
  /**
   * Probability-weighted mean of the worst `1 - confidence` share of losses.
   */
  expected_shortfall: number;
  /**
   * Loss for each path in ascending path-index order.
   */
  losses: number[];
  /**
   * Loss-positive nearest-rank VaR at the configured confidence.
   */
  var: number;
  [k: string]: unknown;
}
/**
 * Budget comparison for a single position.
 */
export interface PositionBudgetEntry {
  /**
   * Actual component VaR from the decomposition, signed as reported by
   * the engine (loss convention: negative for risk consumers when the
   * portfolio VaR is negative).
   */
  actual_component_var: number;
  /**
   * Over/under-budget amount on the consuming side: consuming component
   * VaR minus the target level. Negative when under budget, and always
   * negative for diversifiers.
   */
  excess: number;
  /**
   * Position identifier.
   */
  position_id: string;
  /**
   * Target component VaR level: target fraction times |portfolio VaR|.
   */
  target_component_var: number;
  /**
   * Utilization ratio: consuming-side component VaR over the target level.
   *
   * The component is measured on the consuming side (positive when it has
   * the same sign as portfolio VaR). Values > 1.0 indicate the position
   * uses more risk than budgeted; values in (0, 1) indicate unused
   * budget; **negative values indicate a diversifier** whose component
   * VaR offsets portfolio risk — a diversifier can never breach.
   * `±inf` marks a non-zero component against a zero target.
   */
  utilization: NonFiniteF64Wire;
  [k: string]: unknown;
}
/**
 * Expected Shortfall decomposition result for a single portfolio position.
 */
export interface PositionEsContribution {
  /**
   * Component ES: position's contribution to portfolio Expected Shortfall.
   *
   * Parametric: analytical formula using truncated normal moments.
   * ```text
   * CES_i = w_i * (Sigma * w)_i / sigma_p * phi(z_alpha) / (1 - alpha)
   * ```
   *
   * Historical: average of position-level losses in tail scenarios.
   * ```text
   * CES_i = E[L_i | L_portfolio > VaR_portfolio]
   * ```
   */
  component_es: number;
  /**
   * Marginal ES: per-unit sensitivity of portfolio ES to this position.
   *
   * `None` when the engine cannot produce a true gradient (e.g.
   * historical mode without finite-difference repricing).
   */
  marginal_es?: number | null;
  /**
   * Position identifier.
   */
  position_id: string;
  /**
   * Component ES as a fraction of total portfolio ES.
   */
  relative_es: number;
  [k: string]: unknown;
}
/**
 * Contribution of a single position to a specific factor bucket.
 */
export interface PositionFactorContribution {
  /**
   * Identifier of the contributing factor.
   */
  factor_id: FactorId;
  /**
   * Portfolio position identifier.
   */
  position_id: string;
  /**
   * Risk attributed to this position-factor pair.
   */
  risk_contribution: number;
  [k: string]: unknown;
}
/**
 * Annualized residual variance contributed by a single position.
 *
 * `residual_variance` is reported in variance units (not vol) so that
 * per-position residuals add linearly into a portfolio-level total. Callers
 * that want per-position residual *vol* should take the square root of this
 * field after summing the relevant subset.
 */
export interface PositionResidualContribution {
  /**
   * Portfolio position identifier.
   */
  position_id: string;
  /**
   * Annualized variance contributed by this position's idiosyncratic risk.
   * Always non-negative.
   */
  residual_variance: number;
  /**
   * Where the residual variance came from.
   */
  source: ResidualContributionSource;
  [k: string]: unknown;
}
/**
 * Complete position-level risk decomposition of a portfolio.
 */
export interface PositionRiskDecomposition {
  /**
   * Confidence level used for both VaR and ES.
   */
  confidence: number;
  /**
   * Per-position ES decomposition.
   */
  es_contributions: PositionEsContribution[];
  /**
   * Residual from Euler decomposition (should be near zero).
   *
   * `residual = portfolio_var - sum(component_var_i)`.
   * Only meaningful for the parametric engine, where a non-zero residual
   * signals a numerical issue (ill-conditioned covariance, floating-point
   * accumulation error).
   *
   * `None` in historical mode: the Tasche scaling used there makes the
   * residual algebraically zero by construction, so it carries no
   * diagnostic information.
   */
  euler_residual?: number | null;
  /**
   * Method used for decomposition.
   */
  method: DecompositionMethod;
  /**
   * Number of positions in the portfolio.
   */
  n_positions: number;
  /**
   * Total portfolio Expected Shortfall.
   *
   * Same loss convention as [`Self::portfolio_var`]; ES lies at or
   * beyond VaR in the loss tail (`portfolio_es <= portfolio_var`).
   */
  portfolio_es: number;
  /**
   * Total portfolio VaR.
   *
   * Loss convention (workspace-wide): VaR follows the P&L sign, so
   * losses are reported as **negative** numbers, matching the
   * factor-level engines and `analytics::value_at_risk`.
   */
  portfolio_var: number;
  /**
   * Per-position VaR decomposition.
   */
  var_contributions: PositionVarContribution[];
  [k: string]: unknown;
}
/**
 * Risk decomposition result for a single portfolio position.
 *
 * All monetary fields are in the same units as the portfolio VaR
 * (typically the portfolio's base currency).
 */
export interface PositionVarContribution {
  /**
   * Component VaR: position's Euler-allocated share of portfolio VaR.
   *
   * Sum of all component VaRs equals total portfolio VaR (exact under
   * the parametric normal assumption; approximate for historical).
   *
   * Formula (parametric, loss-signed):
   * ```text
   * CVaR_i = -w_i * (Sigma * w)_i / sigma_p * z_alpha
   * ```
   */
  component_var: number;
  /**
   * Incremental VaR: change in portfolio VaR from removing this position.
   *
   * ```text
   * IVaR_i = VaR(portfolio) - VaR(portfolio \ {i})
   * ```
   *
   * Requires full repricing for each position removal. `None` if
   * incremental VaR was not requested (it is expensive).
   */
  incremental_var?: number | null;
  /**
   * Marginal VaR: per-unit sensitivity of portfolio VaR to this position.
   *
   * Formula (parametric, loss-signed):
   * ```text
   * MVaR_i = -(Sigma * w)_i / sigma_p * z_alpha
   * ```
   *
   * Used as gradient input for mean-variance optimization and
   * risk-budgeting rebalancing.
   *
   * `None` when the engine cannot produce a true gradient (e.g.
   * historical mode without finite-difference repricing); callers that
   * need a marginal must choose a fallback or skip rebalancing.
   */
  marginal_var?: number | null;
  /**
   * Position identifier.
   */
  position_id: string;
  /**
   * Component VaR as a fraction of total portfolio VaR.
   *
   * `relative_var = component_var / portfolio_var`. Sums to 1.0.
   * A negative value indicates the position is a diversifier.
   */
  relative_var: number;
  [k: string]: unknown;
}
/**
 * Prepayment model specification.
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
 * Rating factor table for a specific rating-agency methodology.
 */
export interface RatingFactorTable {
  agency: string;
  default_factor: number;
  factors: {
    [k: string]: number;
  };
  methodology: string;
  [k: string]: unknown;
}
/**
 * A simulated rating trajectory: sequence of (time, state_index) pairs.
 */
export interface RatingPath {
  /**
   * Total simulation horizon.
   */
  horizon: number;
  /**
   * Rating scale used for the simulation. Shared via `Arc` so the
   * hot batch-simulation loops bump a refcount per path instead of
   * re-allocating the labels `Vec` and rebuilding the index map. The serde
   * "rc" feature keeps the wire format identical to an inline `RatingScale`.
   */
  scale: RatingScale;
  /**
   * Transition events as (time, state_index) pairs, starting with (0.0, s₀).
   */
  transitions: [unknown, unknown][];
  [k: string]: unknown;
}
/**
 * Recovery allocated to one claim.
 */
export interface RecoveryAllocation {
  /**
   * Recovery funded by net pledged collateral.
   */
  collateral_recovery: number;
  /**
   * Unrecovered allowed claim amount.
   */
  deficiency: number;
  /**
   * Recovery funded by the residual general estate.
   */
  general_recovery: number;
  /**
   * Stable claim identifier.
   */
  id: string;
  /**
   * Absolute-priority rank.
   */
  priority: number;
  /**
   * Total recovery divided by the allowed claim; zero for a zero claim.
   */
  recovery_rate: number;
  /**
   * Caller-defined seniority label.
   */
  seniority: string;
  /**
   * Total allowed claim amount.
   */
  total_claim: number;
  /**
   * Total recovery from collateral and the general estate.
   */
  total_recovery: number;
  [k: string]: unknown;
}
/**
 * A claim participating in a recovery waterfall.
 */
export interface RecoveryClaim {
  /**
   * Accrued amount.
   */
  accrued: number;
  /**
   * Collateral haircut in the inclusive range `[0, 1]`.
   */
  collateral_haircut: number;
  /**
   * Gross value of collateral pledged exclusively to this claim.
   */
  collateral_value?: number | null;
  /**
   * Stable claim identifier.
   */
  id: string;
  /**
   * Allowed penalties or fees.
   */
  penalties: number;
  /**
   * Principal amount.
   */
  principal: number;
  /**
   * Absolute-priority rank; lower values recover before higher values.
   */
  priority: number;
  /**
   * Caller-defined seniority label used for reporting.
   */
  seniority: string;
  [k: string]: unknown;
}
/**
 * Result of allocating a distributable estate across claims.
 */
export interface RecoveryWaterfallResult {
  /**
   * Per-claim allocations ordered by priority, then original input order.
   */
  allocations: RecoveryAllocation[];
  /**
   * Whether the residual estate was allocated in absolute-priority order.
   */
  apr_satisfied: boolean;
  /**
   * Total value distributed to claims.
   */
  total_distributed: number;
  /**
   * Estate value remaining after all allowed claims are satisfied.
   */
  undistributed_estate: number;
  [k: string]: unknown;
}
/**
 * Result of comparing actual risk decomposition against a risk budget.
 */
export interface RiskBudgetResult {
  /**
   * Whether any position exceeds its utilization threshold.
   */
  has_breach: boolean;
  /**
   * Per-position budget comparison.
   */
  positions: PositionBudgetEntry[];
  /**
   * Total over-budget amount: sum of positive consuming-side exceedances.
   */
  total_overbudget: number;
  [k: string]: unknown;
}
/**
 * Portfolio-level decomposition of total risk across common factors and residuals.
 */
export interface RiskDecomposition {
  /**
   * Aggregate factor-level contributions to portfolio risk.
   */
  factor_contributions: FactorContribution[];
  /**
   * Risk measure used to aggregate and report the decomposition.
   */
  measure: RiskMeasure;
  /**
   * Per-position, per-factor contributions that roll up into the portfolio view.
   */
  position_factor_contributions: PositionFactorContribution[];
  /**
   * Per-position residual (idiosyncratic) variance contributions.
   *
   * Populated only by credit-aware position decomposers that have access to
   * per-issuer idiosyncratic vol estimates. Empty when the decomposer has no
   * position-level residual allocation.
   */
  position_residual_contributions: PositionResidualContribution[];
  /**
   * Unattributed or idiosyncratic risk left after factor aggregation.
   * Same sign convention as `total_risk`.
   */
  residual_risk: number;
  /**
   * Total portfolio risk under the selected `measure`. Sign follows the
   * measure's convention (see struct-level docs).
   */
  total_risk: number;
}
/**
 * rBergomi model parameters.
 */
export interface RoughBergomiParams {
  /**
   * Vol-of-vol scaling (η > 0).
   */
  eta: number;
  /**
   * Hurst exponent H ∈ (0, 0.5), typically 0.07–0.12 for equity indices.
   */
  hurst: HurstExponent;
  /**
   * Dividend yield (annual, continuously compounded).
   */
  q: number;
  /**
   * Risk-free rate (annual, continuously compounded).
   */
  r: number;
  /**
   * Spot-vol correlation ρ ∈ [-1, 1].
   */
  rho: number;
  [k: string]: unknown;
}
/**
 * Rough Heston model parameters for Fourier-based European option pricing.
 */
export interface RoughHestonFourierParams {
  /**
   * Hurst exponent.
   */
  hurst: number;
  /**
   * Mean reversion speed.
   */
  kappa: number;
  /**
   * Spot-vol correlation.
   */
  rho: number;
  /**
   * Vol-of-vol.
   */
  sigma: number;
  /**
   * Long-run variance.
   */
  theta: number;
  /**
   * Initial variance.
   */
  v0: number;
}
/**
 * Rough Heston model parameters.
 */
export interface RoughHestonParams {
  /**
   * Hurst exponent H ∈ (0, 0.5) — controls the roughness of the variance.
   */
  hurst: HurstExponent;
  /**
   * Mean reversion speed (κ > 0).
   */
  kappa: number;
  /**
   * Dividend yield (annual, continuously compounded).
   */
  q: number;
  /**
   * Risk-free rate (annual, continuously compounded).
   */
  r: number;
  /**
   * Spot–variance correlation ρ ∈ \[−1, 1\].
   */
  rho: number;
  /**
   * Volatility of variance — vol-of-vol (σᵥ > 0).
   */
  sigma_v: number;
  /**
   * Long-run variance level (θ > 0).
   */
  theta: number;
  /**
   * Initial variance (v₀ > 0).
   */
  v0: number;
  [k: string]: unknown;
}
/**
 * Market data for SABR calibration.
 */
export interface SabrMarketData {
  /**
   * Fixed beta parameter
   */
  beta: number;
  /**
   * Forward price
   */
  forward: number;
  /**
   * Market implied volatilities
   */
  market_vols: number[];
  /**
   * Optional shift for handling negative rates in lognormal SABR (beta ≈ 1.0)
   * Default: 0.02 (200 basis points) if None and rates are negative
   */
  shift?: number | null;
  /**
   * Strike prices
   */
  strikes: number[];
  /**
   * Time to expiry
   */
  time_to_expiry: number;
  [k: string]: unknown;
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
 * Schwartz-Smith process parameters.
 */
export interface SchwartzSmithParams {
  /**
   * Mean reversion speed for short-term deviation (κ_X)
   */
  kappa: number;
  /**
   * Constant risk-premium drift shift for the short-term factor (λ_X).
   *
   * Under the risk-neutral measure the short-term factor follows
   * `dX = (−κ_X X − λ_X) dt + σ_X dW*` (Schwartz & Smith 2000): a constant
   * drift shift at unchanged κ_X. Defaults to 0 (physical measure).
   */
  lambda_x?: number;
  /**
   * Drift of long-term trend (μ_Y)
   */
  mu_y: number;
  /**
   * Correlation between X and Y (ρ)
   */
  rho_xy: number;
  /**
   * Volatility of short-term component (σ_X)
   */
  sigma_x: number;
  /**
   * Volatility of long-term component (σ_Y)
   */
  sigma_y: number;
}
/**
 * Result from any academic scoring model.
 */
export interface ScoringResult {
  /**
   * Optional implied probability of default.
   *
   * Altman score results leave this as `None` unless an explicit,
   * versioned heuristic calibration is requested. Ohlson and Zmijewski
   * retain their native logistic and probit probabilities.
   */
  implied_pd?: number | null;
  /**
   * Name of the model that produced this result.
   */
  model: string;
  /**
   * The raw score value (Z, Z', Z'', O, or Zmijewski Y).
   */
  score: number;
  /**
   * Risk zone classification (Safe/Grey/Distress).
   */
  zone: ScoringZone;
  [k: string]: unknown;
}
/**
 * Historical recovery calibration by seniority class.
 */
export interface SeniorityCalibration {
  /**
   * Per-seniority Beta recovery parameters.
   */
  classes: [unknown, unknown][];
  /**
   * Source label (e.g., "Moody's 1982-2023").
   */
  source: string;
  [k: string]: unknown;
}
/**
 * Recovery model driven by seniority-class Beta distributions.
 */
export interface SeniorityRecovery {
  class: SeniorityClass;
  dist: BetaRecovery;
  [k: string]: unknown;
}
/**
 * Results from Monte Carlo path simulation.
 */
export interface SimulatedPaths {
  /**
   * Asset values in row-major order: `path_idx * (num_steps + 1) + time_idx`.
   */
  asset_values: number[];
  /**
   * Number of paths simulated.
   */
  num_paths: number;
  /**
   * Number of time steps.
   */
  num_steps: number;
  /**
   * Time grid from 0 to T.
   */
  times: number[];
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
 * Per-position attribution of portfolio losses in tail scenarios.
 */
export interface StressAttribution {
  /**
   * Number of tail scenarios analyzed.
   */
  n_tail_scenarios: number;
  /**
   * Per-position average contribution to tail losses.
   *
   * Sorted by absolute contribution (largest risk driver first).
   */
  position_contributions: StressPositionEntry[];
  /**
   * Canonical position ordering shared by every [`tail_scenarios`] entry.
   *
   * `tail_scenarios[k].position_pnls[i]` is the P&L for `position_ids[i]`.
   * Storing the id list once here (rather than re-attaching it to every tail
   * scenario) avoids duplicating it `n_tail_scenarios` times.
   *
   * [`tail_scenarios`]: Self::tail_scenarios
   */
  position_ids: string[];
  /**
   * Individual tail scenario breakdowns.
   *
   * Contains `n_tail_scenarios` entries, each with per-position P&L
   * index-aligned to [`position_ids`](Self::position_ids).
   * Sorted by portfolio loss (worst first).
   */
  tail_scenarios: TailScenarioBreakdown[];
  /**
   * Portfolio VaR threshold (scenarios with P&L at or below this are
   * "tail events"). Loss convention: reported as a negative number.
   */
  var_threshold: number;
  [k: string]: unknown;
}
/**
 * Single position's contribution to tail stress.
 */
export interface StressPositionEntry {
  /**
   * Average P&L contribution in tail scenarios.
   */
  avg_tail_pnl: number;
  /**
   * Fraction of total portfolio tail loss attributable to this position.
   */
  pct_of_tail_loss: number;
  /**
   * Position identifier.
   */
  position_id: string;
  /**
   * Worst single-scenario P&L for this position.
   */
  worst_scenario_pnl: number;
  [k: string]: unknown;
}
/**
 * Breakdown of a single tail scenario.
 */
export interface TailScenarioBreakdown {
  /**
   * Total portfolio P&L for this scenario.
   */
  portfolio_pnl: number;
  /**
   * Per-position P&L contributions, index-aligned to
   * [`StressAttribution::position_ids`]. Entry `i` is the P&L for
   * `StressAttribution::position_ids[i]`.
   */
  position_pnls: number[];
  /**
   * Scenario index in the original history.
   */
  scenario_index: number;
  [k: string]: unknown;
}
/**
 * SVI (Stochastic Volatility Inspired) raw parameterization.
 */
export interface SviParams {
  /**
   * Overall variance level.
   */
  a: number;
  /**
   * Slope of the wings.
   */
  b: number;
  /**
   * Translation.
   */
  m: number;
  /**
   * Rotation / asymmetry parameter.
   */
  rho: number;
  /**
   * Smoothing parameter.
   */
  sigma: number;
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
 * Input parameters for a market impact calculation.
 */
export interface TradeParams {
  /**
   * Daily return volatility of the instrument.
   */
  daily_volatility: number;
  /**
   * Execution horizon in trading days.
   */
  horizon_days: number;
  /**
   * Liquidity profile for the instrument.
   */
  profile: LiquidityProfile;
  /**
   * Total quantity to execute (positive = buy, negative = sell).
   */
  quantity: number;
  /**
   * Reference price used to convert the return-space volatility
   * `daily_volatility` into a currency-space risk term (execution risk,
   * variance, etc.).
   *
   * `None` means fall back to `profile.mid`, which matches the
   * historical default. Set explicitly when the arrival price or
   * decision-time price differs materially from the profile mid (e.g.
   * when the profile was calibrated from a snapshot stale relative to
   * the order).
   */
  reference_price?: number | null;
  /**
   * Risk aversion parameter for trajectory optimization; `None` falls
   * back to the model's internal default (currently `1e-6` in the
   * Almgren-Chriss trajectory solver).
   */
  risk_aversion?: number | null;
}
/**
 * Loss statistics for one attachment/detachment tranche over a simulated pool loss distribution.
 */
export interface TrancheLossStatistics {
  /**
   * Tranche attachment point as a fraction of pool notional, in `[0, 1)`.
   */
  attachment: number;
  /**
   * Tranche detachment point as a fraction of pool notional, in `(0, 1]`.
   */
  detachment: number;
  /**
   * Mean tranche loss in pool-notional units.
   */
  expected_loss_amount: number;
  /**
   * Mean tranche loss as a fraction of tranche notional, in `[0, 1]`.
   */
  expected_loss_fraction: number;
  /**
   * Probability-weighted mean tranche loss amount in the worst confidence tail.
   */
  expected_shortfall_amount: number;
  /**
   * Probability-weighted mean tranche loss fraction in the worst confidence tail.
   */
  expected_shortfall_fraction: number;
  /**
   * Share of paths whose pool loss fraction strictly exceeds `attachment`.
   */
  prob_attachment_breached: number;
  /**
   * Share of paths whose pool loss fraction reaches or exceeds `detachment`.
   */
  prob_full_writedown: number;
  /**
   * Tranche notional `(detachment - attachment) * pool_notional`.
   */
  tranche_notional: number;
  /**
   * Nearest-rank tranche loss amount at the distribution's confidence.
   */
  var_amount: number;
  /**
   * Nearest-rank tranche loss fraction at the distribution's confidence.
   */
  var_fraction: number;
  [k: string]: unknown;
}
/**
 * Row-stochastic N×N transition matrix representing migration probabilities over a fixed time horizon.
 */
export interface TransitionMatrix {
  /**
   * @minItems 3
   * @maxItems 3
   */
  data: [unknown, unknown, unknown];
  horizon: number;
  scale: RatingScale;
}
/**
 * Variance Gamma model characteristic function.
 */
export interface VarianceGammaCf {
  /**
   * Variance rate of the Gamma subordinator (`nu > 0`).
   */
  nu: number;
  /**
   * Dividend yield.
   */
  q: number;
  /**
   * Risk-free rate.
   */
  r: number;
  /**
   * Volatility of the subordinated Brownian motion (`sigma > 0`).
   */
  sigma: number;
  /**
   * Drift of the subordinated Brownian motion.
   */
  theta: number;
  [k: string]: unknown;
}
/**
 * Workout and resolution costs.
 *
 * These reduce the net recovery available to creditors.
 */
export interface WorkoutCosts {
  /**
   * Direct costs as fraction of EAD (legal fees, administrative). Typical: 3-8%.
   */
  direct_cost_rate: number;
  /**
   * Indirect costs as fraction of EAD (opportunity cost, management distraction). Typical: 2-5%.
   */
  indirect_cost_rate: number;
  [k: string]: unknown;
}
/**
 * Workout-based LGD model using a collateral-first recovery waterfall.
 */
export interface WorkoutLgd {
  /**
   * Ordered collateral waterfall (highest priority first).
   */
  collateral: CollateralPiece[];
  /**
   * Direct and indirect resolution costs.
   */
  costs: WorkoutCosts;
  /**
   * Discount rate for time-value-of-money during workout.
   */
  discount_rate: number;
  /**
   * Expected workout duration in years. Typical: 1-5 years.
   */
  workout_years: number;
  [k: string]: unknown;
}
/**
 * Outcome of evaluating a `WorkoutLgd` model at one exposure at default.
 */
export interface WorkoutLgdResult {
  /**
   * Loss given default as a decimal fraction of EAD in `[0, 1]`.
   */
  lgd: number;
  /**
   * Net recovery amount in the same monetary units as the EAD, after the
   * collateral cap, workout costs, and discounting to the default date.
   */
  net_recovery: number;
  /**
   * Recovery rate `1 - lgd` as a decimal fraction in `[0, 1]`.
   */
  recovery_rate: number;
  [k: string]: unknown;
}
/**
 * h-step ahead yield curve forecast with confidence bands.
 */
export interface YieldForecast {
  /**
   * Factor point forecast [beta1, beta2, beta3].
   *
   * @minItems 3
   * @maxItems 3
   */
  factors: [number, number, number];
  /**
   * Forecast horizon in periods.
   */
  horizon: number;
  /**
   * 95% confidence band lower bound per tenor (length N).
   */
  lower_95: number[];
  /**
   * Tenor grid (length N).
   */
  tenors: number[];
  /**
   * 95% confidence band upper bound per tenor (length N).
   */
  upper_95: number[];
  /**
   * Point forecast: zero rates at each tenor (length N).
   */
  yields: number[];
  [k: string]: unknown;
}
/**
 * A panel of yield observations: rows = dates, columns = tenors.
 */
export interface YieldPanel {
  /**
   * Observation dates (optional, for labeling). Length T if provided.
   */
  dates?: DateWire[] | null;
  /**
   * Tenor grid in years, length N. Must be sorted ascending, all > 0.
   */
  tenors: number[];
  /**
   * Yield matrix: T rows (dates) x N columns (tenors).
   * Entry (t, i) is the zero rate at observation t for tenor i.
   *
   * @minItems 3
   * @maxItems 3
   */
  yields: [unknown, unknown, unknown];
  [k: string]: unknown;
}
/**
 * PCA decomposition of yield curve changes.
 */
export interface YieldPca {
  /**
   * Cumulative fraction of variance explained.
   */
  cumulative_variance: number[];
  /**
   * Eigenvalues in descending order (length min(T-1, N)).
   */
  eigenvalues: number[];
  /**
   * Loadings matrix: N tenors x K components (columns are eigenvectors).
   *
   * @minItems 3
   * @maxItems 3
   */
  loadings: [unknown, unknown, unknown];
  /**
   * Mean yield change vector (length N), subtracted before PCA.
   *
   * @minItems 3
   * @maxItems 3
   */
  mean_change: [unknown, unknown, unknown];
  /**
   * Scores matrix: (T-1) dates x K components.
   *
   * @minItems 3
   * @maxItems 3
   */
  scores: [unknown, unknown, unknown];
  /**
   * Tenor grid (length N).
   */
  tenors: number[];
  /**
   * Fraction of total variance explained by each component.
   */
  variance_explained: number[];
  [k: string]: unknown;
}
/**
 * Serializable view of the leading components of a `YieldPca` fit.
 */
export interface YieldPcaView {
  /**
   * Cumulative explained-variance fraction through each leading component.
   */
  cumulative_variance: number[];
  /**
   * Leading eigenvalues in descending order.
   */
  eigenvalues: number[];
  /**
   * Fraction of total variance explained by each leading component.
   */
  explained_variance_ratio: number[];
  /**
   * Row-major loadings `loadings[tenor][k]` for the leading components.
   */
  loadings: number[][];
  /**
   * Column means subtracted from the yield changes before PCA (one per tenor).
   */
  mean_change: number[];
  /**
   * Row-major scores `scores[t][k]` (one row per yield-change observation).
   */
  scores: number[][];
  /**
   * Tenor grid in years, in loading-row order.
   */
  tenors: number[];
  [k: string]: unknown;
}
/**
 * Input for the Zmijewski (1984) probit bankruptcy prediction model.
 */
export interface ZmijewskiInput {
  /**
   * Current Assets / Current Liabilities (current ratio / liquidity).
   */
  current_assets_to_current_liabilities: number;
  /**
   * Net Income / Total Assets (ROA).
   */
  net_income_to_total_assets: number;
  /**
   * Total Liabilities / Total Assets (leverage / financial leverage ratio).
   */
  total_liabilities_to_total_assets: number;
  [k: string]: unknown;
}
