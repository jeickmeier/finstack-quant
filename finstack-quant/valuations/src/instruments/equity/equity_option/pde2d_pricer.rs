//! Equity option Heston PDE pricer using 2D Modified Craig-Sneyd ADI finite
//! differences.
//!
//! Solves the Heston PDE in (log-spot, variance) coordinates on a tensor-product
//! grid using the Modified Craig-Sneyd (MCS) ADI splitting scheme. Heston model
//! parameters are sourced from required market scalars.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::pricing::{
    collect_inputs_extended, reject_future_discrete_dividends_for_stochastic_vol,
    resolve_lifecycle_value,
};
use crate::instruments::equity::equity_option::types::EquityOption;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

use crate::instruments::common_impl::parameters::OptionType;
use finstack_quant_models::pde::{CraigSneydStepper, Grid1D, Grid2D, HestonPde, Solver2D};

/// Reject an unstable PDE result rather than returning an arbitrage-violating
/// European option value. Only floating-point roundoff at a bound is removed.
fn checked_pde_value(value: f64, upper_bound: f64) -> finstack_quant_core::Result<f64> {
    let roundoff = 128.0 * f64::EPSILON * upper_bound.abs().max(1.0);
    if !value.is_finite()
        || !upper_bound.is_finite()
        || upper_bound < 0.0
        || value < -roundoff
        || value > upper_bound + roundoff
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Heston PDE result {value} violates European option bounds [0, {upper_bound}]; refine or change the numerical grid"
        )));
    }
    Ok(value.clamp(0.0, upper_bound))
}

/// Equity option pricer using 2D ADI PDE (Modified Craig-Sneyd) with Heston
/// stochastic volatility dynamics.
///
/// Solves the Heston PDE on a tensor-product (log-spot x variance) grid.
/// Heston parameters are read from market scalars using the same convention
/// as `EquityOptionHestonFourierPricer`.
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
        if let Some(value) = resolve_lifecycle_value(inst, market, as_of).map_err(|error| {
            PricingError::model_failure_with_context(
                error.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
            )
        })? {
            return Ok(value);
        }
        if !matches!(
            inst.exercise_style,
            crate::instruments::ExerciseStyle::European
        ) {
            return Err(PricingError::model_failure_with_context(
                "Heston PDE supports European exercise only",
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
            ));
        }
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
        let ccy = inst.currency;

        if t <= 0.0 {
            let intrinsic = match inst.option_type {
                OptionType::Call => (spot - inst.strike).max(0.0),
                OptionType::Put => (inst.strike - spot).max(0.0),
            };
            return Money::new(intrinsic * inst.quantity, ccy).map_err(|error| {
                crate::pricer::PricingError::from_core(
                    error,
                    crate::pricer::PricingErrorContext::from_instrument(inst)
                        .model(ModelKey::PdeAdi2D),
                )
            });
        }

        // Source production Heston parameters from explicit market scalars.
        // Validation is still enforced inside `HestonParams::new`.
        let cf_params = crate::instruments::equity::equity_option::heston_market::heston_params_from_market_strict(market, r, q).map_err(|e| {
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

        let mean_reversion_fraction = -(-cf_params.kappa * t).exp_m1();
        let initial_variance_weight = mean_reversion_fraction / cf_params.kappa;
        let mean_integrated_variance =
            v0 * initial_variance_weight + theta_v * (t - initial_variance_weight).max(0.0);
        // High accepted v0 also widens the log-spot distribution. A fixed
        // 10x upper spot boundary remains visible after mesh refinement in
        // those states. Keep at least six integrated-volatility standard
        // deviations, preserving the original local concentration scale.
        let log_tail = 6.0 * mean_integrated_variance.sqrt();
        let x_min = spot.ln() + 0.05_f64.ln().min(-log_tail);
        let x_max = spot.ln() + 10.0_f64.ln().max(log_tail);
        let x_intensity = 0.1 * 200.0_f64.ln() / (x_max - x_min);
        let gx =
            Grid1D::sinh_concentrated(x_min, x_max, self.space_points_x, spot.ln(), x_intensity)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
                    )
                })?;
        // Include the attainable zero boundary and the *actual* initial state.
        // Bound the CIR variance on [0,T]: e(1-e) increases until e=1/2,
        // while (1-e)^2 increases throughout. The mean stays between v0 and
        // theta; six standard deviations above both provide a tail margin.
        // exp_m1 retains accuracy
        // for short maturities and slow mean reversion.
        let initial_variance_factor = if mean_reversion_fraction <= 0.5 {
            mean_reversion_fraction * (1.0 - mean_reversion_fraction)
        } else {
            0.25
        };
        // Divide the small exponential factors first: sigma²/kappa can
        // overflow for valid near-zero kappa even though the variance has a
        // finite continuous limit.
        let variance_bound = cf_params.sigma_v.powi(2)
            * (v0 * (initial_variance_factor / cf_params.kappa)
                + 0.5
                    * theta_v
                    * (mean_reversion_fraction / cf_params.kappa)
                    * mean_reversion_fraction);
        let v_min = 0.0;
        let v_max = 1.5_f64
            .max(5.0 * theta_v)
            .max(v0.max(theta_v) + 6.0 * variance_bound.sqrt())
            .max(1.5 * v0);
        let gv =
            Grid1D::sinh_concentrated(v_min, v_max, self.space_points_v, theta_v.min(v0), 0.025)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
                    )
                })?;

        if !v_max.is_finite() || !(gv.x_min() <= v0 && v0 < gv.x_max()) {
            return Err(PricingError::model_failure_with_context(
                "Heston initial variance is outside the finite PDE domain",
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
            ));
        }
        let grid = Grid2D::new(gx, gv);

        // MCS with implicit split start-up: two pairs of halfsteps damp the
        // payoff-kink error before the second-order MCS scheme takes over
        // (In 't Hout & Foulon 2010) — the configuration the tight-tolerance
        // bridge2d reference tests use.
        let solver = Solver2D::new(grid, CraigSneydStepper::with_damping(2, self.time_steps));

        let solution = solver.solve(&pde, t).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
            )
        })?;
        let upper_bound = if is_call {
            spot * (-q * t).exp()
        } else {
            inst.strike * (-r * t).exp()
        };
        let price = checked_pde_value(solution.interpolate(spot.ln(), v0), upper_bound).map_err(
            |error| {
                PricingError::from_core(
                    error,
                    PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
                )
            },
        )?;

        Money::new(price * inst.quantity, ccy).map_err(|error| {
            crate::pricer::PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(inst).model(ModelKey::PdeAdi2D),
            )
        })
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
        let equity_option = expect_inst::<EquityOption>(instrument, InstrumentType::EquityOption)?;

        let pv = self.price_internal(equity_option, market, as_of)?;

        Ok(ValuationResult::stamped(equity_option.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::{ExerciseStyle, OptionType};
    use crate::instruments::{Attributes, SettlementType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::{CurveId, InstrumentId};
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
            .quantity(100.0)
            .currency(Currency::USD)
            .day_count(DayCount::Act365F)
            .settlement(SettlementType::Cash)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
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

    #[test]
    fn heston_pde_rejects_unstable_prices() {
        for value in [f64::NAN, f64::INFINITY, -1e48, 100.01] {
            assert!(checked_pde_value(value, 100.0).is_err());
        }
        assert!(checked_pde_value(1.0, f64::INFINITY).is_err());
        assert!(checked_pde_value(1.0, -1.0).is_err());
        for value in [0.0, 7.5, 100.0] {
            assert_eq!(checked_pde_value(value, 100.0).expect("valid price"), value);
        }
        assert_eq!(
            checked_pde_value(-f64::EPSILON, 100.0).expect("roundoff"),
            0.0
        );
    }

    fn variance_market(v0: f64) -> MarketContext {
        let mut market = market(date(2025, 1, 1));
        for (name, value) in [
            ("HESTON_KAPPA", 2.0),
            ("HESTON_THETA", 0.04),
            ("HESTON_SIGMA_V", 0.3),
            ("HESTON_RHO", -0.7),
            ("HESTON_V0", v0),
        ] {
            market = market.insert_price(name, MarketScalar::Unitless(value));
        }
        market
    }

    fn reference(v0: f64, sigma: f64) -> f64 {
        use finstack_quant_models::closed_form::heston::{
            heston_call_price_fourier, HestonPricingParams,
        };
        let params = HestonPricingParams::new(0.03, 0.0, 2.0, 0.04, sigma, -0.7, v0)
            .expect("Heston parameters");
        heston_call_price_fourier(100.0, 100.0, 1.0, &params, None).expect("Fourier reference")
    }

    #[test]
    fn heston_pde_includes_low_and_high_initial_variance() {
        let as_of = date(2025, 1, 1);
        let mut inst = option(date(2026, 1, 1));
        inst.quantity = 1.0;
        let mut previous = 0.0;
        for v0 in [0.0001, 0.0009, 0.0011, 0.04, 1.4, 1.6, 2.0] {
            let actual = EquityOptionHestonPdePricer::default()
                .price_internal(&inst, &variance_market(v0), as_of)
                .expect("PDE price")
                .amount();
            let expected = reference(v0, 0.3);
            // A money tolerance plus a relative stress allowance on a unit
            // option. This detects the old state substitution (4+ at v0=2).
            assert!(
                (actual - expected).abs() < 0.10 + 0.005 * expected,
                "v0={v0}: PDE={actual}, Fourier={expected}"
            );
            assert!(actual > previous + 1e-6,
                "initial variance must not be silently clamped: v0={v0}, PV={actual}, previous={previous}");
            previous = actual;
        }
    }

    #[test]
    fn heston_pde_low_and_high_variance_mesh_refinement() {
        let as_of = date(2025, 1, 1);
        let mut inst = option(date(2026, 1, 1));
        inst.quantity = 1.0;
        // Include attainable-zero-variance dynamics (sigma=.6 violates the
        // Feller condition) as well as a high-initial-variance stress.
        for (v0, sigma) in [(0.0001, 0.3), (0.0001, 0.6), (2.0, 0.3)] {
            let market =
                variance_market(v0).insert_price("HESTON_SIGMA_V", MarketScalar::Unitless(sigma));
            let evaluate = |nx, nv, nt| {
                EquityOptionHestonPdePricer {
                    space_points_x: nx,
                    space_points_v: nv,
                    time_steps: nt,
                }
                .price_internal(&inst, &market, as_of)
                .expect("refined PDE")
                .amount()
            };
            let coarse = evaluate(160, 80, 100);
            let time_refined = evaluate(160, 80, 200);
            let space_refined = evaluate(320, 160, 200);
            let expected = reference(v0, sigma);
            assert!(
                (time_refined - coarse).abs() < 0.05,
                "time refinement v0={v0}: {coarse} -> {time_refined}"
            );
            assert!(
                (space_refined - time_refined).abs() < 0.15,
                "space refinement v0={v0}: {time_refined} -> {space_refined}"
            );
            assert!(
                (space_refined - expected).abs() < 0.10,
                "refined v0={v0}: PDE={space_refined}, Fourier={expected}"
            );
        }
    }
}
