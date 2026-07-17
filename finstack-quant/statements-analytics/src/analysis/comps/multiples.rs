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
/// denominator is non-positive (avoids divide-by-zero and
/// meaningless negative multiples).
///
/// # Arguments
///
/// * `metrics` - Company financial and market metrics from which the selected
///   numerator and denominator are read.
/// * `multiple` - Multiple definition that selects the calculation and its
///   required metrics.
pub fn compute_multiple(metrics: &CompanyMetrics, multiple: Multiple) -> Option<f64> {
    match multiple {
        // ---- EV multiples ----
        Multiple::EvEbitda => div_positive(metrics.enterprise_value?, metrics.ebitda?),
        Multiple::EvRevenue => div_positive(metrics.enterprise_value?, metrics.revenue?),
        Multiple::EvEbit => div_positive(metrics.enterprise_value?, metrics.ebit?),
        Multiple::EvFcf => div_positive(metrics.enterprise_value?, metrics.ufcf?),

        // ---- Equity multiples ----
        Multiple::Pe => div_positive(metrics.market_cap?, metrics.net_income?),
        Multiple::Pb => div_positive(metrics.market_cap?, metrics.book_value?),
        Multiple::Ptbv => div_positive(metrics.market_cap?, metrics.tangible_book_value?),
        Multiple::PFcf => div_positive(metrics.market_cap?, metrics.lfcf?),
        Multiple::DividendYield => {
            let price = metrics.share_price?;
            let dps = metrics.dividends_per_share?;
            if price <= 0.0 {
                return None;
            }
            Some(dps / price)
        }

        // ---- Credit multiples ----
        Multiple::SpreadPerTurn => {
            let spread = metrics.oas_bps?;
            let leverage = metrics.leverage?;
            if leverage <= 0.0 {
                return None;
            }
            Some(spread / leverage)
        }
        Multiple::YieldPerCoverage => {
            let yld = metrics.yield_pct?;
            let coverage = metrics.interest_coverage?;
            if coverage <= 0.0 {
                return None;
            }
            Some(yld / coverage)
        }
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
        Some(numerator / denominator)
    }
}
