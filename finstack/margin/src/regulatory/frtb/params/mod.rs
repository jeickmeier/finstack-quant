//! Prescribed FRTB risk weights, correlations, and other regulatory parameters.
//!
//! The per-risk-class submodules (`commodity`, `csr`, `equity`, `fx`,
//! `girr`) expose `pub const` tables matching BCBS d457 (January 2019)
//! and are read directly by the charge-calculation helpers.
//!
//! [`registry::FrtbParams`] bundles the same values into a serializable,
//! revision-tagged struct with a JSON-overlay loader and range
//! validation so alternate parameter sets (e.g. d554) can be tested
//! without recompiling.

pub mod commodity;
pub mod correlation_scenarios;
pub mod csr;
pub mod equity;
pub mod fx;
pub mod girr;
pub mod registry;

pub use registry::{
    CommodityParams, CorrelationScenarioParams, DrcParams, EquityParams, FrtbParams, FrtbRevision,
    FxParams, GirrParams,
};

#[cfg(test)]
mod tests {
    use super::{commodity, csr, equity};

    #[test]
    fn equity_delta_risk_weights_match_bcbs_d457_index_buckets() {
        assert_eq!(equity::equity_risk_weight(11), 70.0);
        assert_eq!(equity::equity_risk_weight(12), 15.0);
        assert_eq!(equity::equity_risk_weight(13), 25.0);
    }

    #[test]
    fn commodity_delta_risk_weights_match_bcbs_d457() {
        let expected = [
            (1, 30.0),
            (2, 35.0),
            (3, 60.0),
            (4, 80.0),
            (5, 40.0),
            (6, 45.0),
            (7, 20.0),
            (8, 35.0),
            (9, 25.0),
            (10, 35.0),
            (11, 50.0),
        ];
        for (bucket, weight) in expected {
            assert_eq!(commodity::commodity_risk_weight(bucket), weight);
        }
    }

    #[test]
    fn csr_nonsec_buckets_8_to_15_match_bcbs_d457() {
        let expected = [
            (8, 1.0),
            (9, 2.5),
            (10, 4.0),
            (11, 12.0),
            (12, 7.0),
            (13, 8.5),
            (14, 5.5),
            (15, 5.0),
        ];
        for (bucket, weight) in expected {
            assert_eq!(csr::csr_nonsec_risk_weight(bucket), weight);
        }
    }
}
