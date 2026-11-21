//! Shared helpers for the instruments' [`Marginable`](finstack_quant_margin::Marginable) implementations.
//!
//! Each instrument family implements `finstack_quant_margin::Marginable`
//! beside its instrument type (`<instrument>/marginable.rs`) and reaches
//! it through `Instrument::as_marginable`; the multi-family tenor-bucketing
//! and netting-set helpers they share live here.

use finstack_quant_margin::constants;
use finstack_quant_margin::types::ClearingStatus;
use finstack_quant_margin::types::NettingSetId;
use finstack_quant_margin::types::OtcMarginSpec;

/// Map a time to maturity in years to its ISDA SIMM IR tenor bucket label.
///
/// # Arguments
///
/// * `years_to_maturity` - Remaining tenor in years using the caller's day-count convention.
#[must_use]
pub fn assign_ir_tenor_bucket(years_to_maturity: f64) -> &'static str {
    use constants::tenor_buckets::*;
    match years_to_maturity {
        y if y <= BUCKET_6M => "6M",
        y if y <= BUCKET_1Y => "1Y",
        y if y <= BUCKET_2Y => "2Y",
        y if y <= BUCKET_3Y => "3Y",
        y if y <= BUCKET_5Y => "5Y",
        y if y <= BUCKET_10Y => "10Y",
        y if y <= BUCKET_15Y => "15Y",
        y if y <= BUCKET_20Y => "20Y",
        _ => "30Y",
    }
}

/// Derive a netting set ID from an OTC margin specification.
///
/// Maps clearing status to the appropriate netting set identifier.
///
/// # Arguments
///
/// * `spec` - OTC margin specification carrying clearing status and bilateral CSA identifier.
#[must_use]
pub fn netting_set_id_from_spec(spec: &OtcMarginSpec) -> NettingSetId {
    match &spec.clearing_status {
        ClearingStatus::Cleared { ccp } => NettingSetId::cleared(ccp),
        _ => NettingSetId::bilateral(&spec.csa.id, &spec.csa.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ir_tenor_buckets_cover_boundaries() {
        assert_eq!(assign_ir_tenor_bucket(0.25), "6M");
        assert_eq!(assign_ir_tenor_bucket(1.0), "1Y");
        assert_eq!(assign_ir_tenor_bucket(5.0), "5Y");
        assert_eq!(assign_ir_tenor_bucket(30.0), "30Y");
    }

    #[test]
    fn netting_set_id_tracks_clearing_status() {
        let bilateral = OtcMarginSpec::usd_bilateral().expect("bilateral spec");
        assert_eq!(
            netting_set_id_from_spec(&bilateral),
            NettingSetId::bilateral(&bilateral.csa.id, &bilateral.csa.id)
        );

        let cleared = OtcMarginSpec::cleared("LCH", finstack_quant_core::currency::Currency::USD)
            .expect("cleared spec");
        assert_eq!(
            netting_set_id_from_spec(&cleared),
            NettingSetId::cleared("LCH")
        );
    }
}
