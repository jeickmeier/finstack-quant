//! Equity option Heston PDE pricer using 2D Modified Craig-Sneyd ADI finite
//! differences.
//!
//! Solves the Heston PDE in (log-spot, variance) coordinates on a tensor-product
//! grid using the Modified Craig-Sneyd (MCS) ADI splitting scheme. Heston model
//! parameters are sourced from market scalars with sensible defaults.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::pricer::collect_inputs_extended;
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

        // Heston parameters: source from market scalars and fall back to
        // centralized defaults; validation (positive κ/θ/σᵥ/v₀, ρ ∈ (−1, 1))
        // is enforced by `HestonParams::from_market`.
        let cf_params = ClosedFormHestonParams::from_market(market, r, q).map_err(|e| {
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
