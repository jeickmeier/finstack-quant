//! Quanto option pricers.
//!
//! Only analytical pricing is supported. Monte Carlo pricing has been removed
//! because the quanto drift adjustment model cannot be correctly represented
//! in a simple 1D MC simulation without a 2D correlated equity/FX process.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::quanto_option::types::QuantoOption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

// ========================= ANALYTICAL PRICER =========================

use crate::models::closed_form::quanto::{quanto_call, quanto_put};

/// Helper to collect inputs for quanto option pricing.
fn collect_quanto_inputs(
    inst: &QuantoOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<(f64, f64, f64, f64, f64, f64, f64)> {
    use crate::instruments::common_impl::helpers::zero_rate_from_df;

    let t = inst
        .day_count
        .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

    // Recover continuously-compounded rates from *date-based* discount factors,
    // mirroring `shared.rs::collect_fx_option_inputs_no_vol`. `curve.zero(t)`
    // interpolates on the curve's own time axis, so the rate it returns does
    // not satisfy `e^{-r·t} = df_between_dates(as_of, expiry)` whenever
    // `as_of != base_date` or the instrument and curve day-counts differ.
    // Using `df_between_dates` + `zero_rate_from_df` keeps the recovered rate
    // consistent with the actual discount factor over the option's life.
    let disc_curve = curves.get_discount(inst.domestic_discount_curve_id.as_str())?;
    let df_dom = disc_curve.df_between_dates(as_of, inst.expiry)?;
    let r_dom = zero_rate_from_df(df_dom, t, "QuantoOption domestic discount")?;

    // Get foreign rate
    let for_curve = curves.get_discount(inst.foreign_discount_curve_id.as_str())?;
    let df_for = for_curve.df_between_dates(as_of, inst.expiry)?;
    let r_for = zero_rate_from_df(df_for, t, "QuantoOption foreign discount")?;

    let spot_scalar = curves.get_price(&inst.spot_id)?;
    let spot = crate::metrics::scalar_numeric_value(spot_scalar);

    let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
        curves,
        inst.div_yield_id.as_ref(),
    )?;

    let sigma_equity = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
        &inst.pricing_overrides.market_quotes,
        curves,
        inst.vol_surface_id.as_str(),
        t,
        inst.equity_strike.amount(),
    )?;

    // Get FX volatility
    let sigma_fx = if let Some(fx_vol_id) = &inst.fx_vol_id {
        let fx_vol_surface = curves.get_surface(fx_vol_id.as_str())?;
        fx_vol_surface.value_clamped(t, 1.0)
    } else {
        return Err(finstack_core::Error::from(
            finstack_core::InputError::NotFound {
                id: "fx_vol_id".to_string(),
            },
        ));
    };

    Ok((spot, r_dom, r_for, q, sigma_equity, sigma_fx, t))
}

fn payoff_scale(inst: &QuantoOption) -> finstack_core::Result<f64> {
    // `inst.validate()` already ran at construction (builder + serde
    // `try_from` go through `validate`). Greek calculators that bump
    // instrument fields directly (e.g. `Correlation01Calculator`) validate
    // the bumped field locally. Re-running the full validation on every
    // pricing call cost ~3-4x for vanna/volga which call `value()` 4x.
    match (inst.underlying_quantity, inst.payoff_fx_rate) {
        (Some(quantity), Some(fx_rate)) => Ok(quantity * fx_rate),
        (None, None) => Ok(inst.notional.amount() / inst.equity_strike.amount()),
        _ => Err(finstack_core::Error::Validation(
            "QuantoOption requires both underlying_quantity and payoff_fx_rate when either is supplied"
                .to_string(),
        )),
    }
}

/// Quanto option analytical pricer.
pub(crate) struct QuantoOptionAnalyticalPricer;

impl QuantoOptionAnalyticalPricer {
    /// Create a new analytical quanto option pricer
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for QuantoOptionAnalyticalPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for QuantoOptionAnalyticalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::QuantoOption, ModelKey::QuantoBS)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
        let quanto = instrument
            .as_any()
            .downcast_ref::<QuantoOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::QuantoOption, instrument.key())
            })?;

        let (spot, r_dom, r_for, q, sigma_equity, sigma_fx, t) =
            collect_quanto_inputs(quanto, market, as_of).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if t <= 0.0 {
            return Ok(ValuationResult::stamped(
                quanto.id(),
                as_of,
                Money::new(0.0, quanto.quote_currency),
            ));
        }

        let price = match quanto.option_type {
            crate::instruments::OptionType::Call => quanto_call(
                spot,
                quanto.equity_strike.amount(),
                t,
                r_dom,
                r_for,
                q,
                sigma_equity,
                sigma_fx,
                quanto.correlation,
            ),
            crate::instruments::OptionType::Put => quanto_put(
                spot,
                quanto.equity_strike.amount(),
                t,
                r_dom,
                r_for,
                q,
                sigma_equity,
                sigma_fx,
                quanto.correlation,
            ),
        };

        let scale = payoff_scale(quanto).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let pv = Money::new(price * scale, quanto.quote_currency);
        Ok(ValuationResult::stamped(quanto.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Attributes;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use finstack_core::types::{CurveId, InstrumentId};
    use time::macros::date;

    /// Item 3 regression: `r_dom`/`r_for` must be recovered from *date-based*
    /// discount factors so that `e^{-r·t} == df_between_dates(as_of, expiry)`.
    /// The fixture uses a discount curve whose `base_date` precedes `as_of`,
    /// which makes the prior `curve.zero(t)` (time-axis interpolation) disagree
    /// with the actual discount factor over the option's life.
    #[test]
    fn quanto_inputs_use_date_based_discount_rates() {
        let curve_base = date!(2025 - 01 - 02);
        let as_of = date!(2025 - 07 - 01);
        let expiry = date!(2026 - 07 - 01);

        // Non-flat curves: the instantaneous forward rate rises across tenors,
        // so `df(t2)/df(t1) != df(t2 - t1)` and the curve-axis `zero()` lookup
        // genuinely disagrees with date-based discounting.
        let usd = DiscountCurve::builder("USD-OIS")
            .base_date(curve_base)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (0.5, (-0.01_f64).exp()),
                (1.5, (-0.09_f64).exp()),
                (3.0, (-0.30_f64).exp()),
            ])
            .build()
            .expect("usd curve");
        let jpy = DiscountCurve::builder("JPY-OIS")
            .base_date(curve_base)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (0.5, (-0.002_f64).exp()),
                (1.5, (-0.012_f64).exp()),
                (3.0, (-0.045_f64).exp()),
            ])
            .build()
            .expect("jpy curve");
        let eq_vol = VolSurface::builder("NKY-VOL")
            .expiries(&[2.0])
            .strikes(&[35000.0])
            .row(&[0.22])
            .build()
            .expect("equity vol");
        let fx_vol = VolSurface::builder("USDJPY-VOL")
            .expiries(&[2.0])
            .strikes(&[1.0])
            .row(&[0.10])
            .build()
            .expect("fx vol");
        let market = MarketContext::new()
            .insert(usd)
            .insert(jpy)
            .insert_surface(eq_vol)
            .insert_surface(fx_vol)
            .insert_price(
                "NKY-SPOT",
                MarketScalar::Price(Money::new(34000.0, Currency::JPY)),
            );

        let quanto = QuantoOption::builder()
            .id(InstrumentId::new("QUANTO-DISC"))
            .underlying_ticker("NKY".to_string())
            .equity_strike(Money::new(35000.0, Currency::JPY))
            .option_type(crate::instruments::OptionType::Call)
            .expiry(expiry)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .base_currency(Currency::JPY)
            .quote_currency(Currency::USD)
            .correlation(-0.2)
            .day_count(DayCount::Act365F)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("JPY-OIS"))
            .spot_id("NKY-SPOT".into())
            .vol_surface_id(CurveId::new("NKY-VOL"))
            .div_yield_id_opt(None)
            .fx_vol_id_opt(Some(CurveId::new("USDJPY-VOL")))
            .attributes(Attributes::new())
            .build()
            .expect("quanto");

        let (_spot, r_dom, r_for, _q, _se, _sf, t) =
            collect_quanto_inputs(&quanto, &market, as_of).expect("inputs");

        let df_dom = market
            .get_discount("USD-OIS")
            .expect("usd")
            .df_between_dates(as_of, expiry)
            .expect("df dom");
        let df_for = market
            .get_discount("JPY-OIS")
            .expect("jpy")
            .df_between_dates(as_of, expiry)
            .expect("df for");

        assert!(
            ((-r_dom * t).exp() - df_dom).abs() < 1e-12,
            "recovered r_dom must satisfy e^(-r·t)=df_between_dates: \
             e^(-r·t)={} df={df_dom}",
            (-r_dom * t).exp()
        );
        assert!(
            ((-r_for * t).exp() - df_for).abs() < 1e-12,
            "recovered r_for must satisfy e^(-r·t)=df_between_dates: \
             e^(-r·t)={} df={df_for}",
            (-r_for * t).exp()
        );

        // The buggy time-axis lookup disagrees: `zero(t)` reads df at curve
        // time `t` (anchored at base_date), not the option's `as_of→expiry` df.
        let bug_r_dom = market.get_discount("USD-OIS").expect("usd").zero(t);
        assert!(
            ((-bug_r_dom * t).exp() - df_dom).abs() > 1e-4,
            "fixture must expose the curve-axis bug for the domestic rate"
        );
    }
}
