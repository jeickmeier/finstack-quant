//! Comparable company and relative value analysis.
//!
//! Provides peer set construction, valuation multiples computation,
//! cross-sectional statistical analysis, and composite rich/cheap scoring.
//!
//! Start with [`PeerSet::from_universe`] to build a filtered peer group,
//! then use [`compute_peer_multiples`] for multiples and
//! [`score_relative_value`] for the composite rich/cheap signal, or call
//! [`relative_value_report`] for all of it at once.

pub(crate) mod multiples;
pub(crate) mod peer_set;
pub(crate) mod report;
pub(crate) mod scoring;
pub(crate) mod stats;
pub(crate) mod types;

#[cfg(test)]
pub(super) use multiples::{compute_multiple, compute_peer_multiples};
#[cfg(test)]
pub(super) use peer_set::{PeerFilter, PeerSet};
#[cfg(test)]
pub(super) use scoring::{
    MetricExtractor, RelativeValueResult, ScoreDirection, ScoringDimension, score_relative_value,
};
#[cfg(test)]
pub(super) use stats::{peer_stats, percentile_rank, regression_fair_value, z_score};
#[cfg(test)]
pub(super) use types::{CompanyId, CompanyMetrics, Multiple, PeriodBasis};

#[cfg(test)]
mod tests;
