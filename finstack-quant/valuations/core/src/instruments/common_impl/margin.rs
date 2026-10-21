//! Shared helpers for the instruments' [`Marginable`](finstack_quant_margin::Marginable) implementations.
//!
//! Each instrument family implements `finstack_quant_margin::Marginable`
//! beside its instrument type (`<instrument>/marginable.rs`) and reaches
//! it through `Instrument::as_marginable`; the repricing, tenor-bucketing,
//! reference-entity and netting-set helpers they share live here.

use crate::instruments::Instrument;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use finstack_quant_margin::constants;
use finstack_quant_margin::types::ClearingStatus;
use finstack_quant_margin::types::NettingSetId;
use finstack_quant_margin::types::OtcMarginSpec;
use finstack_quant_margin::types::SimmCreditClassification;
use finstack_quant_margin::types::SimmSensitivities;

/// Reprice an instrument and return a single scalar metric from its measures.
///
/// Used by the SIMM sensitivity impls to obtain a **repriced** DV01 / CS01
/// (which respects the discount curve, coupon schedule and curve shape) instead
/// of a flat `duration ≈ maturity` proxy. Returns `None` if pricing fails or
/// the metric is absent, so callers can fall back to the proxy.
pub fn repriced_metric(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    metric: crate::metrics::MetricId,
) -> Option<f64> {
    let result = instrument
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&metric),
            crate::instruments::PricingOptions::default(),
        )
        .ok()?;
    result.measures.get(metric.as_str()).copied()
}

/// Reprice an instrument and return the per-tenor `BucketedDv01` series for the
/// unique curves as `(tenor_years, dv01)` pairs.
///
/// The `BucketedDv01` calculator flattens its per-tenor series into `measures`
/// under composite keys `bucketed_dv01::{curve}::{tenor_label}`. Returns an
/// empty vec when no per-tenor series is available.
pub fn repriced_bucketed_dv01(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    curve_ids: &[finstack_quant_core::types::CurveId],
) -> Vec<(f64, f64)> {
    let Ok(result) = instrument.price_with_metrics(
        market,
        as_of,
        &[crate::metrics::MetricId::BucketedDv01],
        crate::instruments::PricingOptions::default(),
    ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for curve_id in curve_ids {
        for (&tenor, label) in crate::metrics::STANDARD_BUCKETS_YEARS
            .iter()
            .zip(crate::metrics::STANDARD_BUCKET_LABELS.iter())
        {
            let key = crate::metrics::MetricId::composite(
                &crate::metrics::MetricId::BucketedDv01,
                &[curve_id.as_str(), label],
            );
            if let Some(&dv01) = result.measures.get(key.as_str()) {
                out.push((tenor, dv01));
            }
        }
    }
    out
}

/// Assign a years-to-maturity value to the appropriate SIMM credit tenor bucket.
#[must_use]
pub fn assign_credit_tenor_bucket(years_to_maturity: f64) -> &'static str {
    use constants::tenor_buckets::*;
    match years_to_maturity {
        y if y <= BUCKET_1Y => "1Y",
        y if y <= BUCKET_2Y => "2Y",
        y if y <= BUCKET_3Y => "3Y",
        y if y <= BUCKET_5Y => "5Y",
        y if y <= BUCKET_10Y => "10Y",
        _ => "15Y",
    }
}

/// Assign a years-to-maturity value to the appropriate SIMM IR tenor bucket.
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

/// Extract reference entity from a credit curve ID.
///
/// Expects format `<ENTITY>-<TYPE>[-<TENOR>]` where ENTITY may contain dashes
/// (e.g. `GOLDMAN-SACHS-CDS-5Y` → `GOLDMAN-SACHS`). Strips a recognized tenor
/// suffix (e.g. `-5Y`, `-10Y`) and curve-type suffix (`-CDS`, `-BOND`,
/// `-CURVE`, `-OIS`) from the right; whatever remains is the entity name.
/// Inputs without a recognized suffix return the whole string with a warning.
pub fn extract_reference_entity(credit_curve_id: &str) -> Result<&str> {
    if credit_curve_id.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "credit curve id is empty".to_string(),
        ));
    }

    // Recognized tenor pattern: an integer followed by Y/M/W/D
    let is_tenor = |s: &str| {
        let last = s.chars().next_back().unwrap_or(' ');
        matches!(last, 'Y' | 'M' | 'W' | 'D')
            && s[..s.len() - 1].chars().all(|c| c.is_ascii_digit())
            && s.len() >= 2
    };
    // Recognized curve types
    const CURVE_TYPES: &[&str] = &[
        "CDS", "BOND", "CURVE", "OIS", "SOFR", "ESTR", "EURIBOR", "LIBOR", "HAZARD", "SPREAD",
    ];
    let is_curve_type = |s: &str| CURVE_TYPES.contains(&s);

    // Strip up to two suffixes from the right: tenor first, then curve type.
    let mut remaining = credit_curve_id;
    for _ in 0..2 {
        if let Some(pos) = remaining.rfind('-') {
            let suffix = &remaining[pos + 1..];
            if is_tenor(suffix) || is_curve_type(suffix) {
                remaining = &remaining[..pos];
                continue;
            }
        }
        break;
    }

    if remaining.is_empty() {
        // Pathological input like "-CDS-5Y" — fall back to the whole string.
        tracing::warn!(
            credit_curve_id,
            "credit curve id has no recognizable entity prefix; using full string"
        );
        return Ok(credit_curve_id);
    }
    Ok(remaining)
}

pub fn add_classified_credit_delta(
    sensitivities: &mut SimmSensitivities,
    margin_spec: Option<&OtcMarginSpec>,
    name: &str,
    tenor: &str,
    amount: f64,
) -> Result<()> {
    match margin_spec.and_then(|spec| spec.simm_credit_classification) {
        Some(SimmCreditClassification::Qualifying { sector }) => {
            sensitivities.add_credit_qualifying_delta(sector, name, tenor, amount);
        }
        Some(SimmCreditClassification::NonQualifying) => {
            sensitivities.add_credit_non_qualifying_delta(name, tenor, amount);
        }
        None => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "credit instrument '{name}' requires OtcMarginSpec.simm_credit_classification"
            )));
        }
        Some(_) => {
            return Err(finstack_quant_core::Error::Validation(
                "unsupported SIMM credit classification".to_string(),
            ));
        }
    }
    Ok(())
}

/// Derive a netting set ID from an OTC margin specification.
///
/// Maps clearing status to the appropriate netting set identifier.
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
    use crate::instruments::common_impl::margin::add_classified_credit_delta;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn explicit_qualifying_classification_routes_credit_delta_to_sector_bucket() {
        use finstack_quant_margin::types::{SimmCreditClassification, SimmCreditSector};

        let spec = OtcMarginSpec::usd_bilateral()
            .expect("margin spec")
            .with_simm_credit_classification(SimmCreditClassification::Qualifying {
                sector: SimmCreditSector::HighYieldFinancial,
            });
        let mut sensitivities = SimmSensitivities::new(Currency::USD);

        add_classified_credit_delta(&mut sensitivities, Some(&spec), "CDX.NA.HY", "5Y", 12_500.0)
            .expect("classified delta");

        assert_eq!(
            sensitivities.credit_qualifying_delta.get(&(
                SimmCreditSector::HighYieldFinancial,
                "CDX.NA.HY".to_string(),
                "5Y".to_string(),
            )),
            Some(&12_500.0)
        );
        assert!(sensitivities.credit_non_qualifying_delta.is_empty());
    }

    #[test]
    fn explicit_non_qualifying_classification_routes_credit_delta_to_cnq() {
        use finstack_quant_margin::types::SimmCreditClassification;

        let spec = OtcMarginSpec::usd_bilateral()
            .expect("margin spec")
            .with_simm_credit_classification(SimmCreditClassification::NonQualifying);
        let mut sensitivities = SimmSensitivities::new(Currency::USD);

        add_classified_credit_delta(&mut sensitivities, Some(&spec), "RMBS-1", "5Y", 8_000.0)
            .expect("classified delta");

        assert_eq!(
            sensitivities
                .credit_non_qualifying_delta
                .get(&("RMBS-1".to_string(), "5Y".to_string())),
            Some(&8_000.0)
        );
        assert!(sensitivities.credit_qualifying_delta.is_empty());
    }

    #[test]
    fn missing_classification_rejects_credit_delta_generation() {
        let mut sensitivities = SimmSensitivities::new(Currency::USD);

        let error = add_classified_credit_delta(
            &mut sensitivities,
            None,
            "UNCLASSIFIED-CDS",
            "5Y",
            5_000.0,
        )
        .expect_err("classification is mandatory");

        assert!(error.to_string().contains("simm_credit_classification"));
        assert!(sensitivities.is_empty());
    }
}
