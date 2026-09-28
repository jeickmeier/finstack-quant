//! Rho calculator for cliquet options.
//!
//! Computes rho (interest rate sensitivity) via finite differences: bump the
//! discount curve by the resolved `rate_bump_bp` (default 1bp), reprice, and
//! report the PV change per 1bp.
//!
//! Units & sign:
//! - Rho is per +1bp parallel discount move
//! - Rho = (PV(rate + bump) − PV(base)) / rate_bump_bp
//! - Positive Rho means the instrument gains value when rates go up
//!
//! The payoff settles at `expiry`, which can fall after the last reset date,
//! so a cliquet whose resets are all observed still carries discount risk
//! until `expiry`.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::cliquet_option::CliquetOption;
use crate::metrics::bump_discount_curve_parallel;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Rho calculator for cliquet options.
pub struct RhoCalculator;

impl MetricCalculator for RhoCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;
        let option: &CliquetOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        // Discount risk runs to settlement at `expiry`, not to the last reset.
        let t = option.day_count.year_fraction(
            as_of,
            option.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // The helper's argument is in basis points (1.0 = 1bp).
        let curves_bumped =
            bump_discount_curve_parallel(&context.curves, &option.discount_curve_id, bump_bp)?;

        // Reprice with bumped curve
        let pv_bumped = option.value(&curves_bumped, as_of)?.amount();

        // Rho per 1bp = (PV(rate + bump) − PV(base)) / bump_bp
        Ok((pv_bumped - base_pv) / bump_bp)
    }
}

#[cfg(test)]
mod tests {
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::instruments::equity::cliquet_option::CliquetOption;
    use crate::instruments::MetricPricingOverrides;
    use crate::metrics::{standard_registry, MetricContext, MetricId};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
    use std::sync::Arc;
    use time::Month;

    fn date(y: i32, m: Month, d: u8) -> Date {
        Date::from_calendar_date(y, m, d).expect("valid date")
    }

    fn market(as_of: Date) -> MarketContext {
        // Log-linear discount factors keep the zero rate flat between knots, so
        // a parallel continuously compounded shock multiplies every DF by
        // exp(-δr·t) exactly, not only at the knots.
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-0.04_f64 * 5.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0, 140.0])
            .row(&[0.20, 0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20, 0.20])
            .build()
            .expect("surface");
        MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
            .insert_price("SPX-DIV", MarketScalar::Unitless(0.01))
    }

    /// A cliquet whose resets are all observed but that settles later.
    fn fully_observed_option() -> CliquetOption {
        let mut option = CliquetOption::builder()
            .id(InstrumentId::new("CLIQ-RHO"))
            .underlying_ticker("SPX".to_string())
            .reset_dates(vec![
                date(2024, Month::June, 30),
                date(2024, Month::December, 31),
            ])
            .expiry(date(2025, Month::December, 31))
            .local_cap(0.05)
            .local_floor(0.0)
            .global_cap(0.20)
            .global_floor(0.0)
            .notional(Money::from((100_000_i64, Currency::USD)))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .path_model(crate::instruments::equity::EquityPathModel::AtmTermGbm)
            .div_yield_id_opt(Some(PriceId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
            .expect("cliquet option");
        option.initial_level = Some(100.0);
        option.past_fixings = vec![
            (date(2024, Month::June, 30), 103.0),
            (date(2024, Month::December, 31), 105.06),
        ];
        option
    }

    fn rho(option: &CliquetOption, market: &MarketContext, as_of: Date) -> (f64, f64) {
        let pv = option.value(market, as_of).expect("pv");
        let mut ctx = MetricContext::new(
            Arc::new(option.clone()),
            Arc::new(market.clone()),
            as_of,
            pv,
            MetricContext::default_config(),
        );
        ctx.set_metric_overrides(Some(option.metric_pricing_overrides.clone()));
        let res = standard_registry()
            .compute(&[MetricId::Rho], &mut ctx)
            .expect("rho");
        (pv.amount(), res[&MetricId::Rho])
    }

    /// A fully observed cliquet is a fixed cashflow at `expiry`, so its rho is
    /// the discount sensitivity PV·(exp(−δr·τ) − 1) per bp of δr, with τ the
    /// curve time from `as_of` to `expiry`. The two old defects both zeroed
    /// it: the time guard stopped at the last reset date, and the bump was
    /// 0.0001bp. Tolerance 1e-9·|PV| covers only the f64 rounding of the
    /// repricing (the log-linear curve makes the shock exact).
    #[test]
    fn fully_observed_cliquet_rho_is_discount_sensitivity() {
        let as_of = date(2025, Month::January, 10);
        let mkt = market(as_of);
        let tau = DayCount::Act365F
            .year_fraction(
                as_of,
                date(2025, Month::December, 31),
                DayCountContext::default(),
            )
            .expect("tau");

        for bump_bp in [None, Some(10.0)] {
            let mut option = fully_observed_option();
            if let Some(bp) = bump_bp {
                option.metric_pricing_overrides =
                    MetricPricingOverrides::default().with_rate_bump_bp(bp);
            }
            let (pv, rho) = rho(&option, &mkt, as_of);
            let bp = bump_bp.unwrap_or(1.0);
            let expected = pv * ((-bp * 1e-4 * tau).exp() - 1.0) / bp;
            assert!(pv > 0.0, "locked-in payoff must be positive: {pv}");
            assert!(
                (rho - expected).abs() <= 1e-9 * pv.abs(),
                "rho at {bp}bp: expected {expected}, got {rho}"
            );
        }
    }
}
