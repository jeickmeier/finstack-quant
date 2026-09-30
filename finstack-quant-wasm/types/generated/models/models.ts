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
 * Exact schema marker accepted by [`FactorModelConfigEnvelope`].
 */
export type FactorModelConfigSchema = "finstack_quant.factor_model_config/1";

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
