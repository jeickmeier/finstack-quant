//! `Marginable` implementation for [`EquityTotalReturnSwap`].

use super::EquityTotalReturnSwap;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_margin::Marginable;
use finstack_quant_margin::types::NettingSetId;
use finstack_quant_margin::types::OtcMarginSpec;
use finstack_quant_margin::types::SimmSensitivities;
use finstack_quant_valuations_core::instruments::{Instrument, PayReceive, PricingOptions};
use finstack_quant_valuations_core::margin::{assign_ir_tenor_bucket, netting_set_id_from_spec};
use finstack_quant_valuations_core::metrics::{
    MetricId, STANDARD_BUCKET_LABELS, STANDARD_BUCKETS_YEARS,
};

impl Marginable for EquityTotalReturnSwap {
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
        let currency = self.notional.currency();
        let mut sens = SimmSensitivities::new(currency);

        let last_payment = self
            .schedule
            .payment_date_for(self.schedule.end)
            .unwrap_or(self.schedule.end);
        if as_of >= last_payment {
            return Ok(sens);
        }

        let underlier = &self.underlying.ticker;
        if let Ok(result) = self.price_with_metrics(
            market,
            as_of,
            &[MetricId::IndexDelta, MetricId::BucketedDv01],
            PricingOptions::default(),
        ) {
            if as_of < self.schedule.end
                && let Some(&index_delta) = result.measures.get(&MetricId::IndexDelta)
                && let Ok(scalar) = market.get_price(&self.underlying.spot_id)
            {
                let spot = finstack_quant_valuations_core::instruments::pricing::market_inputs::scalar_price_amount(
                    scalar,
                    currency,
                )?;
                let eq_delta_1pct = index_delta * spot * 0.01;
                if eq_delta_1pct.is_finite() && eq_delta_1pct != 0.0 {
                    sens.add_equity_delta(underlier, eq_delta_1pct);
                }
            }

            let mut bucket_totals: std::collections::BTreeMap<&'static str, f64> =
                std::collections::BTreeMap::new();
            let curves = [
                &self.financing_leg.discount_curve_id,
                &self.financing_leg.forward_curve_id,
            ];
            for curve_id in curves {
                for (&tenor, label) in STANDARD_BUCKETS_YEARS
                    .iter()
                    .zip(STANDARD_BUCKET_LABELS.iter())
                {
                    let key =
                        MetricId::composite(&MetricId::BucketedDv01, &[curve_id.as_str(), label]);
                    if let Some(&dv01) = result.measures.get(&key) {
                        *bucket_totals
                            .entry(assign_ir_tenor_bucket(tenor))
                            .or_insert(0.0) += dv01;
                    }
                }
            }
            for (bucket, dv01) in bucket_totals {
                if dv01.is_finite() && dv01 != 0.0 {
                    sens.add_ir_delta(currency, bucket, dv01);
                }
            }
            return Ok(sens);
        }

        if as_of >= self.schedule.end {
            return Ok(sens);
        }

        tracing::warn!(
            instrument_id = self.id.as_str(),
            "SIMM equity TRS sensitivities falling back to static notional delta: repriced \
             IndexDelta/BucketedDv01 unavailable (missing market inputs?)"
        );
        // SIMM delta is P&L per 1% relative underlying move, not full notional.
        let delta = match self.side {
            PayReceive::Receive => self.notional.amount(),
            PayReceive::Pay => -self.notional.amount(),
        };
        sens.add_equity_delta(underlier, delta * 0.01);

        Ok(sens)
    }

    fn mtm_for_vm(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        use finstack_quant_valuations_core::instruments::Instrument;
        self.value(market, as_of)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::math::interp::InterpStyle;
    use time::Month;

    fn d(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid date")
    }

    fn simm_market(as_of: Date) -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, (-0.04 * 2.0_f64).exp())])
            .interp(InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve");
        let fwd = ForwardCurve::from_discount_curve(
            "USD-SOFR-3M",
            &disc,
            Some(finstack_quant_core::dates::Tenor::quarterly()),
        );
        MarketContext::new()
            .try_insert(disc)
            .expect("disc")
            .try_insert(fwd)
            .expect("fwd")
            .insert_price("SPX-SPOT", MarketScalar::unitless(105.0).expect("spot"))
            .insert_price("SPX-DIV", MarketScalar::unitless(0.02).expect("div"))
    }

    #[test]
    fn simm_sensitivities_respect_lifecycle() {
        let mut trs = EquityTotalReturnSwap::example().expect("example");
        trs.initial_level = Some(100.0);
        trs.underlying.div_yield_id = None;

        let live_as_of = trs.schedule.start;
        let live_sens = trs
            .simm_sensitivities(&simm_market(live_as_of), live_as_of)
            .expect("live simm");
        let eq_delta = live_sens.equity_delta.get("SPX").copied().unwrap_or(0.0);
        assert!(
            (eq_delta - trs.notional.amount() * 0.01 * (105.0 / 100.0)).abs()
                < 1e-4 * trs.notional.amount(),
            "unexpected live equity SIMM delta: {eq_delta}"
        );
        assert!(
            !live_sens.ir_delta.is_empty(),
            "live TRS should emit IR delta"
        );

        let fwd_as_of = d(2023, Month::December, 1);
        let fwd_sens = trs
            .simm_sensitivities(&simm_market(fwd_as_of), fwd_as_of)
            .expect("fwd simm");
        assert!(
            fwd_sens.equity_delta.is_empty(),
            "forward-starting TRS should have zero equity delta, got {:?}",
            fwd_sens.equity_delta
        );

        let expired_as_of = trs
            .schedule
            .payment_date_for(trs.schedule.end)
            .expect("payment date");
        let expired_sens = trs
            .simm_sensitivities(&simm_market(expired_as_of), expired_as_of)
            .expect("expired simm");
        assert!(expired_sens.equity_delta.is_empty());
        assert!(expired_sens.ir_delta.is_empty());
    }
}
