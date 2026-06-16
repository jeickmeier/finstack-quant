//! Peer set construction and filtering.
//!
//! `PeerFilter` provides declarative criteria for selecting peer companies
//! from a larger universe. `PeerSet` holds the subject company alongside
//! its peers and provides metric extraction helpers.

use super::types::{CompanyId, CompanyMetrics, PeriodBasis};
use serde::{Deserialize, Serialize};

/// Criteria for filtering companies into a peer set.
///
/// All criteria are AND-ed: a company must satisfy every non-empty
/// constraint to be included. For OR semantics across sectors or
/// ratings, supply multiple values in the respective `Vec`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeerFilter {
    /// GICS sector codes to include (meta key: "gics_sector").
    pub gics_sectors: Vec<String>,
    /// GICS industry codes to include (meta key: "gics_industry").
    pub gics_industries: Vec<String>,
    /// ISO country codes to include (meta key: "country").
    pub countries: Vec<String>,
    /// Market cap floor (inclusive).
    pub market_cap_min: Option<f64>,
    /// Market cap ceiling (inclusive).
    pub market_cap_max: Option<f64>,
    /// Credit rating bands to include (meta key: "rating").
    pub ratings: Vec<String>,
    /// Required attribute tags (all must be present).
    pub required_tags: Vec<String>,
    /// Excluded attribute tags (none may be present).
    pub excluded_tags: Vec<String>,
    /// Arbitrary attribute selector strings (uses `Attributes::matches_selector`).
    pub selectors: Vec<String>,
}

impl PeerFilter {
    /// Returns true if the company satisfies all filter criteria.
    pub fn accepts(&self, metrics: &CompanyMetrics) -> bool {
        // GICS sector
        if !self.gics_sectors.is_empty() {
            let sector = metrics.attributes.get_meta("gics_sector").unwrap_or("");
            if !self.gics_sectors.iter().any(|s| s == sector) {
                return false;
            }
        }

        // GICS industry
        if !self.gics_industries.is_empty() {
            let industry = metrics.attributes.get_meta("gics_industry").unwrap_or("");
            if !self.gics_industries.iter().any(|s| s == industry) {
                return false;
            }
        }

        // Country
        if !self.countries.is_empty() {
            let country = metrics.attributes.get_meta("country").unwrap_or("");
            if !self.countries.iter().any(|c| c == country) {
                return false;
            }
        }

        // Market cap range
        if let Some(min) = self.market_cap_min {
            if metrics.market_cap.is_none_or(|mc| mc < min) {
                return false;
            }
        }
        if let Some(max) = self.market_cap_max {
            if metrics.market_cap.is_none_or(|mc| mc > max) {
                return false;
            }
        }

        // Rating
        if !self.ratings.is_empty() {
            let rating = metrics.attributes.get_meta("rating").unwrap_or("");
            if !self.ratings.iter().any(|r| r == rating) {
                return false;
            }
        }

        // Required tags
        for tag in &self.required_tags {
            if !metrics.attributes.has_tag(tag) {
                return false;
            }
        }

        // Excluded tags
        for tag in &self.excluded_tags {
            if metrics.attributes.has_tag(tag) {
                return false;
            }
        }

        // Arbitrary selectors
        for selector in &self.selectors {
            if !metrics.attributes.matches_selector(selector) {
                return false;
            }
        }

        true
    }
}

/// A set of comparable companies with their metrics.
///
/// Constructed from a universe of `CompanyMetrics` with optional filtering.
/// The subject company (the one being analyzed) is stored separately from
/// the peers for clarity in downstream calculations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerSet {
    /// The subject company being evaluated.
    pub subject: CompanyMetrics,
    /// Peer companies in the comparison set.
    pub peers: Vec<CompanyMetrics>,
    /// Period basis for all metrics in this set.
    pub period_basis: PeriodBasis,
}

impl PeerSet {
    /// Construct a peer set from a subject and a universe of candidates.
    ///
    /// Applies `filter` to the universe. The subject is never included
    /// in the peer list even if it passes the filter.
    pub fn from_universe(
        subject: CompanyMetrics,
        universe: &[CompanyMetrics],
        filter: &PeerFilter,
        period_basis: PeriodBasis,
    ) -> Self {
        let peers: Vec<CompanyMetrics> = universe
            .iter()
            .filter(|c| c.id != subject.id && filter.accepts(c))
            .cloned()
            .collect();

        Self {
            subject,
            peers,
            period_basis,
        }
    }

    /// Construct directly from a subject and an explicit peer list.
    pub fn new(
        subject: CompanyMetrics,
        peers: Vec<CompanyMetrics>,
        period_basis: PeriodBasis,
    ) -> Self {
        Self {
            subject,
            peers,
            period_basis,
        }
    }

    /// Number of peers (excluding subject).
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Extract a specific metric as a vec across all peers.
    ///
    /// Returns only peers where the metric is `Some`. The returned
    /// `Vec` contains `(CompanyId, value)` pairs preserving the
    /// association.
    pub fn extract_metric(
        &self,
        extractor: impl Fn(&CompanyMetrics) -> Option<f64>,
    ) -> Vec<(CompanyId, f64)> {
        self.peers
            .iter()
            .filter_map(|c| extractor(c).map(|v| (c.id.clone(), v)))
            .collect()
    }

    /// Extract the subject's value for a given metric.
    pub fn subject_metric(
        &self,
        extractor: impl Fn(&CompanyMetrics) -> Option<f64>,
    ) -> Option<f64> {
        extractor(&self.subject)
    }
}
