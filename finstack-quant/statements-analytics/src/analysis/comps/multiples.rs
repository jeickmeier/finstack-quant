//! Valuation multiples computation.
//!
//! Pure functions that compute a specific multiple from `CompanyMetrics`.
//! Each function returns `None` when required inputs are missing or the
//! denominator is non-positive.

use super::peer_set::PeerSet;
use super::types::{CompanyId, CompanyMetrics, Multiple};

/// Compute the value of a multiple for a single company.
///
/// Returns `None` if the required inputs are missing or the
/// denominator is non-positive, or an input/result is non-finite (avoids
/// divide-by-zero, invalid observations, and overflowing multiples).
///
/// # Arguments
///
/// * `metrics` - Company financial and market metrics from which the selected
///   numerator and denominator are read.
/// * `multiple` - Multiple definition that selects the calculation and its
///   required metrics.
pub fn compute_multiple(metrics: &CompanyMetrics, multiple: Multiple) -> Option<f64> {
    match multiple {
        Multiple::EvEbitda => div_positive(metrics.enterprise_value?, metrics.ebitda?),
        Multiple::EvRevenue => div_positive(metrics.enterprise_value?, metrics.revenue?),
        Multiple::EvEbit => div_positive(metrics.enterprise_value?, metrics.ebit?),
        Multiple::EvFcf => div_positive(metrics.enterprise_value?, metrics.ufcf?),

        Multiple::Pe => div_positive(metrics.market_cap?, metrics.net_income?),
        Multiple::Pb => div_positive(metrics.market_cap?, metrics.book_value?),
        Multiple::Ptbv => div_positive(metrics.market_cap?, metrics.tangible_book_value?),
        Multiple::PFcf => div_positive(metrics.market_cap?, metrics.lfcf?),
        Multiple::DividendYield => div_positive(metrics.dividends_per_share?, metrics.share_price?),
        Multiple::SpreadPerTurn => div_positive(metrics.oas_bp?, metrics.leverage?),
        Multiple::YieldPerCoverage => div_positive(metrics.yield_pct?, metrics.interest_coverage?),
    }
}

/// Compute a multiple for every peer in the set.
///
/// Returns `(company_id, multiple_value)` pairs for peers where the
/// multiple is computable. Peers with missing data are silently skipped.
///
/// # Arguments
///
/// * `peer_set` - Peer universe whose companies are considered in stored order.
/// * `multiple` - Multiple definition to calculate for each eligible peer.
pub fn compute_peer_multiples(peer_set: &PeerSet, multiple: Multiple) -> Vec<(CompanyId, f64)> {
    peer_set
        .peers
        .iter()
        .filter_map(|c| compute_multiple(c, multiple).map(|v| (c.id.clone(), v)))
        .collect()
}

/// Safe division returning `None` when the denominator is non-positive.
#[inline]
fn div_positive(numerator: f64, denominator: f64) -> Option<f64> {
    if denominator <= 0.0 || !denominator.is_finite() || !numerator.is_finite() {
        None
    } else {
        let value = numerator / denominator;
        value.is_finite().then_some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_multiple_branches_reject_non_finite_inputs_and_results() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut metrics = CompanyMetrics::new("invalid");
            metrics.dividends_per_share = Some(invalid);
            metrics.share_price = Some(1.0);
            metrics.oas_bp = Some(invalid);
            metrics.leverage = Some(1.0);
            metrics.yield_pct = Some(invalid);
            metrics.interest_coverage = Some(1.0);
            for multiple in [
                Multiple::DividendYield,
                Multiple::SpreadPerTurn,
                Multiple::YieldPerCoverage,
            ] {
                assert!(compute_multiple(&metrics, multiple).is_none());
            }
        }
        let mut metrics = CompanyMetrics::new("overflow");
        metrics.enterprise_value = Some(f64::MAX);
        metrics.ebitda = Some(f64::MIN_POSITIVE);
        assert!(compute_multiple(&metrics, Multiple::EvEbitda).is_none());
        assert!(div_positive(1.0, f64::NAN).is_none());
    }
}
