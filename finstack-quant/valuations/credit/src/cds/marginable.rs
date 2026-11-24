//! `Marginable` implementation for [`CreditDefaultSwap`].

use super::CreditDefaultSwap;
use finstack_quant_core::Result;
use finstack_quant_core::dates::CALENDAR_DAYS_PER_YEAR;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_core::money::Money;
use finstack_quant_margin::Marginable;
use finstack_quant_margin::constants::DURATION_APPROXIMATION_FACTOR;
use finstack_quant_margin::types::NettingSetId;
use finstack_quant_margin::types::OtcMarginSpec;
use finstack_quant_margin::types::SimmCreditClassification;
use finstack_quant_margin::types::SimmSensitivities;
use finstack_quant_valuations_core::instruments::{Instrument, PayReceive};
use finstack_quant_valuations_core::margin::netting_set_id_from_spec;

fn repriced_metric(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    metric: finstack_quant_valuations_core::metrics::MetricId,
) -> Option<f64> {
    let result = instrument
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&metric),
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .ok()?;
    result.measures.get(metric.as_str()).copied()
}

fn assign_credit_tenor_bucket(years_to_maturity: f64) -> &'static str {
    use finstack_quant_margin::constants::tenor_buckets::*;
    match years_to_maturity {
        y if y <= BUCKET_1Y => "1Y",
        y if y <= BUCKET_2Y => "2Y",
        y if y <= BUCKET_3Y => "3Y",
        y if y <= BUCKET_5Y => "5Y",
        y if y <= BUCKET_10Y => "10Y",
        _ => "15Y",
    }
}

fn extract_reference_entity(credit_curve_id: &str) -> Result<&str> {
    if credit_curve_id.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "credit curve id is empty".to_string(),
        ));
    }

    let is_tenor = |s: &str| {
        let last = s.chars().next_back().unwrap_or(' ');
        matches!(last, 'Y' | 'M' | 'W' | 'D')
            && s[..s.len() - 1].chars().all(|c| c.is_ascii_digit())
            && s.len() >= 2
    };
    const CURVE_TYPES: &[&str] = &[
        "CDS", "BOND", "CURVE", "OIS", "SOFR", "ESTR", "EURIBOR", "LIBOR", "HAZARD", "SPREAD",
    ];
    let is_curve_type = |s: &str| CURVE_TYPES.contains(&s);

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
        tracing::warn!(
            credit_curve_id,
            "credit curve id has no recognizable entity prefix; using full string"
        );
        return Ok(credit_curve_id);
    }
    Ok(remaining)
}

fn add_classified_credit_delta(
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

/// Credit terms of a CDS-like trade that drive its SIMM credit delta.
pub(crate) struct CreditSimmTerms<'a> {
    /// SIMM credit qualifier: the reference entity or the index name.
    pub(crate) qualifier: &'a str,
    /// Protection end date, which sets the SIMM tenor bucket.
    pub(crate) maturity: Date,
    /// Trade notional; its currency is the sensitivities' currency.
    pub(crate) notional: Money,
    /// Protection side; buying protection (`Pay`) is long credit risk.
    pub(crate) side: PayReceive,
}

/// SIMM credit delta of a CDS or CDS index: the repriced `RiskyPv01` metric
/// signed by protection side in its tenor bucket, or a flat risky-duration
/// proxy when curves are unavailable.
pub(crate) fn credit_simm_sensitivities<I: Instrument + Marginable>(
    instrument: &I,
    market: &MarketContext,
    as_of: Date,
    terms: CreditSimmTerms<'_>,
) -> Result<SimmSensitivities> {
    let mut sens = SimmSensitivities::new(terms.notional.currency());

    let raw_days_to_maturity = (terms.maturity - as_of).whole_days();
    if raw_days_to_maturity <= 0 {
        return Ok(sens);
    }

    let years_to_maturity = (raw_days_to_maturity as f64) / CALENDAR_DAYS_PER_YEAR;
    let tenor = assign_credit_tenor_bucket(years_to_maturity);
    let side_sign = match terms.side {
        PayReceive::Pay => 1.0,
        PayReceive::Receive => -1.0,
    };

    // Preferred path: repriced RiskyPv01 (respects survival curve, discount
    // curve, recovery and premium schedule without requiring a recalibration
    // provider).
    if let Some(rpv01) = repriced_metric(
        instrument,
        market,
        as_of,
        finstack_quant_valuations_core::metrics::MetricId::RiskyPv01,
    ) {
        add_classified_credit_delta(
            &mut sens,
            Marginable::margin_spec(instrument),
            terms.qualifier,
            tenor,
            side_sign * rpv01,
        )?;
        return Ok(sens);
    }

    // Fallback: flat risky-duration proxy when curves are unavailable.
    // For par-spread sensitivity CS01, (1 - R) cancels between hazard
    // calibration and protection PV, leaving duration * notional * 1bp.
    tracing::warn!(
        instrument_id = Marginable::id(instrument),
        "SIMM credit delta falling back to risky-duration proxy: repriced \
         RiskyPv01 unavailable"
    );
    let risky_duration = years_to_maturity * DURATION_APPROXIMATION_FACTOR;
    let signed_cs01 = side_sign * terms.notional.amount().abs() * risky_duration * ONE_BASIS_POINT;
    add_classified_credit_delta(
        &mut sens,
        instrument.margin_spec(),
        terms.qualifier,
        tenor,
        signed_cs01,
    )?;

    Ok(sens)
}

impl Marginable for CreditDefaultSwap {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn margin_spec(&self) -> Option<&OtcMarginSpec> {
        self.margin_spec.as_ref()
    }

    fn netting_set_id(&self) -> Option<NettingSetId> {
        self.margin_spec.as_ref().map(netting_set_id_from_spec)
    }

    fn simm_sensitivities(&self, market: &MarketContext, as_of: Date) -> Result<SimmSensitivities> {
        let qualifier = extract_reference_entity(self.protection_leg.credit_curve_id.as_str())?;
        credit_simm_sensitivities(
            self,
            market,
            as_of,
            CreditSimmTerms {
                qualifier,
                maturity: self.premium_leg.end,
                notional: self.notional,
                side: self.side,
            },
        )
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        Instrument::value(self, market, as_of)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_valuations_test_support::discount_forward_curves as discount_forward_curve_support;

    #[test]
    fn cds_marginable_uses_explicit_sector_classification() {
        use finstack_quant_margin::types::{SimmCreditClassification, SimmCreditSector};

        let mut cds = CreditDefaultSwap::example().expect("example");
        cds.margin_spec = Some(
            OtcMarginSpec::usd_bilateral()
                .expect("margin spec")
                .with_simm_credit_classification(SimmCreditClassification::Qualifying {
                    sector: SimmCreditSector::HighYieldFinancial,
                }),
        );
        let as_of = cds.premium_leg.start;

        let sensitivities = cds
            .simm_sensitivities(&MarketContext::new(), as_of)
            .expect("CDS sensitivities");

        assert!(
            sensitivities
                .credit_qualifying_delta
                .keys()
                .any(|(sector, _, _)| *sector == SimmCreditSector::HighYieldFinancial)
        );
        assert!(sensitivities.credit_non_qualifying_delta.is_empty());
    }

    #[test]
    fn variation_margin_inherits_clean_upfront_side_and_scenario_policy() {
        use finstack_quant_core::market_data::term_structures::HazardCurve;
        use finstack_quant_valuations_core::instruments::PayReceive;
        use finstack_quant_valuations_core::market::conventions::CdsValuationConvention;
        let as_of = time::macros::date!(2027 - 02 - 15);
        let mut cds = CreditDefaultSwap::example().expect("CDS");
        let market = MarketContext::new()
            .try_insert(discount_forward_curve_support::flat_discount_with_tenor(
                cds.premium_leg.discount_curve_id.as_str(),
                as_of,
                0.03,
                30.0,
            ))
            .expect("insert market curve")
            .try_insert(
                HazardCurve::builder(cds.protection_leg.credit_curve_id.as_str())
                    .base_date(as_of)
                    .recovery_rate(cds.protection_leg.recovery_rate)
                    .knots([(1.0, 0.02), (10.0, 0.03)])
                    .day_count(finstack_quant_core::dates::DayCount::Act365F)
                    .build()
                    .expect("hazard"),
            )
            .expect("insert market curve");
        cds.upfront = Some((
            as_of + time::Duration::days(2),
            Money::new(12_345.678_9, Currency::USD).expect("upfront"),
        ));
        for side in [PayReceive::Pay, PayReceive::Receive] {
            for convention in [
                CdsValuationConvention::IsdaDirty,
                CdsValuationConvention::BloombergCdswClean,
            ] {
                for shock in [None, Some(-0.05)] {
                    cds.side = side;
                    cds.valuation_convention = convention;
                    cds.scenario_pricing_overrides.scenario_price_shock_decimal = shock;
                    assert_eq!(
                        cds.mtm_for_vm(&market, as_of).expect("VM"),
                        cds.value(&market, as_of).expect("value")
                    );
                }
            }
        }
        cds.upfront = Some((as_of, Money::from((1_i64, Currency::EUR))));
        assert!(cds.value(&market, as_of).is_err());
        assert!(cds.mtm_for_vm(&market, as_of).is_err());
    }

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

    #[test]
    fn expired_cds_returns_empty_simm_sensitivities() {
        use finstack_quant_margin::types::{SimmCreditClassification, SimmCreditSector};

        let mut cds = CreditDefaultSwap::example().expect("example");
        cds.margin_spec = Some(
            OtcMarginSpec::usd_bilateral()
                .expect("margin spec")
                .with_simm_credit_classification(SimmCreditClassification::Qualifying {
                    sector: SimmCreditSector::Residual,
                }),
        );
        let at_expiry = cds
            .simm_sensitivities(&MarketContext::new(), cds.premium_leg.end)
            .expect("at expiry");
        assert!(
            at_expiry.is_empty(),
            "expired CDS must have zero SIMM sensitivity"
        );

        let post_expiry = cds
            .simm_sensitivities(
                &MarketContext::new(),
                cds.premium_leg.end + time::Duration::days(30),
            )
            .expect("post expiry");
        assert!(
            post_expiry.is_empty(),
            "post-expiry CDS must have zero SIMM sensitivity"
        );
    }

    #[test]
    fn live_cds_simm_uses_repriced_risky_pv01_and_proxy_drops_one_minus_r() {
        use finstack_quant_core::market_data::term_structures::HazardCurve;
        use finstack_quant_margin::types::{SimmCreditClassification, SimmCreditSector};

        let as_of = time::macros::date!(2024 - 03 - 20);
        let mut cds = CreditDefaultSwap::example().expect("example");
        cds.margin_spec = Some(
            OtcMarginSpec::usd_bilateral()
                .expect("margin spec")
                .with_simm_credit_classification(SimmCreditClassification::Qualifying {
                    sector: SimmCreditSector::Residual,
                }),
        );
        let market = MarketContext::new()
            .try_insert(discount_forward_curve_support::flat_discount_with_tenor(
                cds.premium_leg.discount_curve_id.as_str(),
                as_of,
                0.03,
                10.0,
            ))
            .expect("discount")
            .try_insert(
                HazardCurve::builder(cds.protection_leg.credit_curve_id.as_str())
                    .base_date(as_of)
                    .recovery_rate(cds.protection_leg.recovery_rate)
                    .knots([(1.0, 0.015), (10.0, 0.02)])
                    .day_count(finstack_quant_core::dates::DayCount::Act365F)
                    .build()
                    .expect("hazard"),
            )
            .expect("hazard insert");

        let expected_rpv01 = repriced_metric(
            &cds,
            &market,
            as_of,
            finstack_quant_valuations_core::metrics::MetricId::RiskyPv01,
        )
        .expect("repriced RiskyPv01 must succeed without recalibration provider");
        let sens = cds.simm_sensitivities(&market, as_of).expect("simm");
        let (&(_, _, _), &delta) = sens
            .credit_qualifying_delta
            .iter()
            .next()
            .expect("delta entry");
        assert!(
            (delta - expected_rpv01).abs() < 1e-10,
            "expected repriced RiskyPv01 {expected_rpv01}, got {delta}"
        );

        // Curve-less fallback must ignore recovery_rate (1 - R cancels in spread CS01).
        let sens_r40 = cds
            .simm_sensitivities(&MarketContext::new(), as_of)
            .expect("fallback r=0.40");
        cds.protection_leg.recovery_rate = 0.20;
        let sens_r20 = cds
            .simm_sensitivities(&MarketContext::new(), as_of)
            .expect("fallback r=0.20");
        assert_eq!(
            sens_r40.credit_qualifying_delta, sens_r20.credit_qualifying_delta,
            "CS01 fallback proxy must not depend on recovery rate"
        );
    }
}
