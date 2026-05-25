//! Equity option Heston PDE pricer using 2D Modified Craig-Sneyd ADI finite
//! differences.
//!
//! Solves the Heston PDE in (log-spot, variance) coordinates on a tensor-product
//! grid using the Modified Craig-Sneyd (MCS) ADI splitting scheme. Heston model
//! parameters are sourced from required market scalars.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::pricer::{
    collect_inputs_extended, reject_future_discrete_dividends_for_stochastic_vol,
};
use crate::instruments::equity::equity_option::types::EquityOption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

use crate::instruments::common_impl::parameters::OptionType;
use crate::models::closed_form::heston::HestonParams as ClosedFormHestonParams;
use crate::models::pde::{Grid1D, Grid2D, HestonPde, Solver2D};

/// Equity option pricer using 2D ADI PDE (Modified Craig-Sneyd) with Heston
/// stochastic volatility dynamics.
///
/// Solves the Heston PDE on a tensor-product (log-spot x variance) grid.
/// Heston parameters are read from market scalars using the same convention
/// as [`EquityOptionHestonFourierPricer`].
pub(crate) struct EquityOptionHestonPdePricer {
    /// Number of spatial grid points along the x (log-spot) axis.
    space_points_x: usize,
    /// Number of spatial grid points along the v (variance) axis.
    space_points_v: usize,
    /// Number of time steps.
    time_steps: usize,
}

impl Default for EquityOptionHestonPdePricer {
    fn default() -> Self {
        Self {
            space_points_x: 200,
            space_points_v: 80,
            time_steps: 100,
        }
    }
}

impl EquityOptionHestonPdePricer {
    /// Price the equity option via the 2D Heston PDE.
    fn price_internal(
        &self,
        inst: &EquityOption,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<Money, PricingError> {
        reject_future_discrete_dividends_for_stochastic_vol(
            inst,
            as_of,
            ModelKey::PdeAdi2D,
            "Heston PDE",
        )?;

        let inputs = collect_inputs_extended(inst, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
            )
        })?;
        let spot = inputs.spot;
        let r = inputs.r;
        let q = inputs.q;
        let t = inputs.t_vol;
        let ccy = inst.notional.currency();

        if t <= 0.0 {
            let intrinsic = match inst.option_type {
                OptionType::Call => (spot - inst.strike).max(0.0),
                OptionType::Put => (inst.strike - spot).max(0.0),
            };
            return Ok(Money::new(intrinsic * inst.notional.amount(), ccy));
        }

        // Source production Heston parameters from explicit market scalars.
        // Validation is still enforced inside `HestonParams::new`.
        let cf_params = ClosedFormHestonParams::from_market_strict(market, r, q).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
            )
        })?;
        let theta_v = cf_params.theta;
        let v0 = cf_params.v0;

        let is_call = matches!(inst.option_type, OptionType::Call);

        let pde = HestonPde {
            r: cf_params.r,
            q: cf_params.q,
            kappa: cf_params.kappa,
            theta_v: cf_params.theta,
            sigma_v: cf_params.sigma_v,
            rho: cf_params.rho,
            strike: inst.strike,
            is_call,
        };

        // X-grid: log-spot concentrated near ln(strike)
        let x_min = (spot * 0.05).ln();
        let x_max = (spot * 10.0).ln();
        let gx = Grid1D::sinh_concentrated(x_min, x_max, self.space_points_x, spot.ln(), 0.1)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
                )
            })?;

        // V-grid: variance from near-zero to well above long-run level
        let v_min = 0.001;
        let v_max = 1.5_f64.max(5.0 * theta_v);
        let gv = Grid1D::sinh_concentrated(v_min, v_max, self.space_points_v, theta_v, 0.15)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
                )
            })?;

        let grid = Grid2D::new(gx, gv);

        let solver = Solver2D::builder()
            .grid(grid)
            .craig_sneyd(self.time_steps)
            .build()
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
                )
            })?;

        let solution = solver.solve(&pde, t).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
            )
        })?;
        let price = solution.interpolate(spot.ln(), v0);

        Ok(Money::new(price * inst.notional.amount(), ccy))
    }
}

impl Pricer for EquityOptionHestonPdePricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::EquityOption, ModelKey::PdeAdi2D)
    }

    #[tracing::instrument(
        name = "equity_option.heston_pde2d.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let equity_option = instrument
            .as_any()
            .downcast_ref::<EquityOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::EquityOption, instrument.key())
            })?;

        let pv = self.price_internal(equity_option, market, as_of)?;

        Ok(ValuationResult::stamped(equity_option.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::{ExerciseStyle, OptionType};
    use crate::instruments::{Attributes, PricingOverrides, SettlementType};
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
            .build()
            .expect("curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.20])
            .build()
            .expect("surface");
        MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
    }

    fn option(expiry: Date) -> EquityOption {
        EquityOption::builder()
            .id(InstrumentId::new("EQ-OPT-PDE-TEST"))
            .underlying_ticker("SPX".to_string())
            .strike(100.0)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .notional(Money::new(100.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .settlement(SettlementType::Cash)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("equity option")
    }

    #[test]
    fn heston_pde_rejects_future_discrete_dividend() {
        let as_of = date(2025, 1, 1);
        let mut inst = option(date(2026, 1, 1));
        inst.discrete_dividends = vec![(date(2025, 7, 1), 2.0)];

        let err = EquityOptionHestonPdePricer::default()
            .price_internal(&inst, &market(as_of), as_of)
            .expect_err("Heston PDE must reject discrete dividends");
        let msg = err.to_string();
        assert!(
            msg.contains("discrete dividends"),
            "unexpected error message: {msg}"
        );
    }
}
