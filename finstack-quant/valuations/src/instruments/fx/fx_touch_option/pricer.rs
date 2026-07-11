//! FX touch option pricer implementation.

use crate::instruments::common_impl::helpers::zero_rate_from_df;
use crate::instruments::fx::fx_touch_option::types::{
    BarrierDirection, FxTouchOption, PayoutTiming, TouchType,
};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

#[derive(Debug, Clone, Default)]
pub(crate) struct FxTouchOptionCalculator;

pub(crate) fn compute_pv(
    inst: &FxTouchOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    if as_of > inst.expiry {
        return Ok(Money::new(0.0, inst.quote_currency));
    }
    FxTouchOptionCalculator.npv(inst, curves, as_of)
}

impl FxTouchOptionCalculator {
    pub(crate) fn npv(
        &self,
        inst: &FxTouchOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        let (spot, r_d, r_f, sigma, t) = self.collect_inputs(inst, curves, as_of)?;

        if t <= 0.0 {
            let observed_touch = inst.observed_touch.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "Expired FX touch option requires explicit observed touch state".to_string(),
                )
            })?;
            let pv = match (inst.touch_type, observed_touch, inst.payout_timing) {
                (TouchType::OneTouch, true, PayoutTiming::AtHit)
                | (TouchType::OneTouch, false, _)
                | (TouchType::NoTouch, true, _) => 0.0,
                (TouchType::OneTouch, true, PayoutTiming::AtExpiry)
                | (TouchType::NoTouch, false, _) => inst.payout_amount.amount(),
            };
            return Ok(Money::new(pv, inst.quote_currency));
        }

        if inst.observed_touch == Some(true) {
            let pv = match (inst.touch_type, inst.payout_timing) {
                (TouchType::OneTouch, PayoutTiming::AtHit) | (TouchType::NoTouch, _) => 0.0,
                (TouchType::OneTouch, PayoutTiming::AtExpiry) => {
                    (-r_d * t).exp() * inst.payout_amount.amount()
                }
            };
            return Ok(Money::new(pv, inst.quote_currency));
        }

        let price = price_touch(
            inst,
            spot,
            inst.barrier_level,
            r_d,
            r_f,
            sigma,
            t,
            inst.touch_type,
            inst.barrier_direction,
            inst.payout_timing,
            inst.payout_amount.amount(),
        )?;

        Ok(Money::new(price, inst.quote_currency))
    }

    pub(crate) fn collect_inputs(
        &self,
        inst: &FxTouchOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<(f64, f64, f64, f64, f64)> {
        if as_of >= inst.expiry {
            return self.collect_inputs_expired(inst, curves, as_of);
        }

        let domestic_disc = curves.get_discount(inst.domestic_discount_curve_id.as_str())?;
        let foreign_disc = curves.get_discount(inst.foreign_discount_curve_id.as_str())?;

        let t_vol = inst
            .day_count
            .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

        // Date-based DF lookups; rates are derived to satisfy
        // `exp(-r * t_vol) = df` so day-count differences between the curves
        // and the vol surface are absorbed into `r`.
        let df_d = domestic_disc.df_between_dates(as_of, inst.expiry)?;
        let df_f = foreign_disc.df_between_dates(as_of, inst.expiry)?;

        let r_d = zero_rate_from_df(df_d, t_vol, "FxTouchOption domestic discount")?;
        let r_f = zero_rate_from_df(df_f, t_vol, "FxTouchOption foreign discount")?;

        let fx_matrix = curves.fx().ok_or(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::NotFound {
                id: "fx_matrix".to_string(),
            },
        ))?;
        let spot = fx_matrix
            .rate(FxQuery::new(inst.base_currency, inst.quote_currency, as_of))?
            .rate;

        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &inst.pricing_overrides.market_quotes,
            curves,
            inst.vol_surface_id.as_str(),
            t_vol,
            inst.barrier_level,
        )?;

        Ok((spot, r_d, r_f, sigma, t_vol))
    }

    fn collect_inputs_expired(
        &self,
        inst: &FxTouchOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<(f64, f64, f64, f64, f64)> {
        let fx_matrix = curves.fx().ok_or(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::NotFound {
                id: "fx_matrix".to_string(),
            },
        ))?;
        let spot = fx_matrix
            .rate(FxQuery::new(inst.base_currency, inst.quote_currency, as_of))?
            .rate;
        Ok((spot, 0.0, 0.0, 0.0, 0.0))
    }
}

#[allow(clippy::too_many_arguments)]
fn price_touch(
    inst: &FxTouchOption,
    spot: f64,
    barrier: f64,
    r_d: f64,
    r_f: f64,
    sigma: f64,
    t: f64,
    touch_type: TouchType,
    barrier_direction: BarrierDirection,
    payout_timing: PayoutTiming,
    payout: f64,
) -> Result<f64> {
    let already_breached = match barrier_direction {
        BarrierDirection::Down => spot <= barrier,
        BarrierDirection::Up => spot >= barrier,
    };
    if already_breached {
        return Ok(match touch_type {
            TouchType::OneTouch => match payout_timing {
                PayoutTiming::AtHit => payout,
                PayoutTiming::AtExpiry => (-r_d * t).exp() * payout,
            },
            TouchType::NoTouch => 0.0,
        });
    }

    let sigma2 = sigma * sigma;
    let sqrt_t = t.sqrt();
    let sigma_sqrt_t = sigma * sqrt_t;
    if sigma_sqrt_t <= 0.0 || t <= 0.0 {
        return Ok(0.0);
    }

    let mu = (r_d - r_f - sigma2 / 2.0) / sigma2;
    // A no-touch can only ever settle at expiry, so its value uses the
    // *undiscounted* touch probability (survival-to-expiry) regardless of
    // `payout_timing`. Only an at-hit one-touch folds hit-time discounting into
    // the touch probability (the extra 2·r_d/σ² term in λ).
    let lambda_r = match (touch_type, payout_timing) {
        (TouchType::NoTouch, _) | (TouchType::OneTouch, PayoutTiming::AtExpiry) => 0.0,
        (TouchType::OneTouch, PayoutTiming::AtHit) => r_d,
    };
    let lambda_sq = mu * mu + 2.0 * lambda_r / sigma2;
    if lambda_sq < 0.0 {
        // mu^2 + 2*r_d/sigma^2 < 0 requires r_d very negative relative to sigma^2.
        // Surfacing this as an error beats silently returning 0 — it tells the
        // caller their rate environment falls outside the closed-form's domain.
        return Err(finstack_quant_core::Error::Validation(format!(
            "FxTouchOption {}: lambda^2 = {lambda_sq:.6e} < 0 (r_d={r_d}, sigma={sigma}, \
             at-hit={:?}); closed-form Rubinstein-Reiner is undefined here",
            inst.id, payout_timing,
        )));
    }
    let lambda = lambda_sq.sqrt();

    let log_hs = (barrier / spot).ln();
    let z = log_hs / sigma_sqrt_t + lambda * sigma_sqrt_t;
    let z_prime = log_hs / sigma_sqrt_t - lambda * sigma_sqrt_t;
    let eta = match barrier_direction {
        BarrierDirection::Down => 1.0,
        BarrierDirection::Up => -1.0,
    };
    let s_over_h = spot / barrier;
    let power1 = s_over_h.powf(-(mu + lambda));
    let power2 = s_over_h.powf(-(mu - lambda));
    let n_eta_z = finstack_quant_core::math::norm_cdf(eta * z);
    let n_eta_z_prime = finstack_quant_core::math::norm_cdf(eta * z_prime);
    let one_touch_prob = power1 * n_eta_z + power2 * n_eta_z_prime;

    let df = (-r_d * t).exp();
    Ok(match touch_type {
        TouchType::OneTouch => match payout_timing {
            // `one_touch_prob` already includes hit-time discounting (λ uses r_d).
            PayoutTiming::AtHit => payout * one_touch_prob,
            // Undiscounted touch probability, then discount the expiry payout.
            PayoutTiming::AtExpiry => df * payout * one_touch_prob,
        },
        // A no-touch pays at expiry iff the barrier is never hit:
        //   payout · e^{-r_d·T} · P(no touch) = df · payout · (1 − P_touch),
        // with P_touch the undiscounted touch probability (λ uses lambda_r = 0).
        TouchType::NoTouch => df * payout * (1.0 - one_touch_prob),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::instruments::PricingOverrides;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use std::sync::Arc;
    use time::macros::date;

    fn build_market(as_of: Date) -> MarketContext {
        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .build()
            .expect("usd curve");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, (-0.01_f64).exp())])
            .build()
            .expect("eur curve");
        let vol_surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[0.9, 1.0, 1.1, 1.2, 1.3])
            .row(&[0.15; 5])
            .row(&[0.15; 5])
            .row(&[0.15; 5])
            .row(&[0.15; 5])
            .build()
            .expect("vol surface");
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.20)
            .expect("valid rate");
        let fx_matrix = FxMatrix::new(Arc::new(provider));

        MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_surface(vol_surface)
            .insert_fx(fx_matrix)
    }

    fn build_option(expiry: Date) -> FxTouchOption {
        FxTouchOption::builder()
            .id(InstrumentId::new("FX-TOUCH-TEST"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .barrier_level(1.10)
            .touch_type(crate::instruments::fx::fx_touch_option::TouchType::OneTouch)
            .barrier_direction(crate::instruments::fx::fx_touch_option::BarrierDirection::Down)
            .payout_amount(Money::new(100_000.0, Currency::USD))
            .payout_timing(crate::instruments::fx::fx_touch_option::PayoutTiming::AtExpiry)
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("fx touch option")
    }

    #[test]
    fn fx_touch_pricer_compute_pv_matches_instrument_value() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let option = build_option(expiry);
        let market = build_market(as_of);

        let via_pricer = compute_pv(&option, &market, as_of).expect("pricer pv");
        let via_instrument = option.value(&market, as_of).expect("instrument pv");

        assert!((via_pricer.amount() - via_instrument.amount()).abs() < 1e-10);
        assert_eq!(via_pricer.currency(), via_instrument.currency());
    }

    fn build_no_touch(
        expiry: Date,
        timing: crate::instruments::fx::fx_touch_option::PayoutTiming,
    ) -> FxTouchOption {
        FxTouchOption::builder()
            .id(InstrumentId::new("FX-NOTOUCH-TEST"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .barrier_level(1.10)
            .touch_type(crate::instruments::fx::fx_touch_option::TouchType::NoTouch)
            .barrier_direction(crate::instruments::fx::fx_touch_option::BarrierDirection::Down)
            .payout_amount(Money::new(100_000.0, Currency::USD))
            .payout_timing(timing)
            .expiry(expiry)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("fx no-touch option")
    }

    /// A no-touch option pays only at expiry (if the barrier is never hit), so its
    /// value must be independent of `payout_timing`. The at-hit branch previously
    /// discounted the touch probability at the (earlier) hit time, understating
    /// the no-touch value.
    #[test]
    fn no_touch_value_is_independent_of_payout_timing() {
        use crate::instruments::fx::fx_touch_option::PayoutTiming;
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);
        let market = build_market(as_of);

        let pv_expiry = compute_pv(
            &build_no_touch(expiry, PayoutTiming::AtExpiry),
            &market,
            as_of,
        )
        .expect("no-touch at-expiry pv");
        let pv_hit = compute_pv(&build_no_touch(expiry, PayoutTiming::AtHit), &market, as_of)
            .expect("no-touch at-hit pv");

        assert!(
            (pv_expiry.amount() - pv_hit.amount()).abs() < 1e-9,
            "no-touch value must not depend on payout timing: at_expiry={} at_hit={}",
            pv_expiry.amount(),
            pv_hit.amount()
        );
        assert!(
            pv_expiry.amount() > 0.0,
            "no-touch should have positive value"
        );
    }
}
