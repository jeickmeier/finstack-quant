//! Core types for comparable company analysis.
//!
//! Defines the building blocks: company identifiers, valuation multiples,
//! period conventions, and per-company metric containers.

use finstack_quant_core::types::Attributes;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Opaque company identifier within a peer set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CompanyId(pub String);

impl CompanyId {
    /// Construct a new `CompanyId` from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the underlying identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CompanyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Time basis for computing a valuation multiple.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PeriodBasis {
    /// Last twelve months (trailing).
    Ltm,
    /// Next twelve months (forward consensus or forecast).
    Ntm,
    /// Custom period identified by a label (e.g., "FY2025E").
    Custom(String),
}

/// Valuation multiple.
///
/// Enterprise value multiples use EV as the numerator. Equity multiples
/// use market capitalization or share price. Credit multiples use spread
/// or yield as the numerator and a fundamental metric as denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Multiple {
    /// EV / EBITDA
    EvEbitda,
    /// EV / Revenue
    EvRevenue,
    /// EV / EBIT
    EvEbit,
    /// EV / Free Cash Flow (unlevered)
    EvFcf,

    /// Price / Earnings
    Pe,
    /// Price / Book Value
    Pb,
    /// Price / Tangible Book Value
    Ptbv,
    /// Price / Free Cash Flow (levered)
    PFcf,
    /// Dividend Yield (dividend / price, expressed as a ratio)
    DividendYield,

    /// Spread per turn of leverage (OAS / (Debt / EBITDA))
    SpreadPerTurn,
    /// Yield / Interest Coverage
    YieldPerCoverage,
}

impl Multiple {
    /// Returns true if this is an enterprise-value-based multiple.
    pub fn is_ev_multiple(&self) -> bool {
        matches!(
            self,
            Self::EvEbitda | Self::EvRevenue | Self::EvEbit | Self::EvFcf
        )
    }

    /// Returns true if this is an equity-based multiple.
    pub fn is_equity_multiple(&self) -> bool {
        matches!(
            self,
            Self::Pe | Self::Pb | Self::Ptbv | Self::PFcf | Self::DividendYield
        )
    }

    /// Returns true if this is a credit-specific multiple.
    pub fn is_credit_multiple(&self) -> bool {
        matches!(self, Self::SpreadPerTurn | Self::YieldPerCoverage)
    }

    /// Human-readable short label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::EvEbitda => "EV/EBITDA",
            Self::EvRevenue => "EV/Revenue",
            Self::EvEbit => "EV/EBIT",
            Self::EvFcf => "EV/FCF",
            Self::Pe => "P/E",
            Self::Pb => "P/B",
            Self::Ptbv => "P/TBV",
            Self::PFcf => "P/FCF",
            Self::DividendYield => "Div Yield",
            Self::SpreadPerTurn => "Spread/Turn",
            Self::YieldPerCoverage => "Yield/Coverage",
        }
    }
}

impl FromStr for Multiple {
    type Err = String;

    /// Parse an exact snake_case multiple identifier.
    ///
    /// Canonical forms: `"ev_ebitda"`, `"ev_revenue"`, `"ev_ebit"`, `"ev_fcf"`,
    /// `"pe"`, `"pb"`, `"ptbv"`, `"p_fcf"`, `"dividend_yield"`,
    /// `"spread_per_turn"`, `"yield_per_coverage"`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ev_ebitda" => Ok(Self::EvEbitda),
            "ev_revenue" => Ok(Self::EvRevenue),
            "ev_ebit" => Ok(Self::EvEbit),
            "ev_fcf" => Ok(Self::EvFcf),
            "pe" => Ok(Self::Pe),
            "pb" => Ok(Self::Pb),
            "ptbv" => Ok(Self::Ptbv),
            "p_fcf" => Ok(Self::PFcf),
            "dividend_yield" => Ok(Self::DividendYield),
            "spread_per_turn" => Ok(Self::SpreadPerTurn),
            "yield_per_coverage" => Ok(Self::YieldPerCoverage),
            _ => Err(format!(
                "unknown multiple {s:?}; expected one of ev_ebitda, ev_revenue, ev_ebit, ev_fcf, pe, pb, ptbv, p_fcf, dividend_yield, spread_per_turn, yield_per_coverage"
            )),
        }
    }
}

/// Metrics for a single company in a peer set.
///
/// All monetary values should be in the same currency before constructing
/// a `PeerSet`. Currency normalization is the caller's responsibility.
/// Ratios are plain scalars (e.g., `6.5` means 6.5x leverage).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CompanyMetrics {
    /// Company identifier.
    pub id: CompanyId,

    /// Optional instrument-level attributes (sector, geography, rating).
    /// Used by `PeerFilter` for inclusion/exclusion decisions.
    #[serde(default)]
    pub attributes: Attributes,

    /// Enterprise value.
    pub enterprise_value: Option<f64>,
    /// Equity market capitalization.
    pub market_cap: Option<f64>,
    /// Share price.
    pub share_price: Option<f64>,
    /// Option-adjusted spread in basis points.
    pub oas_bp: Option<f64>,
    /// Yield to worst / yield to maturity.
    pub yield_pct: Option<f64>,

    /// EBITDA (period basis determined by the PeerSet context).
    pub ebitda: Option<f64>,
    /// Revenue.
    pub revenue: Option<f64>,
    /// EBIT.
    pub ebit: Option<f64>,
    /// Unlevered free cash flow.
    pub ufcf: Option<f64>,
    /// Levered free cash flow.
    pub lfcf: Option<f64>,
    /// Net income / earnings.
    pub net_income: Option<f64>,
    /// Book value of equity.
    pub book_value: Option<f64>,
    /// Tangible book value.
    pub tangible_book_value: Option<f64>,
    /// Dividends per share (annualized).
    pub dividends_per_share: Option<f64>,

    /// Total debt / EBITDA.
    pub leverage: Option<f64>,
    /// EBITDA / Interest Expense.
    pub interest_coverage: Option<f64>,
    /// Revenue growth rate (decimal, e.g., 0.05 = 5%).
    pub revenue_growth: Option<f64>,
    /// EBITDA margin (decimal, e.g., 0.25 = 25%).
    pub ebitda_margin: Option<f64>,

    /// Arbitrary additional metrics keyed by name.
    /// Used for custom multiples or regression factors.
    #[serde(default)]
    pub custom: IndexMap<String, f64>,
}

macro_rules! company_metric_accessors {
    ($($field:ident),+ $(,)?) => {
        /// Return whether `name` is a canonical named metric field.
        ///
        /// # Arguments
        ///
        /// * `name` - Exact snake_case field name to classify.
        pub fn is_named_metric(name: &str) -> bool {
            matches!(name, $(stringify!($field))|+)
        }

        fn insert_flat_metric(&mut self, name: String, value: f64) {
            match name.as_str() {
                $(stringify!($field) => self.$field = Some(value),)+
                _ => {
                    self.custom.insert(name, value);
                }
            }
        }

        /// Return one canonical named metric field.
        ///
        /// Unknown names and named fields with no value both return `None`.
        ///
        /// # Arguments
        ///
        /// * `name` - Exact snake_case field name to read.
        pub fn named_metric(&self, name: &str) -> Option<f64> {
            match name {
                $(stringify!($field) => self.$field,)+
                _ => None,
            }
        }
    };
}

impl CompanyMetrics {
    company_metric_accessors!(
        enterprise_value,
        market_cap,
        share_price,
        oas_bp,
        yield_pct,
        ebitda,
        revenue,
        ebit,
        ufcf,
        lfcf,
        net_income,
        book_value,
        tangible_book_value,
        dividends_per_share,
        leverage,
        interest_coverage,
        revenue_growth,
        ebitda_margin,
    );

    /// Look up a metric by name: a canonical named field first, then a custom metric.
    ///
    /// # Arguments
    ///
    /// * `name` - Exact snake_case canonical field name (for example
    ///   `"ebitda"` or `"leverage"`) or the key of an entry in
    ///   [`custom`](Self::custom).
    ///
    /// # Returns
    ///
    /// The metric value in its own units, or `None` when the name is neither
    /// a populated canonical field nor a custom metric.
    pub fn get(&self, name: &str) -> Option<f64> {
        self.named_metric(name)
            .or_else(|| self.custom.get(name).copied())
    }

    /// Create a new `CompanyMetrics` with only the company ID set.
    /// All other fields default to `None` / empty.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: CompanyId::new(id),
            attributes: Attributes::default(),
            enterprise_value: None,
            market_cap: None,
            share_price: None,
            oas_bp: None,
            yield_pct: None,
            ebitda: None,
            revenue: None,
            ebit: None,
            ufcf: None,
            lfcf: None,
            net_income: None,
            book_value: None,
            tangible_book_value: None,
            dividends_per_share: None,
            leverage: None,
            interest_coverage: None,
            revenue_growth: None,
            ebitda_margin: None,
            custom: IndexMap::new(),
        }
    }

    /// Construct metrics from a flat host-language field map.
    ///
    /// Canonical named fields populate their dedicated slots. Unknown names
    /// are retained in [`CompanyMetrics::custom`]. A `None` value means the
    /// metric is missing and leaves its slot empty, exactly as if the name
    /// were absent, so hosts can pass `null` / `None` for unknown figures.
    ///
    /// # Arguments
    ///
    /// * `id` - Company identifier stored on the resulting record.
    /// * `values` - Flat snake_case metric names with `Some(value)` for a
    ///   supplied figure (finite or non-finite; numeric validation remains the
    ///   responsibility of the consuming analysis) or `None` for a missing
    ///   one.
    pub fn from_flat_metrics(
        id: impl Into<String>,
        values: impl IntoIterator<Item = (String, Option<f64>)>,
    ) -> Self {
        let mut metrics = Self::new(id);
        for (name, value) in values {
            if let Some(value) = value {
                metrics.insert_flat_metric(name, value);
            }
        }
        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_reads_named_fields_before_custom_metrics() {
        let mut metrics = CompanyMetrics::new("ACME");
        metrics.ebitda = Some(100.0);
        metrics.custom.insert("rule_of_40".to_string(), 42.0);
        // A custom entry never shadows a canonical field of the same name.
        metrics.custom.insert("ebitda".to_string(), 1.0);

        assert_eq!(metrics.get("ebitda"), Some(100.0));
        assert_eq!(metrics.get("rule_of_40"), Some(42.0));
        assert_eq!(metrics.get("leverage"), None);
        assert_eq!(metrics.get("unknown"), None);
    }
}
