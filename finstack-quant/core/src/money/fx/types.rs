use crate::currency::Currency;
use crate::dates::Date;
use serde::{Deserialize, Serialize};

/// Standard FX conversion strategies used to hint FX providers.
///
/// The policy tells a provider *how* the rate will be applied so it can decide
/// between spot, forward, or averaged sources.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FxConversionPolicy {
    /// Use spot/forward on the cashflow date.
    CashflowDate,
    /// Use period end date.
    PeriodEnd,
    /// Use an average over the period.
    PeriodAverage,
    /// Custom strategy defined by the caller/provider.
    Custom,
}

impl crate::parse::NormalizedEnum for FxConversionPolicy {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("cashflow_date", Self::CashflowDate),
        ("cashflow", Self::CashflowDate),
        ("period_end", Self::PeriodEnd),
        ("end", Self::PeriodEnd),
        ("period_average", Self::PeriodAverage),
        ("average", Self::PeriodAverage),
        ("custom", Self::Custom),
    ];
}

impl std::fmt::Display for FxConversionPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CashflowDate => write!(f, "cashflow_date"),
            Self::PeriodEnd => write!(f, "period_end"),
            Self::PeriodAverage => write!(f, "period_average"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for FxConversionPolicy {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s).map_err(|_| crate::error::InputError::Invalid.into())
    }
}

/// Simple FX rate query.
///
/// Contains only the essential parameters for currency conversion.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FxQuery {
    /// Source currency
    pub from: Currency,
    /// Target currency
    pub to: Currency,
    /// Applicable date for the rate
    pub on: Date,
    /// Conversion policy (defaults to CashflowDate)
    #[serde(default = "default_policy")]
    pub policy: FxConversionPolicy,
}

fn default_policy() -> FxConversionPolicy {
    FxConversionPolicy::CashflowDate
}

impl FxQuery {
    /// Create a new FX query with default policy.
    pub fn new(from: Currency, to: Currency, on: Date) -> Self {
        Self {
            from,
            to,
            on,
            policy: FxConversionPolicy::CashflowDate,
        }
    }

    /// Create a new FX query with specific policy.
    pub fn with_policy(from: Currency, to: Currency, on: Date, policy: FxConversionPolicy) -> Self {
        Self {
            from,
            to,
            on,
            policy,
        }
    }
}

/// Metadata describing the policy applied by the provider.
///
/// Attach [`FxPolicyMeta`] to valuation results so auditors can understand how
/// FX conversions were sourced.
///
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FxPolicyMeta {
    /// Strategy applied for the conversion.
    pub strategy: FxConversionPolicy,
    /// Optional declared target currency (for stamping).
    pub target_ccy: Option<Currency>,
    /// Optional notes for auditability.
    pub notes: String,
}

impl Default for FxPolicyMeta {
    fn default() -> Self {
        Self {
            strategy: FxConversionPolicy::CashflowDate,
            target_ccy: None,
            notes: String::new(),
        }
    }
}

/// Configuration for [`FxMatrix`](crate::money::fx::FxMatrix) behaviour.
///
/// Controls triangulation and caching.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(default)]
pub struct FxConfig {
    /// Pivot currency for triangulation fallback (typically USD).
    ///
    /// Current limitation: triangulation uses this single configured pivot only.
    /// If a market convention requires a different routing currency for a given
    /// pair, callers must seed that direct quote explicitly instead of relying on
    /// automatic cross construction.
    pub pivot_currency: Currency,
    /// Whether to enable automatic triangulation for missing rates.
    ///
    /// When enabled, the matrix will attempt `from -> pivot -> to` and will not
    /// search multi-hop paths or alternative pivots.
    pub enable_triangulation: bool,
    /// Maximum number of cached quotes to retain in an LRU
    pub cache_capacity: usize,
}

impl Default for FxConfig {
    fn default() -> Self {
        Self {
            pivot_currency: Currency::USD,
            // Triangulation is on by default: most FX deployments need cross
            // rates through a pivot (typically USD). Disable explicitly for
            // strict-direct-quote setups.
            enable_triangulation: true,
            cache_capacity: 256,
        }
    }
}

/// Result of an FX rate lookup with simple triangulation info.
///
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FxRateResult {
    /// The final FX rate
    pub rate: f64,
    /// Whether this rate was obtained via triangulation
    pub triangulated: bool,
}

/// Serializable state of an FxMatrix.
/// Contains the configuration and cached quotes that can be persisted and restored.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FxMatrixState {
    /// FX configuration
    pub config: FxConfig,
    /// Cached FX quotes as (from, to, rate) tuples
    pub quotes: Vec<(Currency, Currency, f64)>,
    /// Pinned, date/policy-scoped quotes as `(from, to, on, policy, rate)`.
    ///
    /// Added snapshots previously
    /// dropped pinned fixings, so a restored matrix silently re-derived
    /// those dates from the provider. The field is serde-additive
    /// (`default`), so older payloads without it still deserialize.
    #[serde(default)]
    pub pinned_quotes: Vec<(Currency, Currency, Date, FxConversionPolicy, f64)>,
}
