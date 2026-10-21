//! `Marginable` implementation for [`CreditDefaultSwap`].

use super::CreditDefaultSwap;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_margin::constants::CALENDAR_DAYS_PER_YEAR;
use finstack_quant_margin::constants::DURATION_APPROXIMATION_FACTOR;
use finstack_quant_margin::constants::STANDARD_CDS_MATURITY_YEARS;
use finstack_quant_margin::Marginable;
use finstack_quant_margin::NettingSetId;
use finstack_quant_margin::OtcMarginSpec;
use finstack_quant_margin::SimmSensitivities;
use finstack_quant_valuations_core::instruments::common_impl::margin::add_classified_credit_delta;
use finstack_quant_valuations_core::instruments::common_impl::margin::assign_credit_tenor_bucket;
use finstack_quant_valuations_core::instruments::common_impl::margin::extract_reference_entity;
use finstack_quant_valuations_core::instruments::common_impl::margin::netting_set_id_from_spec;
use finstack_quant_valuations_core::instruments::common_impl::margin::repriced_metric;
use finstack_quant_valuations_core::instruments::{Instrument, PayReceive};

/// Credit terms of a CDS-like trade that drive its SIMM credit delta.
pub(crate) struct CreditSimmTerms<'a> {
    /// SIMM credit qualifier: the reference entity or the index name.
    pub(crate) qualifier: &'a str,
    /// Protection end date, which sets the SIMM tenor bucket.
    pub(crate) maturity: Date,
    /// Recovery rate (decimal) used by the risky-duration fallback.
    pub(crate) recovery_rate: f64,
    /// Trade notional; its currency is the sensitivities' currency.
    pub(crate) notional: Money,
    /// Protection side; buying protection (`Pay`) is long credit risk.
    pub(crate) side: PayReceive,
}

/// SIMM credit delta of a CDS or CDS index: the repriced CS01 metric in its
/// tenor bucket, or a flat risky-duration proxy when CS01 is unavailable.
pub(crate) fn credit_simm_sensitivities<I: Instrument + Marginable>(
    instrument: &I,
    market: &MarketContext,
    as_of: Date,
    terms: CreditSimmTerms<'_>,
) -> Result<SimmSensitivities> {
    let mut sens = SimmSensitivities::new(terms.notional.currency());

    let days_to_maturity = (terms.maturity - as_of).whole_days().max(0) as f64;
    let years_to_maturity = days_to_maturity / CALENDAR_DAYS_PER_YEAR;
    let years_to_maturity = if years_to_maturity <= 0.0 {
        STANDARD_CDS_MATURITY_YEARS
    } else {
        years_to_maturity
    };
    let tenor = assign_credit_tenor_bucket(years_to_maturity);

    // Preferred path: repriced CS01 (respects the survival curve, discount
    // curve, recovery and premium schedule). The metric is computed on the
    // actual trade, so its sign already reflects the protection side.
    if let Some(cs01) = repriced_metric(
        instrument,
        market,
        as_of,
        finstack_quant_valuations_core::metrics::MetricId::Cs01,
    ) {
        add_classified_credit_delta(
            &mut sens,
            Marginable::margin_spec(instrument),
            terms.qualifier,
            tenor,
            cs01,
        )?;
        return Ok(sens);
    }

    // Fallback: flat risky-duration proxy when the survival curve / pricer
    // is unavailable.
    tracing::warn!(
        instrument_id = Marginable::id(instrument),
        "SIMM credit delta falling back to risky-duration proxy: repriced \
         CS01 unavailable (missing survival curve?)"
    );
    let risky_duration =
        years_to_maturity * (1.0 - terms.recovery_rate) * DURATION_APPROXIMATION_FACTOR;
    let cs01 = terms.notional.amount().abs() * risky_duration * ONE_BASIS_POINT;
    let signed_cs01 = match terms.side {
        PayReceive::Pay => cs01,
        PayReceive::Receive => -cs01,
    };
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
                recovery_rate: self.protection_leg.recovery_rate,
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
        use finstack_quant_margin::{SimmCreditClassification, SimmCreditSector};

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

        assert!(sensitivities
            .credit_qualifying_delta
            .keys()
            .any(|(sector, _, _)| *sector == SimmCreditSector::HighYieldFinancial));
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
            .insert(discount_forward_curve_support::flat_discount_with_tenor(
                cds.premium_leg.discount_curve_id.as_str(),
                as_of,
                0.03,
                30.0,
            ))
            .insert(
                HazardCurve::builder(cds.protection_leg.credit_curve_id.as_str())
                    .base_date(as_of)
                    .recovery_rate(cds.protection_leg.recovery_rate)
                    .knots([(1.0, 0.02), (10.0, 0.03)])
                    .build()
                    .expect("hazard"),
            );
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
}
