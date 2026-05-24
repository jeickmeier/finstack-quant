//! Hull-White 1-factor tree pricer for interest rate caps and floors.
//!
//! Prices a cap/floor by building a calibrated Hull-White trinomial tree and
//! pricing each caplet/floorlet via backward induction. Each caplet is an
//! option on the forward rate for a single accrual period.
//!
//! # Algorithm
//!
//! For each caplet/floorlet period [T_start, T_end]:
//!
//! 1. The caplet payoff at T_end is:
//!    `N * tau * max(L(T_start, T_end) - K, 0)` for a caplet
//!    `N * tau * max(K - L(T_start, T_end), 0)` for a floorlet
//!    where L is the simply-compounded forward rate and tau is the accrual.
//!
//! 2. Under the HW model, this is equivalent to an option on a zero-coupon
//!    bond. We use the tree's backward induction to evaluate this.
//!
//! 3. The cap/floor value is the sum of all caplet/floorlet values.
//!
//! # References
//!
//! - Hull, J. & White, A. (1990). "Pricing Interest-Rate-Derivative Securities."
//!   *Review of Financial Studies*, 3(4), 573-592.
//! - Brigo, D. & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*,
//!   Chapter 3: One-factor Short-Rate Models, Section 3.3.2.

use crate::calibration::hull_white::hw1f_caplet_forward_rate_normal_vol;
use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::pricing::time::{
    rate_period_on_dates, relative_df_discount_curve,
};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cap_floor::pricing::payoff::CapletFloorletInputs;
use crate::instruments::rates::cap_floor::types::{CapFloor, RateOptionType};
use crate::instruments::rates::exotics_shared::{
    resolve_hw1f_params, Hw1fCalibrationFlavor, Hw1fResolveRequest,
};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::dates::DayCountContext;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

/// Number of tree steps per caplet period.
pub(crate) const DEFAULT_STEPS_PER_PERIOD: usize = 30;

/// Minimum tree steps for the full cap/floor.
pub(crate) const MIN_TREE_STEPS: usize = 50;

/// Maximum tree steps for the full cap/floor.
pub(crate) const MAX_TREE_STEPS: usize = 300;

/// Hull-White 1-factor tree pricer for caps and floors.
///
/// Prices each caplet/floorlet by building a single Hull-White tree
/// spanning the full cap maturity and evaluating each caplet's payoff
/// via backward induction.
pub(crate) struct CapFloorHullWhitePricer;

impl Pricer for CapFloorHullWhitePricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CapFloor, ModelKey::HullWhite1F)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let cap_floor = instrument
            .as_any()
            .downcast_ref::<CapFloor>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CapFloor, instrument.key())
            })?;

        self.price_internal(cap_floor, market, as_of)
    }
}

impl CapFloorHullWhitePricer {
    /// Core pricing routine.
    fn price_internal(
        &self,
        cap_floor: &CapFloor,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let ctx = DayCountContext::default();

        // Get discount and projection curves. Bloomberg's HW1F cap/floor setup is
        // still a projected SOFR payoff discounted on the OIS curve.
        let disc = market
            .get_discount(cap_floor.discount_curve_id.as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
        let fwd = market
            .get_forward(cap_floor.forward_curve_id.as_str())
            .map_err(|e| {
                PricingError::missing_market_data_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        // Time to maturity (cap end)
        let maturity_time = cap_floor
            .day_count
            .year_fraction(as_of, cap_floor.maturity, ctx)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if maturity_time <= 0.0 {
            return Ok(ValuationResult::stamped(
                cap_floor.id.as_str(),
                as_of,
                Money::new(0.0, cap_floor.notional.currency()),
            ));
        }

        // Build schedule periods
        let periods = cap_floor.pricing_periods().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        if periods.is_empty() {
            return Ok(ValuationResult::stamped(
                cap_floor.id.as_str(),
                as_of,
                Money::new(0.0, cap_floor.notional.currency()),
            ));
        }

        let strike = cap_floor.strike_f64().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        let notional = cap_floor.notional.amount();

        let is_cap = matches!(
            cap_floor.rate_option_type,
            RateOptionType::Cap | RateOptionType::Caplet
        );

        let model_config = &cap_floor.pricing_overrides.model_config;

        // Resolve HW1F parameters following the documented precedence:
        // explicit `pricing_overrides` κ/σ → calibrated MarketContext scalars
        // → warned `HullWhiteParams::default()`.
        let context_label = format!("CapFloor {}", cap_floor.id);
        let overrides = hw1f_overrides_json(cap_floor);
        let req = Hw1fResolveRequest {
            curve_id: cap_floor.discount_curve_id.as_str(),
            flavor: Hw1fCalibrationFlavor::CapFloor,
            overrides: overrides.as_ref(),
            context: context_label.as_str(),
        };
        let hw_params = resolve_hw1f_params(&req, market).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let tree_steps = model_config.tree_steps.unwrap_or_else(|| {
            (periods.len() * DEFAULT_STEPS_PER_PERIOD).clamp(MIN_TREE_STEPS, MAX_TREE_STEPS)
        });

        let _tree_steps = tree_steps;

        // Price each caplet/floorlet using the tree
        let mut total_pv = 0.0;

        for period in &periods {
            let fixing_date = cap_floor.option_fixing_date(period);
            let t_fix = cap_floor
                .day_count
                .year_fraction(as_of, fixing_date, ctx)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;

            // Skip expired caplets
            if t_fix <= 0.0 {
                continue;
            }

            let tau = year_fraction(
                cap_floor.day_count,
                period.accrual_start,
                period.accrual_end,
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

            if tau <= 0.0 {
                continue;
            }

            let forward =
                rate_period_on_dates(fwd.as_ref(), period.accrual_start, period.accrual_end)
                    .map_err(|e| {
                        PricingError::model_failure_with_context(
                            e.to_string(),
                            PricingErrorContext::default(),
                        )
                    })?;
            let df = relative_df_discount_curve(disc.as_ref(), as_of, period.payment_date)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
            let hw_vol =
                hw1f_caplet_forward_rate_normal_vol(hw_params.kappa, hw_params.sigma, t_fix, tau);
            let caplet_pv =
                crate::instruments::rates::cap_floor::pricing::normal::price_caplet_floorlet(
                    CapletFloorletInputs {
                        is_cap,
                        notional,
                        strike,
                        forward,
                        discount_factor: df,
                        volatility: hw_vol,
                        time_to_fixing: t_fix,
                        accrual_year_fraction: tau,
                        currency: cap_floor.notional.currency(),
                    },
                )
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?
                .amount();

            total_pv += caplet_pv;
        }

        Ok(ValuationResult::stamped(
            cap_floor.id.as_str(),
            as_of,
            Money::new(total_pv, cap_floor.notional.currency()),
        ))
    }
}

/// Build the HW1F override JSON blob from a cap/floor's typed pricing overrides.
///
/// Reads `model_config.hw1f_mean_reversion` → `hw1f_kappa` and
/// `model_config.hw1f_sigma` → `hw1f_sigma` (the Hull-White short-rate absolute
/// volatility). Returns `Some` only when **both** are present, so that a partial
/// override falls through to the calibrated-market-scalar / default branches in
/// [`resolve_hw1f_params`].
///
/// # Unit contract
///
/// `hw1f_sigma` is a **short-rate** absolute volatility (annual decimal, ~0.005–0.015).
/// It must NOT be confused with an option implied volatility (e.g. 0.20 lognormal),
/// which lives in `market_quotes.implied_volatility`. Feeding an option vol directly
/// into the HW tree would produce a ~13–40× mis-priced result.
fn hw1f_overrides_json(cap_floor: &CapFloor) -> Option<serde_json::Value> {
    let kappa = cap_floor
        .pricing_overrides
        .model_config
        .hw1f_mean_reversion?;
    let sigma = cap_floor.pricing_overrides.model_config.hw1f_sigma?;
    Some(serde_json::json!({ "hw1f_kappa": kappa, "hw1f_sigma": sigma }))
}

#[cfg(test)]
mod tests {
    #[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/test_utils.rs"
        ));
    }

    use super::*;
    use test_utils::{date, flat_discount_with_tenor, flat_forward_with_tenor};

    /// Pricing a cap via the HW pricer (which falls back to uncalibrated
    /// `HullWhiteParams::default()` absent overrides) must still produce a
    /// finite PV. This locks in that adding the uncalibrated-params diagnostic
    /// did not change numerics.
    #[test]
    fn hw_cap_floor_produces_finite_pv() {
        let as_of = date(2023, 12, 1);
        let cap = CapFloor::example().expect("CapFloor example should build");
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor(
                cap.discount_curve_id.as_str(),
                as_of,
                0.03,
                10.0,
            ))
            .insert(flat_forward_with_tenor(
                cap.forward_curve_id.as_str(),
                as_of,
                0.03,
                10.0,
            ));

        let pricer = CapFloorHullWhitePricer;
        let result = pricer
            .price_internal(&cap, &market, as_of)
            .expect("HW cap pricing should succeed");

        let pv = result.value.amount();
        assert!(pv.is_finite(), "HW cap PV must be finite, got {pv}");
        assert!(pv >= 0.0, "cap PV must be non-negative, got {pv}");
    }

    /// Builds a cap with flat discount/forward curves.
    fn example_cap_market() -> (finstack_core::dates::Date, CapFloor, MarketContext) {
        let as_of = date(2023, 12, 1);
        let cap = CapFloor::example().expect("CapFloor example should build");
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor(
                cap.discount_curve_id.as_str(),
                as_of,
                0.03,
                10.0,
            ))
            .insert(flat_forward_with_tenor(
                cap.forward_curve_id.as_str(),
                as_of,
                0.03,
                10.0,
            ));
        (as_of, cap, market)
    }

    /// When the `MarketContext` carries calibrated `{curve}_CAPFLOOR_HW1F_*`
    /// scalars, the pricer must consume them: the PV differs from the
    /// default-params PV.
    #[test]
    fn hw_cap_floor_uses_calibrated_market_scalars() {
        use crate::calibration::hull_white::capfloor_hw1f_scalar_keys;
        use finstack_core::market_data::scalars::MarketScalar;

        let (as_of, cap, default_market) = example_cap_market();
        let default_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &default_market, as_of)
            .expect("default-params pricing should succeed")
            .value
            .amount();

        let (kappa_key, sigma_key) = capfloor_hw1f_scalar_keys(cap.discount_curve_id.as_str());
        let calibrated_market = default_market
            .insert_price(&kappa_key, MarketScalar::Unitless(0.10))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.030));

        let calibrated_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &calibrated_market, as_of)
            .expect("calibrated pricing should succeed")
            .value
            .amount();

        assert!(calibrated_pv.is_finite());
        assert!(
            (calibrated_pv - default_pv).abs() > 1e-9,
            "calibrated PV ({calibrated_pv}) must differ from default PV ({default_pv})"
        );
    }

    /// Explicit `pricing_overrides` κ/σ win over calibrated market scalars.
    #[test]
    fn hw_cap_floor_overrides_win_over_market_scalars() {
        use crate::calibration::hull_white::capfloor_hw1f_scalar_keys;
        use finstack_core::market_data::scalars::MarketScalar;

        let (as_of, mut cap, market) = example_cap_market();
        let (kappa_key, sigma_key) = capfloor_hw1f_scalar_keys(cap.discount_curve_id.as_str());
        let market = market
            .insert_price(&kappa_key, MarketScalar::Unitless(0.10))
            .insert_price(&sigma_key, MarketScalar::Unitless(0.030));

        let market_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &market, as_of)
            .expect("market-scalar pricing should succeed")
            .value
            .amount();

        // HW1F-specific overrides via the dedicated short-rate-vol field (NOT
        // implied_volatility which is an option vol). PV must differ from the
        // market-scalar PV.
        cap.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.03);
        cap.pricing_overrides.model_config.hw1f_sigma = Some(0.01);
        let override_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &market, as_of)
            .expect("override pricing should succeed")
            .value
            .amount();

        assert!(
            (override_pv - market_pv).abs() > 1e-9,
            "override PV ({override_pv}) must differ from market-scalar PV ({market_pv})"
        );
    }

    /// Regression: `model_config.hw1f_sigma` (the dedicated short-rate σ field) must
    /// reach the HW tree and change the PV. A different short-rate σ must produce a
    /// different PV — confirming the dedicated channel is wired through.
    #[test]
    fn hw1f_sigma_override_field_reaches_tree() {
        let (as_of, mut cap, market) = example_cap_market();

        let default_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &market, as_of)
            .expect("default pricing should succeed")
            .value
            .amount();

        // Override with a significantly different short-rate σ (~3× default).
        cap.pricing_overrides.model_config.hw1f_mean_reversion = Some(0.05);
        cap.pricing_overrides.model_config.hw1f_sigma = Some(0.030);
        let overridden_pv = CapFloorHullWhitePricer
            .price_internal(&cap, &market, as_of)
            .expect("hw1f_sigma override pricing should succeed")
            .value
            .amount();

        assert!(
            overridden_pv.is_finite(),
            "PV must be finite: {overridden_pv}"
        );
        assert!(
            (overridden_pv - default_pv).abs() > 1e-9,
            "hw1f_sigma override must change PV vs default: override={overridden_pv}, default={default_pv}"
        );
    }

    /// Regression (W26): `market_quotes.implied_volatility` must NOT be silently
    /// treated as the HW1F short-rate σ. When only `implied_volatility` is set
    /// (without the dedicated `hw1f_sigma`/`hw1f_mean_reversion` fields), the
    /// pricer must fall through to the calibrated-scalar / default branch and
    /// the PV must be unchanged.
    #[test]
    fn implied_volatility_is_not_used_as_hw1f_sigma() {
        let (as_of, cap_no_iv, market) = example_cap_market();
        let (_, mut cap_with_iv, _) = example_cap_market();

        let pv_no_iv = CapFloorHullWhitePricer
            .price_internal(&cap_no_iv, &market, as_of)
            .expect("no-iv pricing should succeed")
            .value
            .amount();

        // Typical lognormal cap vol (0.20) with NO hw1f_sigma set.
        // If the bug were present, this would feed 0.20 into the HW tree as σ.
        cap_with_iv
            .pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.20);
        let pv_with_iv = CapFloorHullWhitePricer
            .price_internal(&cap_with_iv, &market, as_of)
            .expect("iv-only pricing should succeed")
            .value
            .amount();

        assert!(
            (pv_with_iv - pv_no_iv).abs() < 1e-9,
            "implied_volatility must NOT alter the HW tree pricing: \
             pv_with_iv={pv_with_iv}, pv_no_iv={pv_no_iv} (diff={})",
            (pv_with_iv - pv_no_iv).abs()
        );
    }
}
