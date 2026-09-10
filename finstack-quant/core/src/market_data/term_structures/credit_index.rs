//! Credit index market data aggregation for CDO/CDS pricing.
//!
//! Packages all market data components needed to price credit derivatives on
//! standardized credit indices like CDX (North America) and iTraxx (Europe).
//! Includes index hazard curve, base correlation curve, recovery rates, and
//! optional constituent issuer curves.
//!
//! # Components
//!
//! - Index-level hazard curve (average credit risk)
//! - Base correlation curve (tranche correlation skew)
//! - Recovery rate (typically 40%)
//! - Optional per-issuer hazard curves (for heterogeneous pools)
//!
//! # Use Cases
//!
//! - CDO tranche pricing (synthetic and cash)
//! - CDS index tranche valuation
//! - Bespoke portfolio pricing
//! - Credit correlation trading

use super::{hazard_curve::HazardCurve, BaseCorrelationCurve};
use crate::Result;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Aggregated market data for a specific credit index.
///
/// Contains all the curves and parameters needed to price credit derivatives
/// on a standardized credit index using models like the Gaussian Copula.
///
/// Note: This struct contains Arc-wrapped curves that cannot be directly serialized.
/// For persistence, extract and serialize the underlying curve data separately.
#[derive(Clone, Debug)]
pub struct CreditIndexData {
    /// Number of constituents in the credit index (e.g., 125 for CDX IG)
    pub num_constituents: u16,
    /// Default recovery rate for the index (typically 40% for senior unsecured)
    pub recovery_rate: f64,
    /// Hazard curve for the index as a whole
    pub index_credit_curve: Arc<HazardCurve>,
    /// Base correlation curve mapping detachment points to correlations
    pub base_correlation_curve: Arc<BaseCorrelationCurve>,
    /// Optional complete set of individual hazard curves for every constituent.
    /// Keys are issuer identifiers (e.g., ticker or CUSIP); when provided the
    /// map must contain exactly `num_constituents` distinct issuers.
    pub issuer_credit_curves: Option<BTreeMap<String, Arc<HazardCurve>>>,
    /// Optional individual recovery rates for each constituent issuer
    /// Key is the issuer identifier (e.g., ticker or CUSIP)
    pub issuer_recovery_rates: Option<BTreeMap<String, f64>>,
    /// Optional individual weights for each constituent issuer (must sum to 1.0)
    /// Key is the issuer identifier (e.g., ticker or CUSIP)
    pub issuer_weights: Option<BTreeMap<String, f64>>,
}

impl CreditIndexData {
    /// Create a new credit index data builder.
    #[must_use]
    pub fn builder() -> CreditIndexDataBuilder {
        CreditIndexDataBuilder::default()
    }

    /// Get the credit curve for a specific issuer.
    ///
    /// Returns the issuer-specific curve if available, otherwise falls back
    /// to the index curve (homogeneous portfolio assumption).
    pub fn get_issuer_curve(&self, issuer_id: &str) -> &HazardCurve {
        self.issuer_credit_curves
            .as_ref()
            .and_then(|curves| curves.get(issuer_id))
            .map(|arc| arc.as_ref())
            .unwrap_or(self.index_credit_curve.as_ref())
    }

    /// Check if heterogeneous pricing mode is available.
    ///
    /// Returns true if individual issuer curves are provided, enabling
    /// more granular portfolio loss modeling.
    #[must_use]
    pub fn has_issuer_curves(&self) -> bool {
        self.issuer_credit_curves
            .as_ref()
            .is_some_and(|curves| !curves.is_empty())
    }

    /// Get all available issuer identifiers, sorted for deterministic output.
    pub fn issuer_ids(&self) -> Vec<String> {
        match &self.issuer_credit_curves {
            Some(curves) => {
                let mut ids: Vec<String> = curves.keys().cloned().collect();
                ids.sort_unstable();
                ids
            }
            None => Vec::new(),
        }
    }

    /// Get the recovery rate for a specific issuer.
    ///
    /// Returns the issuer-specific recovery rate if available, otherwise falls back
    /// to the index recovery rate (homogeneous portfolio assumption).
    pub fn get_issuer_recovery(&self, issuer_id: &str) -> f64 {
        self.issuer_recovery_rates
            .as_ref()
            .and_then(|m| m.get(issuer_id).copied())
            .unwrap_or(self.recovery_rate)
    }

    /// Get the weight for a specific issuer.
    ///
    /// Returns the issuer-specific weight if available, otherwise falls back
    /// to equal weighting (1/N).
    pub fn get_issuer_weight(&self, issuer_id: &str) -> f64 {
        self.issuer_weights
            .as_ref()
            .and_then(|m| m.get(issuer_id).copied())
            .unwrap_or(1.0 / self.num_constituents as f64)
    }
}

/// Builder for creating credit index data.
///
/// The builder collects index-wide metadata (constituent count, recovery) and
/// attaches the market curves required for tranche pricing.
///
/// # Examples
/// ```rust
/// use finstack_quant_core::market_data::term_structures::{
///     BaseCorrelationCurve, CreditIndexData, HazardCurve,
/// };
/// use finstack_quant_core::dates::Date;
/// use std::sync::Arc;
/// use time::Month;
///
/// let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
/// let hazard = Arc::new(
///     HazardCurve::builder("CDX")
///         .base_date(base)
///         .recovery_rate(0.40)
///         .knots([(0.0, 0.01), (5.0, 0.015)])
///         .build()
///         .expect("HazardCurve builder should succeed"),
/// );
/// let base_corr = Arc::new(
///     BaseCorrelationCurve::builder("CDX")
///         .knots([(3.0, 0.25), (10.0, 0.55)])
///         .build()
///         .expect("BaseCorrelationCurve builder should succeed"),
/// );
/// let index = CreditIndexData::builder()
///     .num_constituents(125)
///     .recovery_rate(0.4)
///     .index_credit_curve(hazard)
///     .base_correlation_curve(base_corr)
///     .build()
///     .expect("CreditIndexData builder should succeed");
/// assert_eq!(index.num_constituents, 125);
/// ```
#[derive(Default)]
pub struct CreditIndexDataBuilder {
    num_constituents: Option<u16>,
    recovery_rate: Option<f64>,
    index_credit_curve: Option<Arc<HazardCurve>>,
    base_correlation_curve: Option<Arc<BaseCorrelationCurve>>,
    issuer_credit_curves: Option<BTreeMap<String, Arc<HazardCurve>>>,
    issuer_recovery_rates: Option<BTreeMap<String, f64>>,
    issuer_weights: Option<BTreeMap<String, f64>>,
}

impl CreditIndexDataBuilder {
    /// Set the number of constituents in the index.
    pub fn num_constituents(mut self, count: u16) -> Self {
        self.num_constituents = Some(count);
        self
    }

    /// Set the recovery rate (fraction between 0.0 and 1.0).
    pub fn recovery_rate(mut self, rate: f64) -> Self {
        self.recovery_rate = Some(rate);
        self
    }

    /// Set the index-level credit curve.
    pub fn index_credit_curve(mut self, curve: Arc<HazardCurve>) -> Self {
        self.index_credit_curve = Some(curve);
        self
    }

    /// Set the base correlation curve.
    pub fn base_correlation_curve(mut self, curve: Arc<BaseCorrelationCurve>) -> Self {
        self.base_correlation_curve = Some(curve);
        self
    }

    /// Set issuer-specific credit curves for heterogeneous portfolio modeling.
    ///
    /// Entries are stored by issuer identifier in lexicographic order so
    /// snapshots are deterministic. If `curves` repeats an identifier, the
    /// last curve yielded by the iterator replaces earlier values. Validation
    /// is deferred until [`build`](Self::build), which requires exactly
    /// `num_constituents` distinct issuer curves so no pool weight is omitted.
    ///
    /// # Arguments
    ///
    /// * `curves` - Issuer identifier and hazard-curve pairs. Identifiers are
    ///   caller-defined stable keys and must be reused by
    ///   [`issuer_recovery_rates`](Self::issuer_recovery_rates) and
    ///   [`issuer_weights`](Self::issuer_weights); each curve supplies that
    ///   issuer's default-probability term structure. Supply the complete
    ///   constituent set; partial coverage is rejected at build time.
    pub fn issuer_curves<I>(mut self, curves: I) -> Self
    where
        I: IntoIterator<Item = (String, Arc<HazardCurve>)>,
    {
        self.issuer_credit_curves = Some(curves.into_iter().collect());
        self
    }
    /// Set issuer-specific recovery rates for heterogeneous portfolio modeling.
    ///
    /// Entries are stored in lexicographic issuer order. Repeated identifiers
    /// use the last value yielded by the iterator. Validation is deferred until
    /// [`build`](Self::build), which requires issuer curves, rejects unknown
    /// identifiers, and checks each recovery lies in the inclusive `[0, 1]`
    /// range.
    ///
    /// # Arguments
    ///
    /// * `rates` - Issuer identifier and recovery-rate pairs. Identifiers must
    ///   match keys supplied to [`issuer_curves`](Self::issuer_curves).
    ///   Recoveries use decimal fractions, so `0.40` means 40%, not 40
    ///   percentage points or 4,000 basis points.
    pub fn issuer_recovery_rates<I>(mut self, rates: I) -> Self
    where
        I: IntoIterator<Item = (String, f64)>,
    {
        self.issuer_recovery_rates = Some(rates.into_iter().collect());
        self
    }

    /// Set issuer-specific weights for heterogeneous portfolio modeling.
    ///
    /// Entries are stored in lexicographic issuer order. Repeated identifiers
    /// use the last value yielded by the iterator. Validation is deferred until
    /// [`build`](Self::build), which requires exact coverage of the issuer
    /// curves, rejects unknown identifiers and negative or non-finite values,
    /// and requires the weights to sum to `1.0` within `1e-9`.
    ///
    /// # Arguments
    ///
    /// * `weights` - Issuer identifier and portfolio-weight pairs.
    ///   Identifiers must match keys supplied to
    ///   [`issuer_curves`](Self::issuer_curves). Weights are non-negative
    ///   decimal fractions, so `0.20` represents 20% of index notional.
    pub fn issuer_weights<I>(mut self, weights: I) -> Self
    where
        I: IntoIterator<Item = (String, f64)>,
    {
        self.issuer_weights = Some(weights.into_iter().collect());
        self
    }

    /// Build the credit index data.
    pub fn build(self) -> Result<CreditIndexData> {
        let num_constituents = self
            .num_constituents
            .ok_or_else(|| crate::Error::from(crate::error::InputError::Invalid))?;

        let recovery_rate = self.recovery_rate.ok_or_else(|| {
            crate::Error::Validation(
                "CreditIndexData requires an explicit recovery_rate in [0, 1]".to_string(),
            )
        })?;

        let index_credit_curve = self
            .index_credit_curve
            .ok_or_else(|| crate::Error::from(crate::error::InputError::Invalid))?;

        let base_correlation_curve = self
            .base_correlation_curve
            .ok_or_else(|| crate::Error::from(crate::error::InputError::Invalid))?;

        super::common::validate_unit_range(recovery_rate, "recovery_rate")?;

        if num_constituents == 0 {
            return Err(crate::Error::from(crate::error::InputError::Invalid));
        }

        validate_issuer_curves(&self.issuer_credit_curves, num_constituents)?;
        validate_issuer_recovery_rates(&self.issuer_credit_curves, &self.issuer_recovery_rates)?;
        validate_issuer_weights(
            &self.issuer_credit_curves,
            &self.issuer_weights,
            num_constituents,
        )?;

        Ok(CreditIndexData {
            num_constituents,
            recovery_rate,
            index_credit_curve,
            base_correlation_curve,
            issuer_credit_curves: self.issuer_credit_curves,
            issuer_recovery_rates: self.issuer_recovery_rates,
            issuer_weights: self.issuer_weights,
        })
    }
}

fn validate_issuer_curves(
    issuer_curves: &Option<BTreeMap<String, Arc<HazardCurve>>>,
    num_constituents: u16,
) -> Result<()> {
    if let Some(curves) = issuer_curves {
        if curves.is_empty() {
            return Err(crate::Error::Validation(
                "issuer_credit_curves must not be empty when provided".to_string(),
            ));
        }
        if curves.len() != usize::from(num_constituents) {
            return Err(crate::Error::Validation(format!(
                "issuer_credit_curves has {} entries; complete coverage requires exactly num_constituents={num_constituents}",
                curves.len()
            )));
        }
    }
    Ok(())
}

fn validate_issuer_recovery_rates(
    issuer_curves: &Option<BTreeMap<String, Arc<HazardCurve>>>,
    issuer_recovery_rates: &Option<BTreeMap<String, f64>>,
) -> Result<()> {
    let Some(recovery_rates) = issuer_recovery_rates else {
        return Ok(());
    };
    let Some(curves) = issuer_curves else {
        return Err(crate::Error::Validation(
            "issuer_recovery_rates requires issuer_credit_curves so issuer IDs are well-defined"
                .to_string(),
        ));
    };

    for (issuer, recovery) in recovery_rates {
        if !curves.contains_key(issuer) {
            return Err(crate::Error::Validation(format!(
                "issuer_recovery_rates contains unknown issuer '{issuer}'"
            )));
        }
        super::common::validate_unit_range(*recovery, &format!("issuer_recovery_rates[{issuer}]"))?;
    }

    Ok(())
}

fn validate_issuer_weights(
    issuer_curves: &Option<BTreeMap<String, Arc<HazardCurve>>>,
    issuer_weights: &Option<BTreeMap<String, f64>>,
    num_constituents: u16,
) -> Result<()> {
    let Some(weights) = issuer_weights else {
        return Ok(());
    };
    let Some(curves) = issuer_curves else {
        return Err(crate::Error::Validation(
            "issuer_weights requires issuer_credit_curves so issuer IDs are well-defined"
                .to_string(),
        ));
    };

    if weights.is_empty() {
        return Err(crate::Error::Validation(
            "issuer_weights must not be empty when provided".to_string(),
        ));
    }
    if weights.len() != curves.len() {
        return Err(crate::Error::Validation(format!(
            "issuer_weights has {} entries but issuer_credit_curves has {}; weights must cover every issuer curve exactly once",
            weights.len(),
            curves.len()
        )));
    }
    if weights.len() > usize::from(num_constituents) {
        return Err(crate::Error::Validation(format!(
            "issuer_weights has {} entries, which exceeds num_constituents={num_constituents}",
            weights.len()
        )));
    }

    let mut weight_sum = 0.0;
    for (issuer, weight) in weights {
        if !curves.contains_key(issuer) {
            return Err(crate::Error::Validation(format!(
                "issuer_weights contains unknown issuer '{issuer}'"
            )));
        }
        if !weight.is_finite() {
            return Err(crate::Error::Validation(format!(
                "issuer_weights[{issuer}] must be finite, got {weight}"
            )));
        }
        if *weight < 0.0 {
            return Err(crate::Error::Validation(format!(
                "issuer_weights[{issuer}] must be non-negative, got {weight}"
            )));
        }
        weight_sum += *weight;
    }

    if (weight_sum - 1.0).abs() > 1e-9 {
        return Err(crate::Error::Validation(format!(
            "issuer_weights must sum to 1.0, got {weight_sum}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod production_credit_audit {
    use super::*;
    use crate::dates::{Date, Month};

    #[test]
    fn issuer_curves_cover_every_constituent() {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let hazard = Arc::new(
            HazardCurve::builder("A")
                .base_date(base)
                .recovery_rate(0.4)
                .knots([(1.0, 0.02)])
                .build()
                .expect("hazard"),
        );
        let correlation = Arc::new(
            BaseCorrelationCurve::builder("BC")
                .knots([(3.0, 0.3), (100.0, 0.3)])
                .build()
                .expect("correlation"),
        );
        let result = CreditIndexData::builder()
            .num_constituents(2)
            .recovery_rate(0.4)
            .index_credit_curve(Arc::clone(&hazard))
            .base_correlation_curve(correlation)
            .issuer_curves(BTreeMap::from([("A".to_owned(), hazard)]))
            .build();
        assert!(result.is_err(), "one issuer must not price a two-name pool");
    }
}
